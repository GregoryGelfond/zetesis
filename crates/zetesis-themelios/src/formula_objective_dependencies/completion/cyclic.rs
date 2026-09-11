//! Finite ordinary cyclic cones use complete possible support as a certificate.
//! No model search or least-required solver runs here. Optional covers either
//! truth value; only atoms outside completed support are classified absent.
//! Unresolved dependants use the same conservative carrier. Aggregate generators
//! and richer scoped bodies require their own certificate and remain refused.

use std::collections::BTreeSet;
use themelios_program::symbol::Signature;

use super::{Completion, Context, ordinary, refusal, signature};
use crate::FormulaFailure;
use crate::formula_ir::{HeadIr, Prepared};
use crate::formula_support::Support;

impl Completion {
    pub(super) fn cyclic(
        &mut self,
        prepared: &Prepared,
        support: &Support<'_>,
        unresolved: &BTreeSet<Signature>,
        temporary: usize,
        context: &mut Context<'_>,
    ) -> Result<(), FormulaFailure> {
        for rule in &prepared.rules {
            context.work()?;
            if !crate::formula_objective_dependencies::relevant_head(&rule.head, unresolved) {
                continue;
            }
            if !ordinary(&rule.body) {
                return Err(refusal(rule.location));
            }
            match &rule.head {
                HeadIr::Disjunction(heads)
                    if heads.iter().any(|head| head.positive_atom().is_none()) =>
                {
                    return Err(refusal(rule.location));
                }
                HeadIr::Choice(group)
                    if group
                        .elements
                        .iter()
                        .any(|element| !ordinary(&element.condition)) =>
                {
                    return Err(refusal(rule.location));
                }
                _ => {}
            }
        }
        for predicate in support.predicates() {
            context.work()?;
            if unresolved.contains(&signature(predicate)) {
                self.possible(predicate, support, temporary, context)?;
            }
        }
        Ok(())
    }
}
