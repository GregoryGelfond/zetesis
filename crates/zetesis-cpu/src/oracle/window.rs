//! Ordered relation windows for the leading terms whose values are known.
//!
//! A full match agrees with every bound leading term. In a relation sorted by
//! tuple storage order, all such rows form a contiguous interval. Two monotone
//! boundary searches recover that interval without cloning values or building an
//! index. Remaining columns still need the ordinary transactional matcher.

use std::{cmp::Ordering, ops::Range};

use zetesis_core::{Atom, AtomPattern, Value};

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
    rows: &[&Atom],
    assignment: &[Option<&Value>],
    work: &mut Work<'_>,
) -> Result<Range<usize>, Stop> {
    if rows.is_empty() {
        return Ok(0..0);
    }
    let mut length = 0;
    for term in pattern.terms() {
        work.tick()?;
        if resolve(term, assignment).is_none() {
            break;
        }
        length += 1;
    }
    if length == 0 {
        return Ok(0..rows.len());
    }
    let compare =
        |index, work: &mut Work<'_>| compare_prefix(pattern, rows[index], assignment, length, work);
    let start = boundary(0..rows.len(), compare, Ordering::is_lt, work)?;
    let end = boundary(start..rows.len(), compare, Ordering::is_le, work)?;
    Ok(start..end)
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
    row: &Atom,
    assignment: &[Option<&Value>],
    length: usize,
    work: &mut Work<'_>,
) -> Result<Ordering, Stop> {
    for (term, value) in pattern.terms()[..length].iter().zip(row.values()) {
        work.tick()?;
        let expected = resolve(term, assignment).ok_or(Stop::InvalidProgram)?;
        for compared in [value, expected] {
            for _ in 0..compared.payload_bytes() {
                work.tick()?;
            }
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
