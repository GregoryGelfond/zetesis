//! Domain positions by term identity, for a caller that reads them during
//! one use of a program.

use std::num::NonZeroU32;

use super::Program;
use crate::catalog::{CatalogRead, TermRef};

/// Domain positions by term identity for one program, reserved by the caller
/// under its own accounting and read only during that caller's use.
///
/// One slot per term of the program's catalog: a domain value's slot holds
/// its domain position plus one, and every other term's slot is `None`. A
/// lookup resolves its value through the program's own read, so a value this
/// program's catalog cannot name has no position here, and the caller's
/// content search, which stays the authority, answers for it.
#[derive(Debug)]
pub struct DomainPositions<'p> {
    program: &'p Program,
    slots: Vec<Option<NonZeroU32>>,
}

impl<'p> DomainPositions<'p> {
    /// Build the table for `program`. `reserve` is given the number of slots,
    /// the program catalog's term count, and returns an empty vector with at
    /// least that capacity, or the caller's own refusal. The build is one pass
    /// over the catalog's terms and one write per domain value.
    ///
    /// # Errors
    /// Only the refusal `reserve` returns. `Ok(None)`: the domain holds
    /// `u32::MAX` values or more, too many for a slot to name, and no table
    /// is built.
    ///
    /// # Panics
    /// When `reserve` returns a vector that is not empty or whose capacity is
    /// below the slots it was asked for.
    pub fn build<E>(
        program: &'p Program,
        reserve: impl FnOnce(usize) -> Result<Vec<Option<NonZeroU32>>, E>,
    ) -> Result<Option<Self>, E> {
        let domain = &program.0.domain;
        if !fits(domain.len()) {
            return Ok(None);
        }
        let terms = program.0.catalog.read().term_count();
        let mut slots = reserve(terms)?;
        assert!(
            slots.is_empty() && slots.capacity() >= terms,
            "the caller reserves the table's slots"
        );
        slots.resize(terms, None);
        for (position, id) in domain.iter().enumerate() {
            let slot = u32::try_from(position + 1).expect("the domain is below u32::MAX values");
            slots[id.position()] = NonZeroU32::new(slot);
        }
        Ok(Some(Self { program, slots }))
    }

    /// The domain position of `value`, or `None` when it is not a domain value
    /// of this program or this program's read cannot name it.
    #[must_use]
    pub fn position(&self, value: TermRef<'_>) -> Option<usize> {
        let id = CatalogRead::of(self.program.0.catalog.read())
            .selected_term(value)
            .ok()?;
        let slot = (*self.slots.get(id.position())?)?;
        Some(slot.get() as usize - 1)
    }
}

/// Whether a domain of `values` values gets a table: every position plus one
/// must fit a slot, which holds at most `u32::MAX`.
fn fits(values: usize) -> bool {
    values < u32::MAX as usize
}

#[cfg(test)]
mod tests;
