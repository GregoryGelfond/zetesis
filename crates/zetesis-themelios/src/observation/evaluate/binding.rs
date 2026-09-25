//! Query slots separate borrowed input captures from derived IDs. A generated
//! slot owns its logical local charge only in the scope that introduced it.
//! Inactive ID cells retain capacity, never another copy of term payload.

use zetesis_core::catalog::{AssignmentFailure, TermAssignment, TermKey, TermRead, TermRef};

use super::{Error, ErrorKind, Metric, Work};

#[derive(Clone, Copy)]
enum Slot<'input> {
    Input(TermRef<'input>),
    Term(Option<Metric>),
    Pattern {
        key: super::anonymous::Key,
        charge: Option<Metric>,
    },
}

/// The query owns one generated-ID witness; source captures borrow fixed inputs.
pub(super) struct Binding<'input> {
    slots: Vec<Option<Slot<'input>>>,
    terms: TermAssignment,
}

impl<'input> Binding<'input> {
    pub fn new(
        variables: usize,
        outer: Option<&Self>,
        terms: TermRead<'_>,
        max_frame_bytes: usize,
        work: &mut Work<'_>,
    ) -> Result<Self, Error> {
        let mut slots = work.reserve(variables)?;
        slots.resize(variables, None);
        let mut assignment = terms.assignment();
        if let Some(outer) = outer {
            let inherited = variables.min(outer.slots.len());
            let prefix = outer
                .terms
                .prefix(inherited)
                .map_err(|error| work.error(ErrorKind::TermAssignment(error)))?;
            assignment
                .copy_from_with(prefix, max_frame_bytes, || work.step(1))
                .map_err(|error| failure(error, work))?;
            for (target, source) in slots.iter_mut().zip(&outer.slots) {
                work.step(1)?;
                *target = source.map(|slot| match slot {
                    Slot::Input(value) => Slot::Input(value),
                    Slot::Term(_) => Slot::Term(None),
                    Slot::Pattern { key, .. } => Slot::Pattern { key, charge: None },
                });
            }
        }
        assignment
            .resize_with(variables, max_frame_bytes, || work.step(1))
            .map_err(|error| failure(error, work))?;
        Ok(Self {
            slots,
            terms: assignment,
        })
    }

    pub fn input(&self, slot: usize) -> Option<TermRef<'input>> {
        match self.slots[slot] {
            Some(Slot::Input(value)) => Some(value),
            _ => None,
        }
    }
    pub fn key(&self, slot: usize) -> Result<TermKey, zetesis_core::catalog::AssignmentError> {
        self.terms
            .key(slot)?
            .ok_or(zetesis_core::catalog::AssignmentError::Unbound { slot })
    }

    pub fn is_bound(&self, slot: usize) -> bool {
        self.slots[slot].is_some()
    }

    pub fn value<'read>(
        &'read self,
        slot: usize,
        terms: TermRead<'read>,
        work: &mut Work<'_>,
    ) -> Result<Option<TermRef<'read>>, Error>
    where
        'input: 'read,
    {
        work.step(1)?;
        match self.slots[slot] {
            None | Some(Slot::Pattern { .. }) => Ok(None),
            Some(Slot::Input(value)) => Ok(Some(value)),
            Some(Slot::Term(_)) => self
                .terms
                .as_slice()
                .term_with(terms, slot, || work.step(1))
                .map_err(|error| failure(error, work)),
        }
    }

    pub fn pattern(&self, slot: usize) -> Option<super::anonymous::Key> {
        match self.slots[slot] {
            Some(Slot::Pattern { key, .. }) => Some(key),
            _ => None,
        }
    }

    pub fn capture(&mut self, slot: usize, value: TermRef<'input>) {
        assert!(
            self.slots[slot].is_none(),
            "a capture introduces an absent slot"
        );
        self.slots[slot] = Some(Slot::Input(value));
    }

    /// The caller has admitted the logical charge. Work refusal precedes both
    /// the active slot and its ownership mark, so failed installation owns none.
    pub fn bind_term(
        &mut self,
        slot: usize,
        key: &TermKey,
        metric: Metric,
        work: &mut Work<'_>,
    ) -> Result<(), Error> {
        assert!(
            self.slots[slot].is_none(),
            "a generated value introduces an absent slot"
        );
        self.terms
            .set_with(slot, key, || work.step(1))
            .map_err(|error| failure(error, work))?;
        self.slots[slot] = Some(Slot::Term(Some(metric)));
        work.local_bytes += metric.payload();
        Ok(())
    }

    pub fn bind_pattern(
        &mut self,
        slot: usize,
        key: super::anonymous::Key,
        metric: Metric,
        work: &mut Work<'_>,
    ) {
        assert!(
            self.slots[slot].is_none(),
            "an atom pattern introduces an absent slot"
        );
        self.slots[slot] = Some(Slot::Pattern {
            key,
            charge: Some(metric),
        });
        work.local_bytes += metric.payload();
    }

    /// Cleanup cannot stop. The inactive ID cell is ignored until overwritten;
    /// its canonical term remains in the interpreter's append-only arena.
    pub fn clear(&mut self, slot: usize, work: &mut Work<'_>) {
        if let Some(value) = self.slots[slot].take() {
            work.local_bytes -= charge(value);
        }
    }

    /// Release only this scope's logical ownership, including on a visitor stop.
    pub fn release(&mut self, work: &mut Work<'_>) {
        for slot in &mut self.slots {
            if let Some(value) = slot.take() {
                work.local_bytes -= charge(value);
            }
        }
    }
}

fn charge(slot: Slot<'_>) -> u128 {
    match slot {
        Slot::Input(_) | Slot::Term(None) | Slot::Pattern { charge: None, .. } => 0,
        Slot::Term(Some(metric))
        | Slot::Pattern {
            charge: Some(metric),
            ..
        } => metric.payload(),
    }
}

pub(super) fn failure(error: AssignmentFailure<Error>, work: &Work<'_>) -> Error {
    match error {
        AssignmentFailure::Assignment(error) => work.error(ErrorKind::TermAssignment(error)),
        AssignmentFailure::Stopped(error) => error,
    }
}
