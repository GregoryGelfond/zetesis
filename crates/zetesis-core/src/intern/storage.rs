//! One bounded owner for an interner and its still-live generation remaps.
//!
//! Named vector/table capacity and replacement overlap are measured, not RSS.
//! A refused proposal leaves no observed peak. A successful allocation is
//! settled before any later refusal; dropping its owner releases its capacity.

use std::sync::{Arc, Mutex, PoisonError};

use hashbrown::HashTable;
use zetesis_algebra::MeterBytes;

use super::Refusal;

#[derive(Debug)]
struct State {
    current: u128,
    peak: u128,
    limit: usize,
}

/// The single byte account shared by an interner and its generation remaps.
/// Keep this handle to inspect capacity after an operation refuses. Cloning
/// shares the account and does not provide another allowance.
#[derive(Clone, Debug)]
pub struct Storage(Arc<Mutex<State>>);

impl Storage {
    /// Start one account, including its named synchronization/control storage.
    /// # Errors
    /// Refuses when the account header itself exceeds `max_bytes`.
    /// The fixed Arc control allocation retains Rust's infallible allocation
    /// boundary, as do the catalog's existing shared-owner headers.
    pub fn new(max_bytes: usize) -> Result<Self, Refusal> {
        let header = (size_of::<Mutex<State>>() + 2 * size_of::<usize>()) as u128;
        check(header, max_bytes)?;
        Ok(Self(Arc::new(Mutex::new(State {
            current: header,
            peak: header,
            limit: max_bytes,
        }))))
    }

    /// Live named capacities, including every outstanding generation remap.
    #[must_use]
    pub fn current_bytes(&self) -> u128 {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .current
    }

    /// Observed live capacity and successful replacement overlap, not RSS.
    #[must_use]
    pub fn peak_bytes(&self) -> u128 {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).peak
    }

    pub(super) fn add(&self, bytes: u128) -> Result<(), Refusal> {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let next = state.current.checked_add(bytes).ok_or(Refusal::Exhausted)?;
        check(next, state.limit)?;
        state.current = next;
        state.peak = state.peak.max(next);
        Ok(())
    }

    pub(super) fn release(&self, bytes: u128) {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        state.current = state
            .current
            .checked_sub(bytes)
            .expect("one capacity owner");
    }

    pub(super) fn reserve<T>(
        &self,
        values: &mut Vec<T>,
        additional: usize,
        exact: bool,
    ) -> Result<(), Refusal> {
        let needed = values
            .len()
            .checked_add(additional)
            .ok_or(Refusal::Exhausted)?;
        if needed <= values.capacity() {
            return Ok(());
        }
        let target = if exact {
            needed
        } else {
            needed.max(values.capacity().saturating_mul(2)).max(4)
        };
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let requested = target as u128 * size_of::<T>() as u128;
        check(
            state
                .current
                .checked_add(requested)
                .ok_or(Refusal::Exhausted)?,
            state.limit,
        )?;
        let before = capacity(values);
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| Refusal::Allocation)?;
        observe(&mut state, before, capacity(values))
    }

    pub(super) fn table(
        &self,
        table: &mut HashTable<u32>,
        additional: usize,
        hash: impl Fn(&u32) -> u64,
    ) -> Result<(), Refusal> {
        let needed = table
            .len()
            .checked_add(additional)
            .ok_or(Refusal::Exhausted)?;
        if needed <= table.capacity() {
            return Ok(());
        }
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        // Predict the pinned hashbrown u32-table growth before allocating;
        // observed capacity is still checked, so an allocator/library change
        // cannot escape the receipt. There are no deleted entries here.
        let requested = table_capacity(needed)? as u128 * 5;
        check(
            state
                .current
                .checked_add(requested)
                .ok_or(Refusal::Exhausted)?,
            state.limit,
        )?;
        let before = super::table_bytes(table);
        table
            .try_reserve(additional, hash)
            .map_err(|_| Refusal::Allocation)?;
        observe(&mut state, before, super::table_bytes(table))
    }
}

/// Prospective addressable u32 capacity under hashbrown 0.17's growth rule:
/// 3/7/14 for small tables, then 7/8 of a power-of-two bucket count. u32
/// cells need no small-item alignment adjustment on the supported backends.
/// Keep the real-table comparison test with any dependency/layout change.
fn table_capacity(needed: usize) -> Result<usize, Refusal> {
    if needed < 4 {
        return Ok(3);
    }
    if needed < 8 {
        return Ok(7);
    }
    if needed < 15 {
        return Ok(14);
    }
    let adjusted = needed.checked_mul(8).ok_or(Refusal::Exhausted)? / 7;
    let buckets = adjusted
        .checked_next_power_of_two()
        .ok_or(Refusal::Exhausted)?;
    Ok(buckets / 8 * 7)
}

fn check(required: u128, limit: usize) -> Result<(), Refusal> {
    MeterBytes::check(required, limit as u128).map_err(|_| Refusal::Storage { required, limit })
}

fn observe(state: &mut State, before: u128, after: u128) -> Result<(), Refusal> {
    let overlap = state.current.checked_add(after).ok_or(Refusal::Exhausted)?;
    state.current = state
        .current
        .checked_sub(before)
        .and_then(|n| n.checked_add(after))
        .ok_or(Refusal::Exhausted)?;
    state.peak = state.peak.max(overlap);
    check(overlap, state.limit)
}

pub(super) fn capacity<T>(values: &Vec<T>) -> u128 {
    values.capacity() as u128 * size_of::<T>() as u128
}

/// Temporary outer storage. Payload owners moved into it keep their own
/// receipts; this guard releases only this vector's capacity on every exit.
pub(super) struct Scratch<'a, T> {
    pub(super) values: Vec<T>,
    storage: &'a Storage,
}

impl<'a, T> Scratch<'a, T> {
    pub(super) fn new(storage: &'a Storage, count: usize) -> Result<Self, Refusal> {
        let mut scratch = Self {
            values: Vec::new(),
            storage,
        };
        storage.reserve(&mut scratch.values, count, true)?;
        Ok(scratch)
    }
}

impl<T> Drop for Scratch<'_, T> {
    fn drop(&mut self) {
        self.storage.release(capacity(&self.values));
    }
}

#[cfg(test)]
mod tests;
