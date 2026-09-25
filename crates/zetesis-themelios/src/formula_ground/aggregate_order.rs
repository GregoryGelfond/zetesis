//! Typed contribution order over canonical keys, separate from discovery IDs.
//!
//! Coalescing uses local identity coordinates. Its terminal entries are sorted
//! in place using borrowed typed keys, with no additional payload or permutation.

use std::cmp::Ordering;

use super::{
    GroundKey, Measure,
    cache::{CoordinateMap, Entries},
};
use crate::FormulaFailure;
use crate::formula_support::{
    Computation, Context, GroundingWork, SourceSelection, TermTable, sort,
};

/// Consume the coalescing index while retaining its vector and storage lease.
/// Work permits precede navigation and moves. Refusal drops private unfinished
/// entries; no ordered contributions are published. Comparison costs include
/// canonical navigation and text, beyond the O(k log k) sorting comparisons.
pub(super) fn ordered(
    grouped: CoordinateMap<GroundKey, (Measure, usize)>,
    terms: &TermTable,
    atoms: &SourceSelection,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<Entries<GroundKey, (Measure, usize)>, FormulaFailure> {
    let Context { computation, work } = context;
    let keys = Keys {
        terms,
        atoms,
        computation,
    };
    let mut entries = grouped.into_entries();
    sort::by(entries.slice_mut(), work, |left, right, work| {
        keys.compare(left.0, right.0, work)
    })?;
    Ok(entries)
}

struct Keys<'a, 'terms, 'source> {
    terms: &'a TermTable,
    atoms: &'a SourceSelection,
    computation: &'a Computation<'terms, 'source>,
}

impl Keys<'_, '_, '_> {
    fn compare(
        &self,
        left: GroundKey,
        right: GroundKey,
        work: &mut GroundingWork<'_>,
    ) -> Result<Ordering, FormulaFailure> {
        match (left, right) {
            (GroundKey::Tuple(left), GroundKey::Tuple(right)) => self.tuples(left, right, work),
            (GroundKey::Atom(left), GroundKey::Atom(right)) => {
                let left = self.atoms.atom(
                    left,
                    self.computation,
                    work.limits,
                    work.counters,
                    work.location,
                )?;
                let right = self.atoms.atom(
                    right,
                    self.computation,
                    work.limits,
                    work.counters,
                    work.location,
                )?;
                left.compare_ref_with(right, || work.counters.work(work.limits, work.location))
            }
            (GroundKey::Tuple(_), GroundKey::Atom(_)) => Ok(Ordering::Less),
            (GroundKey::Atom(_), GroundKey::Tuple(_)) => Ok(Ordering::Greater),
        }
    }

    /// Aggregate tuple keys compare components lexicographically, then length,
    /// rather than comparing the tuple root's arity descriptor first.
    fn tuples(
        &self,
        left: usize,
        right: usize,
        work: &mut GroundingWork<'_>,
    ) -> Result<Ordering, FormulaFailure> {
        work.counters.work(work.limits, work.location)?;
        let read = self.computation.read();
        let left = self.terms.value(left, read, work.location)?;
        let right = self.terms.value(right, read, work.location)?;
        let mut index = 0;
        loop {
            work.counters.work(work.limits, work.location)?;
            match (left.child(index), right.child(index)) {
                (None, None) => return Ok(Ordering::Equal),
                (None, Some(_)) => return Ok(Ordering::Less),
                (Some(_), None) => return Ok(Ordering::Greater),
                (Some(left), Some(right)) => {
                    let order = left.compare_ref_with(right, || {
                        work.counters.work(work.limits, work.location)
                    })?;
                    if order != Ordering::Equal {
                        return Ok(order);
                    }
                }
            }
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests;
