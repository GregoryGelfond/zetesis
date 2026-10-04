//! Closed model queries retain scoped source coordinates until final publication.

use crate::ProgramSite;
use themelios_program::program::DefaultNegation;
use zetesis_core::catalog::Error as CatalogError;
use zetesis_objective::{
    Condition, ConditionError, ConditionFailure, ConditionIndex, ConditionNode,
};

use super::Context;
use crate::formula_binding::Binding;
use crate::formula_ir::LiteralIr;
use crate::formula_support::{Counters, Publication, SourceSelection, StorageLease};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

/// No query publishes a snapshot while source identity admission remains active.
/// Atom operands name this selection's local coordinates, not source positions.
pub(crate) struct PendingCondition {
    atoms: SourceSelection,
    query: Query,
}

impl PendingCondition {
    pub(crate) fn new(atoms: SourceSelection, query: Query) -> Self {
        Self { atoms, query }
    }

    pub(crate) fn publish(
        self,
        publication: &mut Publication<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Condition, FormulaFailure> {
        let Self {
            atoms,
            query: Query { nodes, mut lease },
        } = self;
        publication.check_lease(&lease, counters, location)?;
        let atoms = publication.atoms(atoms, limits, counters, location)?;
        let catalog_bytes = atoms
            .storage_with(|| counters.work(limits, location))?
            .bytes;
        let buffer_bytes = nodes.capacity() as u128 * size_of::<ConditionIndex>() as u128;
        let remaining = publication.remaining_bytes(limits, counters, location)?;
        // The existing constructor excludes caller input. Its capacity check can
        // therefore admit the replacement envelope while our header and buffer
        // remain charged, without guessing the library's private envelope size.
        let base = catalog_bytes + buffer_bytes;
        let source_bound = base + remaining as u128;
        let node_bound = catalog_bytes + limits.objective.max_condition_node_bytes as u128;
        let configured_bound = limits.objective.max_bytes as u128;
        let admitted = configured_bound.min(source_bound).min(node_bound);
        let condition = Condition::from_catalog_with(
            atoms,
            nodes,
            usize::try_from(admitted).unwrap_or(usize::MAX),
            || counters.work(limits, location),
        )
        .map_err(|error| match error {
            ConditionFailure::Stopped(error) => error,
            ConditionFailure::Condition(ConditionError::Storage(CatalogError::Storage {
                required,
                ..
            })) if source_bound < configured_bound && source_bound <= node_bound => {
                FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    observed: limits.max_support_bytes as u128 - remaining as u128 + required
                        - base,
                    limit: limits.max_support_bytes as u128,
                    location,
                }
            }
            ConditionFailure::Condition(ConditionError::Storage(CatalogError::Storage {
                required,
                ..
            })) if node_bound < configured_bound && node_bound < source_bound => condition_storage(
                required - catalog_bytes,
                limits.objective.max_condition_node_bytes,
                location,
            ),
            ConditionFailure::Condition(error) => {
                FormulaFailure::ObjectiveCondition { error, location }
            }
        })?;
        let retained = condition.node_storage_bytes();
        publication.observe_peak(retained - buffer_bytes, limits, counters, location)?;
        let retained = usize::try_from(retained).map_err(|_| {
            condition_storage(
                retained,
                limits.objective.max_condition_node_bytes,
                location,
            )
        })?;
        // The library retained the same buffer. Its canonical envelope replaces
        // the consumed Query header; only the observed coexistence above is peak.
        lease.observe(retained, location)?;
        publication.retain(lease, limits, counters, location)?;
        Ok(condition)
    }
}

pub(crate) fn condition(
    literals: &[LiteralIr],
    binding: &Binding,
    context: &mut Context<'_, '_, '_>,
) -> Result<PendingCondition, FormulaFailure> {
    let mut atoms = SourceSelection::new(
        context.computation,
        context.limits,
        context.counters,
        context.location,
    )?;
    let mut query = Query::new(context)?;
    let mut root = query.node(ConditionNode::Boolean(true), context)?;
    for literal in literals {
        context.work()?;
        let node = if let LiteralIr::Atom(negation, pattern) = literal {
            let atom = context.atom(*pattern, binding)?;
            let (local, _) = atoms.insert(
                &atom,
                (
                    FormulaResource::ObjectiveFormulaAtoms,
                    context.limits.max_objective_formula_atoms,
                ),
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?;
            let mut node = query.node(ConditionNode::Atom(local), context)?;
            if *negation != DefaultNegation::None {
                node = query.node(ConditionNode::Not(node), context)?;
                if *negation == DefaultNegation::NotNot {
                    node = query.node(ConditionNode::Not(node), context)?;
                }
            }
            node
        } else {
            let value = context.scalar(literal, binding)?;
            query.node(ConditionNode::Boolean(value), context)?
        };
        root = query.node(ConditionNode::And(root, node), context)?;
    }
    Ok(PendingCondition::new(atoms, query))
}

pub(crate) struct Query {
    nodes: Vec<ConditionIndex>,
    lease: StorageLease,
}

impl Query {
    pub(crate) fn new(context: &Context<'_, '_, '_>) -> Result<Self, FormulaFailure> {
        let mut lease = context.computation.lease();
        lease.observe(size_of::<Self>(), context.location)?;
        context.computation.storage_observed(
            &lease,
            0,
            size_of::<Self>(),
            context.limits,
            context.counters,
            context.location,
        )?;
        Ok(Self {
            nodes: Vec::new(),
            lease,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }

    pub(crate) fn node(
        &mut self,
        node: ConditionIndex,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<usize, FormulaFailure> {
        context.work()?;
        if self.nodes.len() >= context.limits.objective.max_condition_nodes {
            return Err(FormulaFailure::Objective {
                error: zetesis_objective::AdmissionError::Limit {
                    resource: zetesis_objective::AdmissionResource::ConditionNodes,
                    template: None,
                    actual: self.nodes.len().saturating_add(1),
                    limit: context.limits.objective.max_condition_nodes,
                },
                location: context.location,
            });
        }
        let limit = context.limits.objective.max_condition_node_bytes;
        let requested = (self.nodes.len() as u128 + 1) * size_of::<ConditionIndex>() as u128;
        if requested > limit as u128 {
            return Err(condition_storage(requested, limit, context.location));
        }
        crate::formula_support::reserve(
            &mut self.nodes,
            1,
            &mut self.lease,
            size_of::<Self>(),
            crate::formula_support::Context::new(
                &*context.computation,
                context.limits,
                context.counters,
                context.location,
            ),
        )?;
        let actual = self.nodes.capacity() as u128 * size_of::<ConditionIndex>() as u128;
        if actual > limit as u128 {
            return Err(condition_storage(actual, limit, context.location));
        }
        context.work()?;
        let index = self.nodes.len();
        self.nodes.push(node);
        Ok(index)
    }
}

fn condition_storage(required: u128, limit: usize, location: ProgramSite) -> FormulaFailure {
    FormulaFailure::ObjectiveCondition {
        error: ConditionError::Storage(CatalogError::Storage { required, limit }),
        location,
    }
}

#[cfg(test)]
mod tests;
