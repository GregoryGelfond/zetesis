//! Checked slot projections use canonical children without subtree copies.

use super::control::Work;
use super::nodes::ceiling;
use super::{AtomId, Failure, Fault, PredicateId, Store, TermId, budget};
use crate::catalog::Limits;
use crate::{ValueNodeRef, ValueResource};

impl Store {
    pub(crate) fn check_assigned_with<E>(
        &self,
        values: &[Option<TermId>],
        slots: &[usize],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        let mut work = Work::new(&mut before);
        for &slot in slots {
            work.step()?;
            let id = values.get(slot).copied().flatten().ok_or(Fault::Shape)?;
            work.step()?;
            self.term_measures(id).check(limits)?;
        }
        Ok(())
    }

    pub(crate) fn construct_assigned_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        values: &[Option<TermId>],
        slots: &[usize],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermId, Failure<E>> {
        let mut work = Work::new(&mut before);
        let mut ids = Vec::new();
        let result = (|| {
            work.step()?;
            ceiling(
                ValueResource::Bytes,
                budget::reservation_bytes(&ids, slots.len())?,
                limits.max_bytes,
            )?;
            work.reserve(&mut ids, slots.len(), &mut self.budget)?;
            for &slot in slots {
                work.step()?;
                let id = values.get(slot).copied().flatten().ok_or(Fault::Shape)?;
                work.step()?;
                ids.push(id);
            }
            self.intern_node_scratch_with(
                descriptor,
                &ids,
                budget::capacity(&ids),
                limits,
                &mut work,
            )
        })();
        self.budget.used -= budget::capacity(&ids);
        result
    }

    pub(crate) fn import_assigned_row_with<E>(
        &mut self,
        predicate: PredicateId,
        values: &[Option<TermId>],
        slots: &[usize],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, Failure<E>> {
        let mut work = Work::new(&mut before);
        let mut ids = Vec::new();
        let result = (|| {
            work.reserve(&mut ids, slots.len(), &mut self.budget)?;
            for &slot in slots {
                work.step()?;
                let id = values.get(slot).copied().flatten().ok_or(Fault::Shape)?;
                work.step()?;
                self.term_measures(id).check(limits)?;
                work.step()?;
                ids.push(id);
            }
            self.intern_atom_with(predicate, &ids, &mut work)
        })();
        self.budget.used -= budget::capacity(&ids);
        result
    }
}
