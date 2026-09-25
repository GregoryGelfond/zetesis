//! Named-capacity accounting, with requested replacement and observed capacity
//! checks. A refused reservation can leave larger retained capacity; it is
//! charged before returning. These measures are not allocator/RSS guarantees.

use std::mem::size_of;

use super::Fault;

#[derive(Clone, Debug)]
pub(super) struct Budget {
    pub(super) used: u128,
    pub(super) peak: u128,
    pub(super) limit: usize,
}

impl Budget {
    pub(super) fn new(limit: usize, initial: usize) -> Self {
        Self {
            used: initial as u128,
            peak: initial as u128,
            limit,
        }
    }

    pub(super) fn check(&self) -> Result<(), Fault> {
        self.check_extra(0)
    }

    pub(super) fn check_extra(&self, extra: u128) -> Result<(), Fault> {
        let required = self.used.checked_add(extra).ok_or(Fault::Overflow)?;
        if required > self.limit as u128 {
            return Err(Fault::Storage {
                required,
                limit: self.limit,
            });
        }
        Ok(())
    }

    pub(super) fn add(&mut self, bytes: u128) -> Result<(), Fault> {
        self.check_extra(bytes)?;
        self.used += bytes;
        self.peak = self.peak.max(self.used);
        Ok(())
    }

    pub(super) fn observed(&mut self, before: u128, after: u128) -> Result<(), Fault> {
        // Reservation succeeded. Count both old and replacement capacities as
        // a conservative overlap envelope; rejected preflight checks never
        // contribute to this peak. This is not an allocator trace.
        let overlap = self.used.checked_add(after).ok_or(Fault::Overflow)?;
        self.peak = self.peak.max(overlap);
        self.used = self
            .used
            .checked_sub(before)
            .and_then(|n| n.checked_add(after))
            .ok_or(Fault::Overflow)?;
        if overlap > self.limit as u128 {
            return Err(Fault::Storage {
                required: overlap,
                limit: self.limit,
            });
        }
        self.check()
    }
}

pub(super) fn capacity<T>(values: &Vec<T>) -> u128 {
    values.capacity() as u128 * size_of::<T>() as u128
}

fn target_capacity(len: usize, capacity: usize, additional: usize) -> Result<usize, Fault> {
    let count = len.checked_add(additional).ok_or(Fault::Overflow)?;
    Ok(if count <= capacity {
        capacity
    } else {
        count.max(capacity.checked_mul(2).unwrap_or(count)).max(4)
    })
}

/// Known requested capacity, before any allocation. An allocator may still
/// return more, so callers must also check the observed capacity afterwards.
pub(super) fn reservation_bytes<T>(values: &Vec<T>, additional: usize) -> Result<u128, Fault> {
    Ok(
        target_capacity(values.len(), values.capacity(), additional)? as u128
            * size_of::<T>() as u128,
    )
}

pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let target = target_capacity(values.len(), values.capacity(), additional)?;
    if target <= values.capacity() {
        return budget.check();
    }
    let before = capacity(values);
    budget.check_extra(target as u128 * size_of::<T>() as u128)?;
    values
        .try_reserve_exact(target - values.len())
        .map_err(|_| Fault::Allocation)?;
    budget.observed(before, capacity(values))
}

pub(super) fn reserve_text(
    text: &mut String,
    additional: usize,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let target = target_capacity(text.len(), text.capacity(), additional)?;
    if target <= text.capacity() {
        return budget.check();
    }
    let before = text.capacity() as u128;
    budget.check_extra(target as u128)?;
    text.try_reserve_exact(target - text.len())
        .map_err(|_| Fault::Allocation)?;
    budget.observed(before, text.capacity() as u128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_overlap_refuses_excess_allocator_capacity() {
        let mut budget = Budget::new(24, 12);
        assert_eq!(
            budget.observed(8, 16),
            Err(Fault::Storage {
                required: 28,
                limit: 24
            })
        );
        // A refusal cannot undo the completed allocation. Its retained capacity
        // remains charged even though the transient envelope exceeded the limit.
        assert_eq!(budget.used, 20);
        assert_eq!(budget.peak, 28);
    }

    #[test]
    fn observed_overlap_accepts_the_exact_allowance() {
        let mut budget = Budget::new(28, 12);
        assert_eq!(budget.observed(8, 16), Ok(()));
        assert_eq!(budget.used, 20);
        assert_eq!(budget.peak, 28);
    }
}
