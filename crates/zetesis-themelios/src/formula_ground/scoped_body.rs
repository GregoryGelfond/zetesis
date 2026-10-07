//! Scoped body validation and original-model objective truth share formula lowering.
//!
//! A transient builder borrows complete Support and selects the shared source
//! authority while owning its node table and aggregate cache. Objective rows use their explicit scratch
//! ceilings even when filters or numeric selection later discard them. Rejected
//! rule rows use the theory ceilings for their discarded validation scratch.
//! Both share cumulative source work. Only final reachable objective Condition
//! nodes consume the retained query ceiling. No roots, producers, coherence or
//! support-guard completion from scratch reach the original program.

use crate::formula_binding::Binding;

use super::{Builder, Node, Purpose, atoms::Catalog};
use crate::FormulaFailure;
use crate::formula_ir::LiteralIr;
use crate::formula_source_activity::model_query::{PendingCondition, Query};
use crate::formula_source_activity::{Activity, Context, SourceEligibility};
use crate::formula_support::{Buffer, Support};
use zetesis_objective::ConditionNode;

pub(super) struct ValidatedBody {
    atoms: Catalog,
    nodes: zetesis_ferraris::FormulaParts,
    root: usize,
}

pub(super) fn validate(
    literals: &[LiteralIr],
    binding: &Binding,
    support: &Support<'_>,
    context: &mut Context<'_, '_, '_>,
) -> Result<ValidatedBody, FormulaFailure> {
    validate_with_purpose(literals, binding, support, context, Purpose::Objective)
}

/// Source activity uses the same complete scoped lowering as original-model
/// queries, with theory scratch ceilings and no retained query publication.
pub(crate) fn source_activity(
    literals: &[LiteralIr],
    binding: &Binding,
    support: &Support<'_>,
    eligibility: &SourceEligibility,
    context: &mut Context<'_, '_, '_>,
) -> Result<Activity, FormulaFailure> {
    validate_with_purpose(literals, binding, support, context, Purpose::Validation)?
        .source_activity(eligibility, context)
}

pub(super) fn validate_with_purpose(
    literals: &[LiteralIr],
    binding: &Binding,
    support: &Support<'_>,
    context: &mut Context<'_, '_, '_>,
    purpose: Purpose,
) -> Result<ValidatedBody, FormulaFailure> {
    // Empty validates its catalog before transferring the cumulative counters;
    // every later Result path returns those counters to the caller unchanged.
    let mut builder = Builder::empty(
        context.computation,
        context.limits,
        context.budget,
        context.counters,
        purpose,
        None,
        context.location,
    )?;
    let result = (|| {
        builder.initialize(context.location)?;
        builder.body(literals, binding, context.location, support)
    })();
    *context.counters = builder.counters;
    let root = result?;
    Ok(ValidatedBody {
        atoms: builder.catalog,
        nodes: builder.nodes.into_parts(),
        root,
    })
}

impl ValidatedBody {
    /// Fold independent source-atom possibilities over the lowered DAG. An
    /// optional atom and its negation remain optional even when correlated;
    /// this is neither satisfiability nor a frozen-reduct evaluation.
    pub(super) fn source_activity(
        &self,
        eligibility: &SourceEligibility,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        let count = self.root + 1;
        let mut values = Buffer::new(
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        values.resize(
            count,
            Activity::Absent,
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        for index in 0..count {
            let node = self.nodes.view().node(index).expect("owned formula node");
            context.counters.work(context.limits, context.location)?;
            let activity = match node {
                Node::False => Activity::Absent,
                Node::Atom(atom) => {
                    let source = self.atoms.source(
                        atom,
                        context.counters,
                        context.limits,
                        context.location,
                    )?;
                    eligibility.source_activity(&source, context)?
                }
                Node::And(row) | Node::Or(row) => {
                    let conjunction = matches!(node, Node::And(_));
                    let mut activity = if conjunction {
                        Activity::Required
                    } else {
                        Activity::Absent
                    };
                    for &child in row {
                        context.counters.work(context.limits, context.location)?;
                        activity = if conjunction {
                            activity.min(values.slice()[child])
                        } else {
                            activity.max(values.slice()[child])
                        };
                    }
                    activity
                }
                Node::Implies(left, right) => {
                    context
                        .counters
                        .charge_work(2, context.limits, context.location)?;
                    values.slice()[left].negate().max(values.slice()[right])
                }
            };
            values.slice_mut()[index] = activity;
        }
        Ok(values.slice()[self.root])
    }

    /// A canonical false body cannot contribute a source projection instance.
    pub(super) fn is_false(&self) -> bool {
        self.root == super::FALSUM
    }
    /// Boolean implication is translated only for a complete original model.
    /// The same rewrite is invalid for frozen Ferraris reducts.
    pub(super) fn condition(
        self,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<PendingCondition, FormulaFailure> {
        let mut needed = Buffer::new(
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        needed.resize(
            self.nodes.view().len(),
            false,
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        context.counters.work(context.limits, context.location)?;
        needed.slice_mut()[self.root] = true;
        // Backward edges make one descending pass sufficient to find ancestors.
        for index in (0..=self.root).rev() {
            context.counters.work(context.limits, context.location)?;
            if needed.slice()[index] {
                let pair;
                let row = match self.nodes.view().node(index).expect("owned formula node") {
                    Node::And(row) | Node::Or(row) => row,
                    Node::Implies(a, b) => {
                        pair = [a, b];
                        &pair
                    }
                    Node::Atom(_) | Node::False => &[],
                };
                for &child in row {
                    context.counters.work(context.limits, context.location)?;
                    needed.slice_mut()[child] = true;
                }
            }
        }
        let mut mapping = Buffer::new(
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        mapping.resize(
            self.root + 1,
            0,
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        let mut query = Query::new(context)?;
        for index in 0..=self.root {
            let node = self.nodes.view().node(index).expect("owned formula node");
            context.counters.work(context.limits, context.location)?;
            if !needed.slice()[index] {
                continue;
            }
            let node = match node {
                Node::False => ConditionNode::Boolean(false),
                Node::Atom(atom) => ConditionNode::Atom(atom),
                Node::And(row) | Node::Or(row) => {
                    let conjunction = matches!(node, Node::And(_));
                    let mut previous = None;
                    for &child in row {
                        context.counters.work(context.limits, context.location)?;
                        let next = mapping.slice()[child];
                        previous = Some(match previous {
                            None => next,
                            Some(previous) => query.node(
                                if conjunction {
                                    ConditionNode::And(previous, next)
                                } else {
                                    ConditionNode::Or(previous, next)
                                },
                                context,
                            )?,
                        });
                    }
                    mapping.slice_mut()[index] =
                        previous.expect("stored groups have at least two children");
                    continue;
                }
                Node::Implies(left, right) => {
                    let negated = query.node(ConditionNode::Not(mapping.slice()[left]), context)?;
                    ConditionNode::Or(negated, mapping.slice()[right])
                }
            };
            mapping.slice_mut()[index] = query.node(node, context)?;
        }
        // The root has the greatest reachable index and is appended last.
        debug_assert_eq!(mapping.slice()[self.root] + 1, query.len());
        Ok(PendingCondition::new(self.atoms.into_selection(), query))
    }
}

#[cfg(test)]
mod tests;
