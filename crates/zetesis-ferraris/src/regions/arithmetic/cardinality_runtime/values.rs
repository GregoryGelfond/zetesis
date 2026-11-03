//! Reconstruct one retained occurrence at a particular partial interpretation.
//!
//! Prepared supports are padded, whereas snapshots have the region's logical
//! width. The omitted support suffix is authenticated zero. Guard occurrences
//! stay ordered and are never cached, canonicalized or skipped after a count.

use zetesis_algebra::words::Words;
use zetesis_algebra::{Failure, Meter, Quota, Stop};

use super::super::{
    cardinality_source::Bound,
    runtime::{charge, counted_intersection},
    source::Literal,
    uniform::BoundSide,
};
use super::PairSums;

pub(super) fn effective<Q: Quota + ?Sized>(
    bound: &Bound,
    masks: (&[u64], &[u64]),
    work: &mut Meter<'_, Q>,
) -> Result<u64, Failure<Q::Error>> {
    charge(work, 1)?; // select the authenticated bound direction
    let selected = match bound.side {
        BoundSide::Lower => masks.0,
        BoundSide::Upper => masks.1,
    };
    let count = counted_intersection(&bound.support[..selected.len()], selected, work)?;
    charge(work, 1)?; // materialize the fixed-width count
    let count = u64::try_from(count).map_err(|_| Stop::InvalidProgram)?;
    charge(work, 1)?; // dispatch the effective-count construction
    let value = match bound.side {
        BoundSide::Lower => count,
        BoundSide::Upper => {
            charge(work, 2)?; // subtraction and checked-result inspection
            bound
                .cardinality
                .checked_sub(count)
                .ok_or(Stop::InvalidProgram)?
        }
    };
    let active = guard(&bound.guard, masks, work)?;
    charge(work, 1)?; // inspect the completed guard result
    if active {
        charge(work, 3)?; // direction, clamp comparison and selected value
        let value = match bound.side {
            BoundSide::Lower => bound.value.max(value),
            BoundSide::Upper => bound.value.min(value),
        };
        Ok(value)
    } else {
        Ok(value)
    }
}

fn guard<Q: Quota + ?Sized>(
    literals: &[Literal],
    masks: (&[u64], &[u64]),
    work: &mut Meter<'_, Q>,
) -> Result<bool, Failure<Q::Error>> {
    for literal in literals {
        charge(work, 3)?;
        let (slot, bit) = Words::<u64>::locate(literal.atom);
        charge(work, 1)?;
        let selected = if literal.positive { masks.0 } else { masks.1 };
        charge(work, 2)?; // AND and membership comparison
        if selected[slot] & bit == 0 {
            charge(work, 1)?;
            return Ok(false);
        }
    }
    charge(work, 1)?;
    Ok(true)
}

/// Replace one summand before testing the mixed old/current family bounds.
/// Both old and current bounds apply to the descendant, so a contradiction
/// already excludes it without visiting a later occurrence.
pub(super) fn replace<Q: Quota + ?Sized>(
    sums: &mut PairSums,
    side: BoundSide,
    old: u64,
    current: u64,
    work: &mut Meter<'_, Q>,
) -> Result<bool, Failure<Q::Error>> {
    charge(work, 1)?;
    let sum = match side {
        BoundSide::Lower => &mut sums.lower,
        BoundSide::Upper => &mut sums.upper,
    };
    charge(work, 2)?;
    let rest = sum.checked_sub(old).ok_or(Stop::InvalidProgram)?;
    charge(work, 2)?;
    let next = rest.checked_add(current).ok_or(Stop::InvalidProgram)?;
    charge(work, 1)?;
    *sum = next;
    charge(work, 2)?; // comparison and its complete Boolean result
    Ok(sums.lower > sums.upper)
}

#[cfg(test)]
mod tests;
