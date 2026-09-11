//! Finite ordinary cyclic cones use complete possible support as a certificate.
//! No model search or least-required solver runs here. Optional covers either
//! truth value; only atoms outside completed support are classified absent.
//! Unresolved dependants use the same conservative carrier. Aggregate generators
//! and richer scoped bodies require their own certificate and remain refused.

use std::collections::BTreeSet;
use themelios_program::symbol::Signature;
use zetesis_core::Atom;

use super::{Activity, Completion, Context, refusal, signature};
use crate::formula_ir::{HeadIr, LiteralIr, Prepared};
use crate::formula_support::{self, Support};
use crate::{ExpansionResource, FormulaFailure};

fn ordinary(literals: &[LiteralIr]) -> bool {
    literals.iter().all(|literal| {
        matches!(
            literal,
            LiteralIr::Atom(..)
                | LiteralIr::Compare(..)
                | LiteralIr::Guard(_)
                | LiteralIr::Bind { .. }
                | LiteralIr::Range { .. }
        )
    })
}

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
        for rule in &prepared.rules {
            context.work()?;
            context.location = rule.location;
            match &rule.head {
                HeadIr::Normal(Some(atom)) => {
                    self.possible(atom.predicate(), support, unresolved, temporary, context)?;
                }
                HeadIr::Disjunction(heads) => {
                    for atom in heads.iter().filter_map(|head| head.positive_atom()) {
                        self.possible(atom.predicate(), support, unresolved, temporary, context)?;
                    }
                }
                HeadIr::Choice(group) => {
                    for atom in group
                        .elements
                        .iter()
                        .filter_map(|element| element.head.positive_atom())
                    {
                        self.possible(atom.predicate(), support, unresolved, temporary, context)?;
                    }
                }
                HeadIr::Normal(None) => {}
            }
        }
        Ok(())
    }

    fn possible(
        &mut self,
        predicate: &zetesis_core::Predicate,
        support: &Support<'_>,
        unresolved: &BTreeSet<Signature>,
        temporary: usize,
        context: &mut Context<'_>,
    ) -> Result<(), FormulaFailure> {
        context.work()?;
        if !unresolved.contains(&signature(predicate)) {
            return Ok(());
        }
        for row in support.rows(predicate) {
            context.work()?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(predicate.arity())
                .map_err(|_| context.allocation())?;
            for value in formula_support::row_values(row) {
                context.work()?;
                values.push(formula_support::copy(
                    value,
                    context.budget,
                    context.location,
                )?);
            }
            context.budget.charge(
                ExpansionResource::ScalarBytes,
                predicate.name().len() as u128,
                context.location,
            )?;
            let atom = Atom::new(predicate.clone(), values).expect("completed support arity");
            if !self.atoms.contains_key(&atom) {
                context.entries(temporary.saturating_add(self.atoms.len()).saturating_add(1))?;
                self.atoms.insert(atom, Activity::Optional);
            }
        }
        Ok(())
    }
}
