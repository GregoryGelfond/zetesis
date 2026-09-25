//! Finite source-activity refinement starts from complete possible support.
//!
//! Each round reads the old atom table and completely aggregates all producer
//! alternatives into separately admitted ID-only classification cells. Only Optional entries
//! become Required or Absent; known information is retained. A changing round
//! therefore removes at least one Optional entry. At most N changing rounds and
//! one final no-change round occur for N initially possible source atoms.
//! This computes neither answer sets nor correlated condition satisfiability.

use std::collections::BTreeSet;
use themelios_program::symbol::Signature;

use super::{Activity, Context, SourceEligibility, authority::Round, signature};
use crate::FormulaFailure;
use crate::formula_ir::Prepared;
use crate::formula_support::Support;

impl SourceEligibility {
    pub(super) fn cyclic(
        &mut self,
        prepared: &Prepared,
        support: &Support<'_>,
        unresolved: &BTreeSet<Signature>,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        for predicate in support.predicates() {
            context.work()?;
            context.counters.charge_work(
                predicate.name().len() as u128,
                context.limits,
                context.location,
            )?;
            if unresolved.contains(&signature(predicate)) {
                self.possible(predicate, support, temporary, context)?;
            }
        }
        loop {
            context.work()?;
            self.round = Some(Round::new(context));
            let result = self.derive_round(prepared, support, unresolved, temporary, context);
            self.round = None;
            if !result? {
                return Ok(());
            }
        }
    }

    fn derive_round(
        &mut self,
        prepared: &Prepared,
        support: &Support<'_>,
        unresolved: &BTreeSet<Signature>,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<bool, FormulaFailure> {
        let overlap = temporary.saturating_add(self.activity.len());
        for predicate in unresolved {
            context.work()?;
            self.derive_predicate(prepared, predicate, support, overlap, context)?;
        }
        self.refine_round(unresolved, context)
    }

    /// Validate the complete carrier and information relation before publishing
    /// classifications. Work refusal aborts the certificate's constructor.
    fn refine_round(
        &mut self,
        unresolved: &BTreeSet<Signature>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<bool, FormulaFailure> {
        let round = self.round.as_ref().expect("producer round active");
        for (id, update) in round.updates.iter().enumerate() {
            context.work()?;
            if update.is_some()
                && (id >= self.activity.len() || !self.unresolved(id, unresolved, context)?)
            {
                return Err(FormulaFailure::SourceActivity {
                    location: context.location,
                });
            }
        }
        for (id, activity) in self.activity.iter().enumerate() {
            context.work()?;
            if *activity != Activity::Optional
                && self.unresolved(id, unresolved, context)?
                && round.at(id) != *activity
            {
                return Err(FormulaFailure::SourceActivity {
                    location: context.location,
                });
            }
        }
        let mut changed = false;
        for id in 0..self.activity.len() {
            context.work()?;
            if self.activity[id] == Activity::Optional
                && self.unresolved(id, unresolved, context)?
            {
                let next = round.at(id);
                if next != Activity::Optional {
                    self.activity[id] = next;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }

    fn unresolved(
        &self,
        id: usize,
        unresolved: &BTreeSet<Signature>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<bool, FormulaFailure> {
        context.work()?;
        let atom = self.atoms.atom(
            id,
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        let predicate = atom.predicate();
        context.counters.charge_work(
            predicate.name().len() as u128,
            context.limits,
            context.location,
        )?;
        Ok(unresolved.contains(&signature(predicate)))
    }
}

#[cfg(test)]
mod tests;
