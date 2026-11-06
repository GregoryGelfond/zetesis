//! Shared storage and counted reductions for original-region arithmetic.
//!
//! These helpers borrow the existing Meter. They neither poll nor refund;
//! the outer attempted operation owns both. Allocation internals and ABI
//! transport are excluded, as in preparation Storage. Logical field writes,
//! initialized copies and length changes are admitted before they execute.

use std::mem::size_of;

use zetesis_algebra::{
    Cancellation, Failure, Meter, Quota, Stop,
    region::SubjectStep,
    words::{self, Words},
};

use super::super::NarrowingStatistics;

/// One complete arithmetic inspection/application batch and its real prefix.
/// A changed batch is local progress, not a joint or local fixed-point claim.
#[derive(Debug)]
#[must_use = "account the receipt and inspect the result before reusing state"]
pub struct ArithmeticAttempt<E = Stop> {
    /// Local progress or the unchanged caller/control failure.
    pub result: Result<SubjectStep, E>,
    /// Performed work and applied decisions, including failed prefixes.
    pub statistics: NarrowingStatistics,
}

/// Sequentially shared worker storage; never copied with a region.
#[derive(Debug, Default)]
pub(in crate::regions) struct ArithmeticScratch {
    pub(super) touched: Words<u64>,
    pub(super) open: Words<u64>,
    pub(super) consequences: Vec<(usize, bool)>,
}

impl ArithmeticScratch {
    /// Payload capacities only: the enclosing worker counts this record once.
    pub(in crate::regions) fn allocated_bytes(&self) -> u128 {
        (self.touched.capacity() as u128 + self.open.capacity() as u128) * size_of::<u64>() as u128
            + self.consequences.capacity() as u128 * size_of::<(usize, bool)>() as u128
    }
}

