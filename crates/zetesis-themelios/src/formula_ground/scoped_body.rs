//! Scoped body validation and original-model objective truth share formula lowering.
//!
//! Scoped builders borrow complete Support and select the shared source
//! authority while owning their node tables and aggregate caches. One source
//! producer pass retains its builder until all bindings have been observed under
//! the same activity snapshot; independent validation keeps transient builders.
//! Objective rows use their explicit scratch ceilings even when filters or
//! numeric selection later discard them. Rejected
//! rule rows with nested scopes or generators use the theory ceilings for
//! their discarded validation scratch; flat rejected rows validate atom keys.
//! Both share cumulative source work. Only final reachable objective Condition
//! nodes consume the retained query ceiling. No roots, producers, coherence or
//! support-guard completion from scratch reach the original program.

mod threshold;
mod activity;
mod source;

pub(crate) use source::SourceBodies;

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

pub(super) fn validate<'source>(
    literals: &[LiteralIr],
    binding: &Binding,
    support: &Support<'source>,
    context: &mut Context<'_, '_, 'source>,
) -> Result<ValidatedBody, FormulaFailure> {
    validate_with_purpose(literals, binding, support, context, Purpose::Objective)
}

pub(super) fn validate_with_purpose<'source>(
    literals: &[LiteralIr],
    binding: &Binding,
    support: &Support<'source>,
    context: &mut Context<'_, '_, 'source>,
    purpose: Purpose,
) -> Result<ValidatedBody, FormulaFailure> {
    // The builder borrows the cumulative account through every outcome,
    // including an unwind during scoped lowering.
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
    let root = result?;
    Ok(ValidatedBody {
        atoms: builder.catalog,
        nodes: builder.nodes.into_parts(),
        root,
    })
}

impl ValidatedBody {
    /// Independent callers evaluate their owned body against one fresh source
    /// snapshot. Producer passes use `SourceBodies` to retain this same fold.
    pub(super) fn source_activity(
        &self,
        eligibility: &SourceEligibility,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        activity::Evaluation::new(context)?.evaluate(
            &self.nodes,
            &self.atoms,
            self.root,
            eligibility,
            context,
        )
    }

    /// A canonical false body cannot contribute a source projection instance.
    pub(super) fn is_false(&self) -> bool {
        self.root == super::FALSUM
    }
    /// Boolean implication is translated only for a complete original model.
    /// The same rewrite is invalid for frozen Ferraris reducts.
    /// Select the complete backward-closed formula slice before translating it.
    /// Physical numeric children remain selected even when a guard is constant.
    fn ancestor_mask(
        &self,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Buffer<bool>, FormulaFailure> {
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
                    Node::Threshold(row) => row.conditions,
                    Node::ExtremumCut(cut) => cut.row.conditions,
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
        Ok(needed)
    }

    pub(super) fn condition(
        self,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<PendingCondition, FormulaFailure> {
        let needed = self.ancestor_mask(context)?;
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
                Node::Threshold(row) => {
                    mapping.slice_mut()[index] =
                        threshold::lower(row, mapping.slice(), &mut query, context)?;
                    continue;
                }
                Node::ExtremumCut(cut) => {
                    mapping.slice_mut()[index] =
                        threshold::lower_extremum(cut, mapping.slice(), &mut query, context)?;
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
