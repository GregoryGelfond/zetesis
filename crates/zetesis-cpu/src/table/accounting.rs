//! Operation-local checked capacity and interruptible work accounting.

use std::{cmp::Ordering, mem::size_of};

use zetesis_core::catalog::TermRef;

use crate::{Cancellation, Stop};

use super::{Cause, Failure, Limits, MeteredCause, MeteredFailure, Resource, Statistics};

pub(super) struct Work<F> {
    limits: Limits,
    before: F,
    used: u64,
    live: usize,
    peak: usize,
}

impl<E, F: FnMut(usize) -> Result<(), E>> Work<F> {
    pub(super) fn new(
        limits: Limits,
        before: F,
        external: usize,
        frame: usize,
    ) -> Result<Self, MeteredFailure<E>> {
        let mut work = Self {
            limits,
            before,
            used: 0,
            live: 0,
            peak: 0,
        };
        let result = (|| {
            work.poll()?;
            let bytes = external.checked_add(frame).ok_or(Cause::Overflow)?;
            work.admit(bytes)?;
            work.live = bytes;
            Ok(())
        })();
        result.map_err(|cause| work.failure(cause))?;
        Ok(work)
    }

    pub(super) fn failure(&self, cause: MeteredCause<E>) -> MeteredFailure<E> {
        MeteredFailure {
            cause,
            work: self.used,
            peak_bytes: self.peak,
        }
    }

    pub(super) fn statistics(&self, external: usize) -> Statistics {
        Statistics {
            work: self.used,
            retained_bytes: self.live - external,
            peak_bytes: self.peak,
        }
    }

    pub(super) fn entries(&self, observed: usize) -> Result<(), MeteredCause<E>> {
        ceiling(
            Resource::Entries,
            observed as u128,
            self.limits.max_entries as u128,
        )
        .map_err(MeteredCause::Table)
    }

    pub(super) fn tick(&mut self, amount: usize) -> Result<(), MeteredCause<E>> {
        if amount == 0 {
            return Ok(());
        }
        self.poll()?;
        let next = u128::from(self.used) + amount as u128;
        ceiling(Resource::Work, next, u128::from(self.limits.max_work))?;
        let next = u64::try_from(next).map_err(|_| Cause::Overflow)?;
        (self.before)(amount).map_err(MeteredCause::Stopped)?;
        self.used = next;
        Ok(())
    }

    fn poll(&mut self) -> Result<(), MeteredCause<E>> {
        (self.before)(0).map_err(MeteredCause::Stopped)
    }

    pub(super) fn compare(
        &mut self,
        left: TermRef<'_>,
        right: TermRef<'_>,
    ) -> Result<Ordering, MeteredCause<E>> {
        left.compare_ref_with(right, || self.tick(1))
    }

    fn admit(&mut self, bytes: usize) -> Result<(), MeteredCause<E>> {
        ceiling(
            Resource::Bytes,
            bytes as u128,
            self.limits.max_bytes as u128,
        )?;
        self.peak = self.peak.max(bytes);
        Ok(())
    }

    pub(super) fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, MeteredCause<E>> {
        self.poll()?;
        let proposed = bytes::<T>(count)?
            .checked_add(self.live)
            .ok_or(Cause::Overflow)?;
        self.admit(proposed)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Cause::Allocation)?;
        let actual = bytes::<T>(values.capacity())?
            .checked_add(self.live)
            .ok_or(Cause::Overflow)?;
        self.peak = self.peak.max(actual);
        self.admit(actual)?;
        self.live = actual;
        Ok(values)
    }

    pub(super) fn zeros(&mut self, count: usize) -> Result<Vec<u32>, MeteredCause<E>> {
        let mut words = self.reserve(count)?;
        for _ in 0..count {
            self.tick(1)?;
            words.push(0);
        }
        Ok(words)
    }

    pub(super) fn grow<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), MeteredCause<E>> {
        self.poll()?;
        let needed = values
            .len()
            .checked_add(additional)
            .ok_or(Cause::Overflow)?;
        if needed <= values.capacity() {
            return Ok(());
        }
        let old = bytes::<T>(values.capacity())?;
        let proposed = needed.max(values.capacity().saturating_mul(2));
        self.admit(
            self.live
                .checked_add(bytes::<T>(proposed)?)
                .ok_or(Cause::Overflow)?,
        )?;
        self.tick(values.len())?;
        values
            .try_reserve_exact(proposed - values.len())
            .map_err(|_| Cause::Allocation)?;
        let actual = bytes::<T>(values.capacity())?;
        let simultaneous = self.live.checked_add(actual).ok_or(Cause::Overflow)?;
        self.peak = self.peak.max(simultaneous);
        self.admit(simultaneous)?;
        self.live = self.live - old + actual;
        Ok(())
    }

    pub(super) fn release<T>(&mut self, values: Vec<T>) {
        self.live -= values.capacity() * size_of::<T>();
        drop(values);
    }

    pub(super) fn release_bytes(&mut self, bytes: usize) {
        self.live -= bytes;
    }
}

/// Legacy entry points poll at the existing control boundaries only. Positive
/// work requests need no additional policy beyond the operation's local ceiling.
pub(super) fn control(cancellation: &Cancellation) -> impl FnMut(usize) -> Result<(), Stop> + '_ {
    move |amount| {
        if amount == 0 {
            cancellation.poll()
        } else {
            Ok(())
        }
    }
}

