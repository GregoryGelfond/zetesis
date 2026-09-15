//! Finite source-activity refinement starts from complete possible support.
//!
//! Each round reads the old atom table and completely aggregates all producer
//! alternatives into a separately admitted new table. Only Optional entries
//! become Required or Absent; known information is retained. A changing round
//! therefore removes at least one Optional entry. At most N changing rounds and
//! one final no-change round occur for N initially possible source atoms.
//! This computes neither answer sets nor correlated condition satisfiability.

use std::collections::BTreeSet;
use themelios_program::symbol::Signature;

use super::{Activity, Context, SourceEligibility, signature};
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
        context: &mut Context<'_>,
    ) -> Result<(), FormulaFailure> {
        for predicate in support.predicates() {
            context.work()?;
            if unresolved.contains(&signature(predicate)) {
                self.possible(predicate, support, temporary, context)?;
            }
        }
        loop {
            context.work()?;
            let mut derived = Self::default();
            let overlap = temporary.saturating_add(self.atoms.len());
            for predicate in unresolved {
                context.work()?;
                self.derive_predicate(
                    prepared,
                    predicate,
                    support,
                    overlap,
                    &mut derived,
                    context,
                )?;
            }
            if !self.refine_round(&derived, unresolved, context)? {
                return Ok(());
            }
        }
    }

    /// Check the complete round's carrier and information relation before any
    /// new classifications are published. Work failure still aborts the whole
    /// source-activity attempt; no partial certificate escapes its constructor.
    fn refine_round(
        &mut self,
        derived: &Self,
        unresolved: &BTreeSet<Signature>,
        context: &mut Context<'_>,
    ) -> Result<bool, FormulaFailure> {
        for atom in derived.atoms.keys() {
            context.work()?;
            if !self.atoms.contains_key(atom) || !unresolved.contains(&signature(atom.predicate()))
            {
                return Err(FormulaFailure::SourceActivity {
                    location: context.location,
                });
            }
        }
        for (atom, activity) in &self.atoms {
            context.work()?;
            if unresolved.contains(&signature(atom.predicate()))
                && *activity != Activity::Optional
                && derived.atom_activity(atom) != *activity
            {
                return Err(FormulaFailure::SourceActivity {
                    location: context.location,
                });
            }
        }
        let mut changed = false;
        for (atom, activity) in &mut self.atoms {
            context.work()?;
            if unresolved.contains(&signature(atom.predicate())) && *activity == Activity::Optional
            {
                let next = derived.atom_activity(atom);
                if next != Activity::Optional {
                    *activity = next;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests;
