//! Closed original-model queries are retained only for eligible objective rows.
//! Source producer analysis does not construct these nodes or consume their cap.

use crate::formula_binding::Binding;

use themelios_program::program::DefaultNegation;
use zetesis_objective::{Condition, ConditionNode};

use super::Context;
use crate::FormulaFailure;
use crate::formula_ir::LiteralIr;

pub(crate) fn condition(
    literals: &[LiteralIr],
    binding: &Binding,
    context: &mut Context<'_>,
) -> Result<Condition, FormulaFailure> {
    let mut query = Query { nodes: Vec::new() };
    let mut root = query.node(ConditionNode::Boolean(true), context)?;
    for literal in literals {
        context.work()?;
        let node = if let LiteralIr::Atom(negation, pattern) = literal {
            let atom = context.atom(pattern, binding)?;
            let mut node = query.node(ConditionNode::Atom(atom), context)?;
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
    Ok(Condition::new(query.nodes))
}

pub(crate) struct Query {
    pub(crate) nodes: Vec<ConditionNode>,
}

impl Query {
    pub(crate) fn node(
        &mut self,
        node: ConditionNode,
        context: &mut Context<'_>,
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
        self.nodes
            .try_reserve(1)
            .map_err(|_| context.allocation())?;
        let index = self.nodes.len();
        self.nodes.push(node);
        Ok(index)
    }
}
