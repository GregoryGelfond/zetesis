//! Source identity is shared; refinement owns only a sparse selection and cells.
//! A discovered atom outside the completed activity selection remains absent.

use crate::formula_support::components::Pattern as AtomPattern;
use zetesis_core::catalog::AtomRef;

use super::{Activity, Context, SourceEligibility};
use crate::formula_binding::Binding;
use crate::formula_support::{SourceAtom, SourceSelection, StorageLease};
use crate::{FormulaFailure, FormulaResource};

pub(super) struct Round {
    pub(super) updates: Vec<Option<Activity>>,
    present: usize,
    lease: StorageLease,
}

impl Round {
    pub(super) fn new(context: &Context<'_, '_, '_>) -> Self {
        // This inline header already belongs to SourceEligibility's Option.
        Self {
            updates: Vec::new(),
            present: 0,
            lease: context.computation.lease(),
        }
    }

    pub(super) fn at(&self, id: usize) -> Activity {
        self.updates
            .get(id)
            .copied()
            .flatten()
            .unwrap_or(Activity::Absent)
    }

    fn reserve(
        &mut self,
        required: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let additional = required.saturating_sub(self.updates.len());
        crate::formula_support::reserve(
            &mut self.updates,
            additional,
            &mut self.lease,
            0,
            crate::formula_support::Context::new(
                &*context.computation,
                context.limits,
                context.counters,
                context.location,
            ),
        )?;
        context
            .counters
            .charge_work(additional as u128, context.limits, context.location)?;
        self.updates.resize(required.max(self.updates.len()), None);
        Ok(())
    }
}

impl SourceEligibility {
    pub(crate) fn new(context: &Context<'_, '_, '_>) -> Result<Self, FormulaFailure> {
        let atoms = SourceSelection::new(
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        let mut lease = context.computation.lease();
        let header = Self::header_bytes();
        lease.observe(header, context.location)?;
        context.computation.storage_observed(
            &lease,
            0,
            header,
            context.limits,
            context.counters,
            context.location,
        )?;
        Ok(Self {
            atoms,
            activity: Vec::new(),
            round: None,
            lease,
        })
    }

    fn header_bytes() -> usize {
        size_of::<Self>() - size_of::<SourceSelection>()
    }

    pub(super) fn retain_atom(
        &mut self,
        atom: AtomRef<'_>,
        activity: Activity,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let atom = context.computation.atom_ref(
            atom,
            context.limits,
            context.counters,
            context.location,
        )?;
        self.stage(&atom, activity, temporary, context)
    }

    pub(super) fn stage(
        &mut self,
        atom: &SourceAtom,
        activity: Activity,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let found =
            self.atoms
                .position(atom, context.limits, context.counters, context.location)?;
        let id = found.unwrap_or_else(|| self.atoms.len());
        let round = self.round.as_ref().expect("producer round active");
        let present = round.updates.get(id).copied().flatten().is_some();
        if !present {
            context.entries(temporary.saturating_add(round.present).saturating_add(1))?;
        }
        self.round
            .as_mut()
            .expect("producer round active")
            .reserve(id.saturating_add(1), context)?;
        let (inserted, _) = self.atoms.insert(
            atom,
            (
                FormulaResource::ObjectivePresenceEntries,
                context.limits.max_objective_presence_entries,
            ),
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        debug_assert_eq!(inserted, id, "exclusive activity selection append");
        context.work()?;
        let round = self.round.as_mut().expect("producer round active");
        round.updates[id] =
            Some(round.updates[id].map_or(activity, |previous| previous.max(activity)));
        if !present {
            round.present += 1;
        }
        Ok(())
    }

    pub(super) fn publish_predicate(
        &mut self,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let additional = self.atoms.len().saturating_sub(self.activity.len());
        crate::formula_support::reserve(
            &mut self.activity,
            additional,
            &mut self.lease,
            Self::header_bytes(),
            crate::formula_support::Context::new(
                &*context.computation,
                context.limits,
                context.counters,
                context.location,
            ),
        )?;
        context
            .counters
            .charge_work(additional as u128, context.limits, context.location)?;
        self.activity.resize(self.atoms.len(), Activity::Absent);
        let round = self.round.as_ref().expect("producer round active");
        for (id, update) in round.updates.iter().enumerate() {
            context.work()?;
            if let Some(activity) = update {
                self.activity[id] = self.activity[id].max(*activity);
            }
        }
        Ok(())
    }

    pub(super) fn pattern_activity(
        &self,
        pattern: AtomPattern,
        binding: &Binding<'_>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        let atom = context.atom(pattern, binding)?;
        self.lookup(&atom, context)
    }

    pub(crate) fn source_activity(
        &self,
        atom: &SourceAtom,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        self.lookup(atom, context)
    }

    fn lookup(
        &self,
        atom: &SourceAtom,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        let found =
            self.atoms
                .position(atom, context.limits, context.counters, context.location)?;
        context.work()?;
        Ok(found
            .and_then(|id| self.activity.get(id))
            .copied()
            .unwrap_or(Activity::Absent))
    }
}

#[cfg(test)]
mod tests;
