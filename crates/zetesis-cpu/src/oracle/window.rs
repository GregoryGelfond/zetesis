//! Ordered relation windows for the leading terms whose values are known.
//!
//! A full match agrees with every bound leading term. In a relation sorted by
//! tuple storage order, all such rows form a contiguous interval. Two monotone
//! boundary searches recover that interval without cloning values or building an
//! index. Remaining columns still need the ordinary transactional matcher.

use std::{cmp::Ordering, ops::Range};

use zetesis_core::{AtomPattern, Value};

use super::relations::{Row, Rows};
use super::{Work, resolve};
use crate::Stop;

/// Requires rows of the pattern's predicate and arity in ascending Atom/Value
/// storage order, and an assignment covering the admitted pattern's variable
/// slots. Borrows all values; allocates nothing. For n rows and k bound leading
/// terms, performs O(k * log(n + 1)) value comparisons and at most k + 1 readiness
/// inspections. Comparison costs include payload bytes. The caller must reopen
/// after parent bindings change, and must still check the full pattern and guards
/// for each returned row.
pub(super) fn matching_prefix(
    pattern: &AtomPattern,
    rows: Rows<'_>,
    run: usize,
    assignment: &[Option<&Value>],
    work: &mut Work<'_>,
) -> Result<Range<usize>, Stop> {
    let length = rows.run_len(run);
    if length == 0 {
        return Ok(0..0);
    }
    let mut bound = 0;
    for term in pattern.terms() {
        work.tick()?;
        if resolve(term, assignment).is_none() {
            break;
        }
        bound += 1;
    }
    if bound == 0 {
        return Ok(0..length);
    }
    if let Rows::Dense { relation, .. } = rows {
        // The bound prefix names one block of positions. Its terms resolved
        // in the count above, so the values are read where they lie.
        work.charge(bound)?;
        let prefix = pattern.terms()[..bound]
            .iter()
            .map_while(|term| resolve(term, assignment));
        return Ok(relation.layout().prefix_range(prefix));
    }
    let compare = |index, work: &mut Work<'_>| {
        compare_prefix(
            pattern,
            rows.get(run, index).ok_or(Stop::InvalidProgram)?,
            assignment,
            bound,
            work,
        )
    };
    let start = boundary(0..length, compare, Ordering::is_lt, work)?;
    let end = boundary(start..length, compare, Ordering::is_le, work)?;
    Ok(start..end)
}

/// The rows of a view matching one bound prefix, visited run by run. Each
/// run's window is one binary search, taken when the cursor enters the run,
/// so an unentered run costs nothing; a dense view's window is its block of
/// positions, of which the set ones are the rows.
#[derive(Clone, Debug)]
pub(super) struct Window {
    run: usize,
    range: Range<usize>,
}

impl Window {
    pub(super) fn open(
        pattern: &AtomPattern,
        rows: Rows<'_>,
        assignment: &[Option<&Value>],
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        Ok(Self {
            run: 0,
            range: matching_prefix(pattern, rows, 0, assignment, work)?,
        })
    }

    /// The next matching row as its run and position, or `None` once every
    /// run's window is exhausted.
    pub(super) fn next(
        &mut self,
        pattern: &AtomPattern,
        rows: Rows<'_>,
        assignment: &[Option<&Value>],
        work: &mut Work<'_>,
    ) -> Result<Option<(usize, usize)>, Stop> {
        loop {
            if let Some(position) = rows.next_row(&mut self.range, work)? {
                return Ok(Some((self.run, position)));
            }
            self.run += 1;
            if self.run >= rows.runs() {
                return Ok(None);
            }
            self.range = matching_prefix(pattern, rows, self.run, assignment, work)?;
        }
    }
}

/// The predicate is true on the initial segment of the supplied interval.
/// At each step the answer remains in the closed boundary range [start, end];
/// end - start strictly decreases. Midpoint arithmetic stays within that range.
fn boundary(
    mut range: Range<usize>,
    compare: impl Fn(usize, &mut Work<'_>) -> Result<Ordering, Stop>,
    before: impl Fn(Ordering) -> bool,
    work: &mut Work<'_>,
) -> Result<usize, Stop> {
    while range.start < range.end {
        let middle = range.start + (range.end - range.start) / 2;
        if before(compare(middle, work)?) {
            range.start = middle + 1;
        } else {
            range.end = middle;
        }
    }
    Ok(range.start)
}

fn compare_prefix(
    pattern: &AtomPattern,
    row: Row<'_>,
    assignment: &[Option<&Value>],
    length: usize,
    work: &mut Work<'_>,
) -> Result<Ordering, Stop> {
    for (term, value) in pattern.terms()[..length].iter().zip(row.values()) {
        work.tick()?;
        let expected = resolve(term, assignment).ok_or(Stop::InvalidProgram)?;
        for compared in [value, expected] {
            work.charge(compared.payload_bytes())?;
        }
        let order = value.cmp(expected);
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}

#[cfg(test)]
mod tests;
