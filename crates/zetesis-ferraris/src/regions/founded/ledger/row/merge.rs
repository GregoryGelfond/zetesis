//! Bounded output union over identical, canonical signed conditions.
//!
//! A first ordered pass counts distinct outputs without allocating or changing
//! either input. A second pass fills a reserved temporary buffer. Only complete
//! success replaces the temporary explanation's outputs; the retained row never
//! changes. Every comparison and union step shares the capture allowance.

use super::{Explanation, Row, Stop, Work, capture_read};

#[cfg(test)]
mod tests;

pub(super) fn union_outputs(
    row: &Row,
    other: &mut Explanation,
    maximum: usize,
    remaining: &mut u64,
    work: &mut Work<'_>,
    peak: &mut u128,
) -> Result<Option<bool>, Stop> {
    let (conditions, outputs) = row.lengths();
    if conditions != other.conditions.len()
        || outputs > maximum
        || other.unsupported.len() > maximum
    {
        return Ok(Some(false));
    }
    let mut premises = row.conditions();
    for position in 0..conditions {
        if !capture_read(remaining, work)? {
            return Ok(None);
        }
        let left = premises
            .next()
            .expect("the row's logical condition count matches its iterator");
        if left != other.conditions[position] {
            return Ok(Some(false));
        }
    }

    let count = {
        let mut count = 0;
        let mut values = union(row.outputs(), other.unsupported.iter().copied());
        loop {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            if values.next().is_none() {
                break;
            }
            if count == maximum {
                return Ok(Some(false));
            }
            count += 1;
        }
        count
    };
    // Every old output belongs to the union, so equal cardinality proves that
    // the proposed buffer already represents the complete union.
    if count == other.unsupported.len() {
        return Ok(Some(true));
    }

    let simultaneous =
        size_of::<Explanation>() as u128 + size_of::<Vec<usize>>() as u128 + other.retained_bytes();
    *peak = (*peak).max(simultaneous);
    let mut combined = Vec::new();
    combined
        .try_reserve_exact(count)
        .map_err(|_| Stop::Allocation)?;
    *peak = (*peak).max(simultaneous + combined.capacity() as u128 * size_of::<usize>() as u128);
    {
        let mut values = union(row.outputs(), other.unsupported.iter().copied());
        for _ in 0..count {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            combined.push(
                values
                    .next()
                    .expect("the count pass exhausts the same ordered output streams"),
            );
        }
    }
    other.unsupported = combined;
    Ok(Some(true))
}

/// Both inputs are already strictly increasing. An equality advances both, so
/// this stream retains each output once without another ordering or dedup pass.
fn union(
    left: impl Iterator<Item = usize>,
    right: impl Iterator<Item = usize>,
) -> impl Iterator<Item = usize> {
    let mut left = left.peekable();
    let mut right = right.peekable();
    std::iter::from_fn(
        move || match (left.peek().copied(), right.peek().copied()) {
            (Some(a), Some(b)) if a < b => left.next(),
            (Some(a), Some(b)) if a == b => {
                right.next();
                left.next()
            }
            (_, Some(_)) => right.next(),
            (Some(_), None) => left.next(),
            (None, None) => None,
        },
    )
}