pub(super) fn unmetered(error: MeteredFailure<Stop>) -> Failure {
    Failure {
        cause: match error.cause {
            MeteredCause::Table(cause) => cause,
            MeteredCause::Stopped(stop) => Cause::Interrupted(stop),
        },
        work: error.work,
        peak_bytes: error.peak_bytes,
    }
}

fn bytes<T>(count: usize) -> Result<usize, Cause> {
    count.checked_mul(size_of::<T>()).ok_or(Cause::Overflow)
}

fn ceiling(resource: Resource, observed: u128, limit: u128) -> Result<(), Cause> {
    if observed > limit {
        Err(Cause::Limit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Stop;

    #[test]
    fn interrupted_charge_preserves_the_completed_prefix() {
        let cancellation = Cancellation::default();
        let mut work = Work::new(Limits::default(), control(&cancellation), 0, 0).unwrap();
        work.tick(3).unwrap();
        cancellation.cancel();
        work.tick(0).unwrap();
        let cause = work.tick(1).unwrap_err();
        assert_eq!(
            unmetered(work.failure(cause)).cause,
            Cause::Interrupted(Stop::Cancelled)
        );
        assert_eq!(work.statistics(0).work, 3);
    }

    #[test]
    fn rejected_charge_does_not_increment_work() {
        let cancellation = Cancellation::default();
        let mut work = Work::new(
            Limits {
                max_work: 3,
                ..Limits::default()
            },
            control(&cancellation),
            0,
            0,
        )
        .unwrap();
        work.tick(3).unwrap();
        let cause = work.tick(1).unwrap_err();
        assert_eq!(
            unmetered(work.failure(cause)).cause,
            Cause::Limit {
                resource: Resource::Work,
                observed: 4,
                limit: 3
            }
        );
        assert_eq!(work.statistics(0).work, 3);
    }

    #[test]
    fn refused_growth_preserves_the_original_buffer() {
        let cancellation = Cancellation::default();
        let mut work = Work::new(Limits::default(), control(&cancellation), 0, 0).unwrap();
        let mut values = work.reserve::<u32>(2).unwrap();
        let capacity = values.capacity();
        values.resize(capacity, 7);
        let proposed = (capacity + 1).max(2 * capacity);
        let required = (capacity + proposed) * size_of::<u32>();
        work.limits.max_bytes = required - 1;
        let cause = work.grow(&mut values, 1).unwrap_err();
        assert_eq!(
            unmetered(work.failure(cause)).cause,
            Cause::Limit {
                resource: Resource::Bytes,
                observed: required as u128,
                limit: (required - 1) as u128
            }
        );
        assert_eq!(values.len(), capacity);
        assert!(values.iter().all(|value| *value == 7));
        assert_eq!(values.capacity(), capacity);
    }

    #[test]
    fn growth_accounts_for_coexisting_buffers() {
        let cancellation = Cancellation::default();
        let mut work = Work::new(Limits::default(), control(&cancellation), 0, 0).unwrap();
        let mut values = work.reserve::<u32>(2).unwrap();
        let initial = values.capacity();
        values.resize(initial, 7);
        work.grow(&mut values, 1).unwrap();
        let final_capacity = values.capacity();
        assert!(final_capacity > initial);
        assert_eq!(
            work.statistics(0).peak_bytes,
            (initial + final_capacity) * size_of::<u32>()
        );
        assert_eq!(
            work.statistics(0).retained_bytes,
            final_capacity * size_of::<u32>()
        );
    }

    #[test]
    fn reserve_zero_polls_caller_control() {
        let stopped = std::cell::Cell::new(false);
        let mut work = Work::new(
            Limits::default(),
            |amount| {
                assert_eq!(amount, 0);
                if stopped.get() {
                    Err("reservation stopped")
                } else {
                    Ok(())
                }
            },
            0,
            0,
        )
        .unwrap();
        stopped.set(true);
        let cause = work.reserve::<u32>(0).unwrap_err();
        assert_eq!(cause, MeteredCause::Stopped("reservation stopped"));
        assert_eq!(work.failure(cause).work, 0);
        assert_eq!(work.statistics(0).peak_bytes, 0);
    }

    #[test]
    fn growth_polls_even_without_reallocation() {
        let stopped = std::cell::Cell::new(false);
        let mut work = Work::new(
            Limits::default(),
            |amount| {
                assert_eq!(amount, 0);
                if stopped.get() {
                    Err("growth stopped")
                } else {
                    Ok(())
                }
            },
            0,
            0,
        )
        .unwrap();
        let mut values = work.reserve::<u32>(1).unwrap();
        values.push(7);
        let capacity = values.capacity();
        let peak = work.statistics(0).peak_bytes;
        stopped.set(true);
        let cause = work.grow(&mut values, 0).unwrap_err();
        assert_eq!(cause, MeteredCause::Stopped("growth stopped"));
        assert_eq!(values, [7]);
        assert_eq!(values.capacity(), capacity);
        assert_eq!(work.statistics(0).peak_bytes, peak);
        assert_eq!(work.failure(cause).work, 0);
    }

    #[test]
    fn refused_group_adds_no_local_work() {
        let mut requested = Vec::new();
        {
            let mut work = Work::new(
                Limits::default(),
                |amount| {
                    if amount != 0 {
                        requested.push(amount);
                    }
                    if amount == 4 { Err(amount) } else { Ok(()) }
                },
                0,
                0,
            )
            .unwrap();
            work.tick(3).unwrap();
            let cause = work.tick(4).unwrap_err();
            assert_eq!(cause, MeteredCause::Stopped(4));
            assert_eq!(work.failure(cause).work, 3);
        }
        assert_eq!(requested, [3, 4]);
    }
}
