//! Closed queries over the caller's original model.
//!
//! Unlike a Ferraris theory, a condition has no atom universe, rule roots or
//! support semantics. Its typed atoms refer directly to the supplied model.
//! Backward references permit bounded iterative evaluation without recursive
//! syntax or a hidden solver. Source grounding owns eligibility and must not
//! infer objective priority presence from this model-relative query.

use zetesis_core::Atom;

use crate::{AdmissionError, AdmissionLimits, AdmissionResource};

/// One operation in an acyclic query over a complete model.
/// Every operand index must precede the operation that references it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionNode {
    /// Constant logical truth.
    Boolean(bool),
    /// Membership of a complete typed atom, including its strong-negation sign.
    Atom(Atom),
    /// Logical negation of an earlier result.
    Not(usize),
    /// Conjunction of earlier results.
    And(usize, usize),
    /// Disjunction of earlier results.
    Or(usize, usize),
}

/// A closed model query whose final node is its result.
///
/// [`crate::ObjectiveProgram::new`] checks all backward references and the
/// configured node and predicate-arity ceilings. Evaluation visits every node
/// once and charges atom comparisons to the ordinary objective work budget;
/// temporary Boolean storage is linear in the node count. An empty query is
/// true. Conditions never bind variables or create atoms.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Condition {
    nodes: Vec<ConditionNode>,
}

impl Condition {
    /// Assemble an owned query for subsequent objective-program admission.
    #[must_use]
    pub fn new(nodes: Vec<ConditionNode>) -> Self {
        Self { nodes }
    }

    /// Operations in their original evaluation order.
    #[must_use]
    pub fn nodes(&self) -> &[ConditionNode] {
        &self.nodes
    }

    pub(crate) fn admit(
        &self,
        limits: AdmissionLimits,
        template: usize,
    ) -> Result<(), AdmissionError> {
        super::program::check_bound(
            AdmissionResource::ConditionNodes,
            self.nodes.len(),
            limits.max_condition_nodes,
            Some(template),
        )?;
        for (index, node) in self.nodes.iter().enumerate() {
            match node {
                ConditionNode::Boolean(_) => {}
                ConditionNode::Atom(atom) => super::program::check_bound(
                    AdmissionResource::PredicateArity,
                    atom.values().len(),
                    limits.max_predicate_arity,
                    Some(template),
                )?,
                ConditionNode::Not(operand) => reference(*operand, index, template)?,
                ConditionNode::And(left, right) | ConditionNode::Or(left, right) => {
                    reference(*left, index, template)?;
                    reference(*right, index, template)?;
                }
            }
        }
        Ok(())
    }
}

fn reference(operand: usize, node: usize, template: usize) -> Result<(), AdmissionError> {
    if operand < node {
        Ok(())
    } else {
        Err(AdmissionError::ConditionReference {
            template,
            node,
            operand,
        })
    }
}