/// Common copy boundary for opaque arithmetic knowledge.
///
/// Poll before inspecting the optional destination. The initializer charges its
/// own real empty record fields; this helper charges the presence test and a
/// new Some tag. The copy closure charges all initialized semantic payload.
/// Any failure makes the destination unavailable. That finalization and Drop
/// are failure cleanup, not another semantic read or copy after refusal.
/// No refund or exit poll occurs here; the outer owner publishes the Meter
/// delta and refunds at its existing copy-attempt boundary.
pub(super) fn copy_attempt<T, Q: Quota + ?Sized>(
    source: &T,
    destination: &mut Option<T>,
    cancellation: &Cancellation,
    work: &mut Meter<'_, Q>,
    initialize: impl FnOnce(&mut Meter<'_, Q>) -> Result<T, Failure<Q::Error>>,
    copy: impl FnOnce(&mut T, &T, &mut Meter<'_, Q>) -> Result<(), Failure<Q::Error>>,
) -> Result<(), Failure<Q::Error>> {
    let result = cancellation
        .poll()
        .map_err(Failure::Stopped)
        .and_then(|()| {
            charge(work, 1)?;
            if destination.is_none() {
                let empty = initialize(work)?;
                charge(work, 1)?;
                *destination = Some(empty);
            }
            copy(
                destination
                    .as_mut()
                    .expect("copy destination is initialized"),
                source,
                work,
            )
        });
    if result.is_err() {
        *destination = None;
    }
    result
}

/// Admit a guaranteed primitive chunk immediately before its execution.
/// The caller runs all of it after success; use `Meter::bounded` directly when
/// the operation may itself return a shorter performed prefix.
pub(super) fn charge<Q: Quota + ?Sized>(
    work: &mut Meter<'_, Q>,
    count: u64,
) -> Result<(), Failure<Q::Error>> {
    work.bounded(count, || ((), count))
}

fn reserved<T, Q: Quota + ?Sized>(
    length: usize,
    work: &mut Meter<'_, Q>,
) -> Result<Vec<T>, Failure<Q::Error>> {
    charge(work, 3)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| Failure::Stopped(Stop::Allocation))?;
    Ok(values)
}

/// Replace initialized values, reusing capacity only when sufficient.
/// A refused replacement may have overwritten a prefix; discard the target.
/// The source accessor is an internal infallible value/read, not a public hook.
fn overwrite<T: Copy, Q: Quota + ?Sized>(
    target: &mut Vec<T>,
    length: usize,
    fields: u64,
    mut value: impl FnMut(usize) -> T,
    acquire: impl FnOnce(usize, &mut Meter<'_, Q>) -> Result<Vec<T>, Failure<Q::Error>>,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    charge(work, 1)?;
    if target.capacity() < length {
        let mut next = acquire(length, work)?;
        for index in 0..length {
            charge(
                work,
                fields
                    .checked_add(1)
                    .ok_or(Failure::Stopped(Stop::WorkLimit))?,
            )?;
            next.push(value(index));
        }
        charge(work, 6)?;
        std::mem::swap(target, &mut next);
    } else {
        let previous = target.len();
        for (index, destination) in target.iter_mut().enumerate().take(length.min(previous)) {
            charge(work, fields)?;
            *destination = value(index);
        }
        for index in previous..length {
            charge(
                work,
                fields
                    .checked_add(1)
                    .ok_or(Failure::Stopped(Stop::WorkLimit))?,
            )?;
            target.push(value(index));
        }
        charge(work, 1)?;
        if previous > length {
            charge(work, 1)?;
            target.truncate(length);
        }
    }
    Ok(())
}

/// Set the exact logical word length to zero, preserving sufficient capacity.
pub(super) fn prepare_words<Q: Quota + ?Sized>(
    target: &mut Words<u64>,
    length: usize,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    overwrite(target.storage_mut(), length, 1, |_| 0, reserved, work)
}

/// Initialize the snapshot through the same metered replacement operation
/// with storage already reserved by the caller. Moving the three-word header
/// takes the place of building it in the ordinary reservation.
pub(super) fn prepare_supplied_words<Q: Quota + ?Sized>(
    target: &mut Words<u64>,
    length: usize,
    supplied: Vec<u64>,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    overwrite(
        target.storage_mut(),
        length,
        1,
        |_| 0,
        |length, work| {
            charge(work, 3)?;
            debug_assert!(supplied.is_empty() && supplied.capacity() >= length);
            Ok(supplied)
        },
        work,
    )
}

/// Copy initialized words without allocation, preserving the next-word refusal.
/// Bulk only an already-admitted run; its body has no callbacks or early exit.
pub(super) fn copy_words<Q: Quota + ?Sized, S: AsRef<[u64]> + AsMut<[u64]>>(
    target: &mut Words<u64, S>,
    source: &[u64],
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    assert_eq!(target.len(), source.len(), "snapshot word shapes agree");
    // Acquire the backing once, as the original mutable iterator did. A
    // generic backing's AsMut need not be free of observable effects.
    let target = &mut **target;
    let length = target.len().min(source.len());
    let mut copied = 0;
    while copied < length {
        let available = usize::try_from(work.remaining()).unwrap_or(usize::MAX);
        let covered = available.min(length - copied);
        if covered == 0 {
            // Replenish only for the next original one-word primitive.
            charge(work, 1)?;
            target[copied] = source[copied];
            copied += 1;
        } else {
            // This complete, infallible slice is already covered by the grant:
            // no quota callback or poll can occur between its word copies.
            let count = u64::try_from(covered).expect("covered words fit the permit balance");
            charge(work, count)?;
            let end = copied + covered;
            for (target, source) in target[copied..end].iter_mut().zip(&source[copied..end]) {
                *target = *source;
            }
            copied = end;
        }
    }
    Ok(())
}

/// Copy the declared initialized scalar fields, never unused capacity.
pub(super) fn copy_values<T: Copy, Q: Quota + ?Sized>(
    target: &mut Vec<T>,
    source: &[T],
    fields: u64,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    overwrite(
        target,
        source.len(),
        fields,
        |index| source[index],
        reserved,
        work,
    )
}

/// Append a scalar record under the existing preparation-style doubling rule.
/// Actual initialized transfers are charged; a failed destination is unusable.
pub(super) fn push<T: Copy, Q: Quota + ?Sized>(
    values: &mut Vec<T>,
    value: T,
    fields: u64,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    charge(work, 1)?;
    if values.len() == values.capacity() {
        let capacity = values.capacity().saturating_mul(2).max(1);
        let mut next = reserved(capacity, work)?;
        for old in values.iter() {
            charge(
                work,
                fields
                    .checked_add(1)
                    .ok_or(Failure::Stopped(Stop::WorkLimit))?,
            )?;
            next.push(*old);
        }
        charge(work, 6)?;
        std::mem::swap(values, &mut next);
    }
    charge(
        work,
        fields
            .checked_add(1)
            .ok_or(Failure::Stopped(Stop::WorkLimit))?,
    )?;
    values.push(value);
    Ok(())
}

/// AND/popcount plus its caller-owned initialization and accumulator additions.
/// On success the charge is 3*length+1. Each AND admits one; each following
/// popcount and guaranteed addition admits two. No whole-row precharge occurs.
pub(super) fn counted_intersection<Q: Quota + ?Sized>(
    left: &[u64],
    right: &[u64],
    work: &mut Meter<'_, Q>,
) -> Result<usize, Failure<Q::Error>> {
    assert_eq!(left.len(), right.len(), "intersection word shapes agree");
    charge(work, 1)?;
    let mut popcount = false;
    words::intersection_count(left, right, &mut || {
        charge(work, if popcount { 2 } else { 1 })?;
        popcount = !popcount;
        Ok(())
    })
}

#[cfg(test)]
mod tests;
