//! Scoped body validation and original-model objective truth share formula lowering.
//!
//! A transient builder borrows complete Support and owns a separate atom table,
//! node table and aggregate cache. Objective rows use their explicit scratch
//! ceilings even when filters or numeric selection later discard them. Rejected
//! rule rows use the theory ceilings for their discarded validation scratch.
//! Both share cumulative source work. Only final reachable objective Condition
//! nodes consume the retained query ceiling. No roots, producers, coherence or
//! support-guard completion from scratch reach the original program.

use super::{Atom, Builder, Node, Purpose, Value};
use crate::formula_ir::LiteralIr;
use crate::formula_objective_dependencies::eligibility::Context;
use crate::formula_objective_dependencies::eligibility::query::Query;
use crate::formula_support::{self, Support};
use crate::{ExpansionResource, FormulaFailure};
use zetesis_objective::{Condition, ConditionNode};

pub(super) struct ValidatedBody {
    atoms: Vec<Atom>,
    nodes: Vec<Node>,
    root: usize,
}

pub(super) fn validate(
    literals: &[LiteralIr],
    binding: &[Value],
    support: &Support<'_>,
    context: &mut Context<'_>,
) -> Result<ValidatedBody, FormulaFailure> {
    validate_with_purpose(literals, binding, support, context, Purpose::Objective)
}

pub(super) fn validate_with_purpose(
    literals: &[LiteralIr],
    binding: &[Value],
    support: &Support<'_>,
    context: &mut Context<'_>,
    purpose: Purpose,
) -> Result<ValidatedBody, FormulaFailure> {
    // Transfer the sole cumulative counter owner and restore it on every error.
    // This never resets generated-value accounting or detaches its observer.
    let counters = std::mem::take(context.counters);
    let mut builder = Builder::empty(context.limits, context.budget, counters, purpose, None);
    let result = (|| {
        builder.node(Node::False, context.location)?;
        builder.node(Node::Implies(0, 0), context.location)?;
        builder.body(literals, binding, context.location, support)
    })();
    *context.counters = builder.counters;
    let root = result?;
    Ok(ValidatedBody {
        atoms: builder.catalog.into_atoms(),
        nodes: builder.nodes,
        root,
    })
}

impl ValidatedBody {
    /// Boolean implication is translated only for a complete original model.
    /// The same rewrite is invalid for frozen Ferraris reducts.
    pub(super) fn condition(self, context: &mut Context<'_>) -> Result<Condition, FormulaFailure> {
        let mut needed = reserved(self.nodes.len(), context)?;
        needed.resize(self.nodes.len(), false);
        needed[self.root] = true;
        // Backward edges make one descending pass sufficient to find ancestors.
        for index in (0..=self.root).rev() {
            context.counters.work(context.limits, context.location)?;
            if needed[index]
                && let Node::And(left, right) | Node::Or(left, right) | Node::Implies(left, right) =
                    self.nodes[index]
            {
                needed[left] = true;
                needed[right] = true;
            }
        }
        let mut mapping = reserved(self.root + 1, context)?;
        mapping.resize(self.root + 1, 0);
        let mut query = Query { nodes: Vec::new() };
        for (index, node) in self.nodes.into_iter().enumerate().take(self.root + 1) {
            context.counters.work(context.limits, context.location)?;
            if !needed[index] {
                continue;
            }
            let node = match node {
                Node::False => ConditionNode::Boolean(false),
                Node::Atom(atom) => ConditionNode::Atom(copy_atom(&self.atoms[atom], context)?),
                Node::And(left, right) => ConditionNode::And(mapping[left], mapping[right]),
                Node::Or(left, right) => ConditionNode::Or(mapping[left], mapping[right]),
                Node::Implies(left, right) => {
                    let negated = query.node(ConditionNode::Not(mapping[left]), context)?;
                    ConditionNode::Or(negated, mapping[right])
                }
            };
            mapping[index] = query.node(node, context)?;
        }
        // The root has the greatest reachable index and is appended last.
        debug_assert_eq!(mapping[self.root] + 1, query.nodes.len());
        Ok(Condition::new(query.nodes))
    }
}

fn copy_atom(atom: &Atom, context: &mut Context<'_>) -> Result<Atom, FormulaFailure> {
    let mut values = reserved(atom.values().len(), context)?;
    for value in atom.values() {
        context.counters.work(context.limits, context.location)?;
        values.push(formula_support::copy(
            value,
            context.budget,
            context.location,
        )?);
    }
    context.budget.charge(
        ExpansionResource::ScalarBytes,
        atom.predicate().name().len() as u128,
        context.location,
    )?;
    Ok(Atom::new(atom.predicate().clone(), values).expect("unchanged typed scratch atom"))
}

fn reserved<T>(count: usize, context: &Context<'_>) -> Result<Vec<T>, FormulaFailure> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::Allocation,
            location: context.location,
        })?;
    Ok(result)
}
