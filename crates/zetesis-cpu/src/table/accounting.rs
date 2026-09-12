//! Operation-local checked capacity and interruptible work accounting.

use std::{cmp::Ordering, mem::size_of};

use zetesis_core::Value;

use crate::Control;

use super::{Cause, Failure, Limits, Resource, Statistics};

pub(super) struct Work<'a> {
    limits: Limits,
    control: &'a Control,
    used: u64,
    live: usize,
    peak: usize,
}

impl<'a> Work<'a> {
    pub(super) fn new(
        limits: Limits,
        control: &'a Control,
        external: usize,
        frame: usize,
    ) -> Result<Self, Failure> {
        let mut work = Self {
            limits,
            control,
            used: 0,
            live: 0,
            peak: 0,
        };
        let result = (|| {
            control.poll().map_err(Cause::Interrupted)?;
            let bytes = external.checked_add(frame).ok_or(Cause::Overflow)?;
            work.admit(bytes)?;
            work.live = bytes;
            Ok(())
        })();
        result.map_err(|cause| work.failure(cause))?;
        Ok(work)
    }

    pub(super) fn failure(&self, cause: Cause) -> Failure {
        Failure {
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

    pub(super) fn entries(&self, observed: usize) -> Result<(), Cause> {
        ceiling(
            Resource::Entries,
            observed as u128,
            self.limits.max_entries as u128,
        )
    }

    pub(super) fn tick(&mut self, amount: usize) -> Result<(), Cause> {
        if amount == 0 {
            return Ok(());
        }
        self.control.poll().map_err(Cause::Interrupted)?;
        let next = u128::from(self.used) + amount as u128;
        ceiling(Resource::Work, next, u128::from(self.limits.max_work))?;
        self.used = u64::try_from(next).map_err(|_| Cause::Overflow)?;
        Ok(())
    }

    pub(super) fn compare(&mut self, left: &Value, right: &Value) -> Result<Ordering, Cause> {
        let amount = left
            .payload_bytes()
            .checked_add(right.payload_bytes())
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(Cause::Overflow)?;
        self.tick(amount)?;
        Ok(left.cmp(right))
    }

    fn admit(&mut self, bytes: usize) -> Result<(), Cause> {
        ceiling(
            Resource::Bytes,
            bytes as u128,
            self.limits.max_bytes as u128,
        )?;
        self.peak = self.peak.max(bytes);
        Ok(())
    }

    pub(super) fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, Cause> {
        self.control.poll().map_err(Cause::Interrupted)?;
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

    pub(super) fn zeros(&mut self, count: usize) -> Result<Vec<u32>, Cause> {
        let mut words = self.reserve(count)?;
        for _ in 0..count {
            self.tick(1)?;
            words.push(0);
        }
        Ok(words)
    }

    pub(super) fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Cause> {
        self.control.poll().map_err(Cause::Interrupted)?;
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
        let control = Control::default();
        let mut work = Work::new(Limits::default(), &control, 0, 0).unwrap();
        work.tick(3).unwrap();
        control.cancel();
        work.tick(0).unwrap();
        let cause = work.tick(1).unwrap_err();
        assert_eq!(cause, Cause::Interrupted(Stop::Cancelled));
        assert_eq!(work.failure(cause).work, 3);
    }

    #[test]
    fn rejected_charge_does_not_increment_work() {
        let control = Control::default();
        let mut work = Work::new(
            Limits {
                max_work: 3,
                ..Limits::default()
            },
            &control,
            0,
            0,
        )
        .unwrap();
        work.tick(3).unwrap();
        let cause = work.tick(1).unwrap_err();
        assert_eq!(
            cause,
            Cause::Limit {
                resource: Resource::Work,
                observed: 4,
                limit: 3
            }
        );
        assert_eq!(work.failure(cause).work, 3);
    }

    #[test]
    fn refused_growth_preserves_the_original_buffer() {
        let control = Control::default();
        let mut work = Work::new(
            Limits {
                max_bytes: 20,
                ..Limits::default()
            },
            &control,
            0,
            0,
        )
        .unwrap();
        let mut values = work.reserve::<u32>(2).unwrap();
        values.extend([1, 2]);
        let capacity = values.capacity();
        let cause = work.grow(&mut values, 1).unwrap_err();
        assert_eq!(
            cause,
            Cause::Limit {
                resource: Resource::Bytes,
                observed: 24,
                limit: 20
            }
        );
        assert_eq!(values, [1, 2]);
        assert_eq!(values.capacity(), capacity);
    }

    #[test]
    fn growth_accounts_for_coexisting_buffers() {
        let control = Control::default();
        let mut work = Work::new(
            Limits {
                max_bytes: 24,
                ..Limits::default()
            },
            &control,
            0,
            0,
        )
        .unwrap();
        let mut values = work.reserve::<u32>(2).unwrap();
        values.extend([1, 2]);
        work.grow(&mut values, 1).unwrap();
        assert_eq!(work.statistics(0).peak_bytes, 24);
        assert_eq!(work.statistics(0).retained_bytes, 16);
    }
}
