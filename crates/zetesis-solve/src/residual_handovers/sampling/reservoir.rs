//! Fixed-capacity Algorithm R. Population ordinals include missing measurements.

use std::mem::size_of;
use zetesis_algebra::MeterBytes;
use zetesis_backend::residual_evidence::Draw;

use super::ResidualSamplePopulation;

pub(super) const WORLD_TAG: u64 = 0x574f_524c_445f_5631;
pub(super) const BATCH_TAG: u64 = 0x4241_5443_485f_5631;

pub(super) fn bytes<T>(count: usize) -> u128 {
    count as u128 * size_of::<T>() as u128
}

/// Return the largest observed simultaneously-live payload even when the
/// allocator supplied excess capacity that cannot be kept.
pub(super) fn reserve<T>(count: usize, limit: u128, live: u128, peak: &mut u128) -> Option<Vec<T>> {
    MeterBytes::check(bytes::<T>(count), limit).ok()?;
    let mut storage = Vec::new();
    storage.try_reserve_exact(count).ok()?;
    let actual = bytes::<T>(storage.capacity());
    *peak = (*peak).max(live + actual);
    MeterBytes::check(actual, limit).ok()?;
    Some(storage)
}

pub(super) struct Reservoir<T> {
    limit: usize,
    seen: Option<u64>,
    draws: Draw,
    pub(super) records: Vec<T>,
}

impl<T> Reservoir<T> {
    pub(super) fn new(limit: usize, seed: u64, tag: u64) -> Self {
        Self {
            limit,
            seen: Some(0),
            draws: Draw::new(seed, tag),
            records: Vec::new(),
        }
    }

    pub(super) fn population(&self) -> ResidualSamplePopulation {
        ResidualSamplePopulation {
            seen: self.seen,
            capacity: self.limit,
            retained: self.records.len(),
        }
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        bytes::<T>(self.records.capacity())
    }

    pub(super) fn lose_population(&mut self) {
        self.seen = None;
        self.limit = 0;
        self.records = Vec::new();
    }

    pub(super) fn insert(&mut self, record: T, live: u128, peak: &mut u128) {
        self.seen = self.seen.and_then(|n| n.checked_add(1));
        let Some(seen) = self.seen else {
            self.lose_population();
            return;
        };
        if self.limit == 0 {
            return;
        }
        if self.records.capacity() == 0 {
            let Some(rows) = reserve(self.limit, bytes::<T>(self.limit), live, peak) else {
                self.limit = 0;
                return;
            };
            self.records = rows;
        }
        if self.records.len() < self.limit {
            self.records.push(record);
        } else if let Ok(index) = usize::try_from(self.draws.below(seen))
            && let Some(slot) = self.records.get_mut(index)
        {
            *slot = record;
        }
    }
}
