//! Fallible aggregate coordinate indexes over the source workspace.
//!
//! Keys contain only fixed-width coordinates. Canonical terms remain in the
//! source authority. Sorted indexes govern lookup, never ASP value order.

use crate::formula_support::{Context, GroundingWork};

use std::cmp::Ordering;

use crate::ProgramSite;
use zetesis_core::catalog::{AssignmentError, Error};

use crate::formula::ceiling;
use crate::formula_support::{
    Buffer, Computation, Counters, StorageLease, growth_capacity, reserve, reserve_exact,
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

/// A sorted map of fixed-width keys and values. Comparisons cost one work unit;
/// callers must not put variable-width payload behind their `Ord` implementation.
pub(super) struct CoordinateMap<K, V> {
    entries: Vec<(K, V)>,
    lease: StorageLease,
}

/// Terminal entries retain the map's allocation and receipt after coordinate
/// lookup is no longer needed. Reordering cannot expose a broken search index.
pub(super) struct Entries<K, V> {
    map: CoordinateMap<K, V>,
}

impl<K, V> Entries<K, V> {
    pub(super) fn len(&self) -> usize {
        self.map.entries.len()
    }

    pub(super) fn slice_mut(&mut self) -> &mut [(K, V)] {
        &mut self.map.entries
    }

    pub(super) fn iter(&self) -> std::slice::Iter<'_, (K, V)> {
        self.map.entries.iter()
    }
}

impl<K: Copy + Ord, V: Copy> CoordinateMap<K, V> {
    pub(super) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        Ok(Self {
            entries: Vec::new(),
            lease,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn slice(&self) -> &[(K, V)] {
        &self.entries
    }
    pub(super) fn iter(&self) -> std::slice::Iter<'_, (K, V)> {
        self.entries.iter()
    }

    /// Move the complete owner, including its named header and lease. This
    /// neither allocates nor releases capacity and cannot be converted back.
    pub(super) fn into_entries(self) -> Entries<K, V> {
        Entries { map: self }
    }

    pub(super) fn reserve(
        &mut self,
        additional: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        reserve(
            &mut self.entries,
            additional,
            &mut self.lease,
            size_of::<Self>(),
            Context::new(computation, limits, counters, location),
        )
    }

    pub(super) fn find(
        &self,
        key: &K,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<V>, FormulaFailure> {
        computation.allowance(&self.lease, limits, location)?;
        Ok(self
            .search(key, limits, counters, location)?
            .ok()
            .map(|at| self.entries[at].1))
    }

    /// All allocation and movement permits precede publication. Refusal can
    /// retain reserved capacity, but preserves every existing key and value.
    pub(super) fn insert(
        &mut self,
        key: K,
        value: V,
        bound: Option<(FormulaResource, usize)>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<V>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        computation.allowance(&self.lease, limits, location)?;
        let at = match self.search(&key, limits, counters, location)? {
            Ok(at) => {
                counters.work(limits, location)?;
                return Ok(Some(std::mem::replace(&mut self.entries[at].1, value)));
            }
            Err(at) => at,
        };
        if let Some((resource, limit)) = bound {
            ceiling(
                resource,
                self.entries.len() as u128 + 1,
                limit as u128,
                location,
            )?;
        }
        self.reserve(1, computation, limits, counters, location)?;
        counters.charge_work((self.entries.len() - at) as u128 + 1, limits, location)?;
        self.entries.insert(at, (key, value));
        Ok(None)
    }

    fn search(
        &self,
        key: &K,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Result<usize, usize>, FormulaFailure> {
        let mut start = 0;
        let mut end = self.entries.len();
        while start < end {
            counters.work(limits, location)?;
            let middle = start + (end - start) / 2;
            match self.entries[middle].0.cmp(key) {
                Ordering::Less => start = middle + 1,
                Ordering::Greater => end = middle,
                Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(start))
    }
}

struct Key {
    aggregate: usize,
    start: usize,
    len: usize,
}

struct Row<V> {
    key: Key,
    value: V,
}

/// First-occurrence rows with a pooled outer-binding key and sorted row index.
/// Values may own separately leased shared aggregate buffers; no value is
/// cloned by lookup or publication. Key comparisons charge each visited slot.
pub(super) struct Contexts<V> {
    rows: Vec<Row<V>>,
    outer: Vec<Option<usize>>,
    order: Vec<usize>,
    lease: StorageLease,
}

impl<V> Contexts<V> {
    pub(super) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        Ok(Self {
            rows: Vec::new(),
            outer: Vec::new(),
            order: Vec::new(),
            lease,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }
    pub(super) fn get(&self, index: usize) -> Option<&V> {
        self.rows.get(index).map(|row| &row.value)
    }
    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut V> {
        self.rows.get_mut(index).map(|row| &mut row.value)
    }

    /// Retained key capacity: row-key headers, outer-coordinate pool and index.
    /// Replacement overlap belongs to `SupportBytes`, not this retained measure.
    /// The fixed empty owner and value storage also belong to `SupportBytes`.
    pub(super) fn key_bytes(&self) -> u128 {
        key_bytes(
            self.rows.capacity(),
            self.outer.capacity(),
            self.order.capacity(),
        )
    }

    pub(super) fn find(
        &self,
        aggregate: usize,
        probe: &Buffer<Option<usize>>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        computation.allowance(&self.lease, limits, location)?;
        Ok(self
            .search(aggregate, probe.slice(), limits, counters, location)?
            .ok()
            .map(|at| self.order[at]))
    }

    /// Existing keys keep their original value and coordinate. For a new key,
    /// all reserves and work permits finish before any logical population grows.
    pub(super) fn insert(
        &mut self,
        aggregate: usize,
        probe: &Buffer<Option<usize>>,
        value: V,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<usize, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        computation.allowance(&self.lease, limits, location)?;
        let at = match self.search(aggregate, probe.slice(), limits, counters, location)? {
            Ok(at) => return Ok(self.order[at]),
            Err(at) => at,
        };
        ceiling(
            FormulaResource::AggregateCacheRows,
            self.rows.len() as u128 + 1,
            limits.max_aggregate_cache_rows as u128,
            location,
        )?;
        self.reserve(probe.len(), computation, limits, counters, location)?;
        counters.charge_work(
            probe.len() as u128 + (self.order.len() - at) as u128 + 3,
            limits,
            location,
        )?;
        let slot = self.rows.len();
        let key = Key {
            aggregate,
            start: self.outer.len(),
            len: probe.len(),
        };
        self.outer.extend_from_slice(probe.slice());
        self.rows.push(Row { key, value });
        self.order.insert(at, slot);
        Ok(slot)
    }

    fn reserve(
        &mut self,
        additional: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let [rows, outer, order] = self.capacities(additional, computation, limits, location)?;
        let header = self.storage_without(
            self.rows.capacity() as u128 * size_of::<Row<V>>() as u128,
            location,
        )?;
        let additional = rows - self.rows.len();
        reserve_exact(
            &mut self.rows,
            additional,
            &mut self.lease,
            header,
            Context::new(computation, limits, counters, location),
        )?;
        check_key_bytes(
            key_bytes(self.rows.capacity(), outer, order),
            limits,
            location,
        )?;
        let header = self.storage_without(
            self.outer.capacity() as u128 * size_of::<Option<usize>>() as u128,
            location,
        )?;
        let additional = outer - self.outer.len();
        reserve_exact(
            &mut self.outer,
            additional,
            &mut self.lease,
            header,
            Context::new(computation, limits, counters, location),
        )?;
        check_key_bytes(
            key_bytes(self.rows.capacity(), self.outer.capacity(), order),
            limits,
            location,
        )?;
        let header = self.storage_without(
            self.order.capacity() as u128 * size_of::<usize>() as u128,
            location,
        )?;
        let additional = order - self.order.len();
        reserve_exact(
            &mut self.order,
            additional,
            &mut self.lease,
            header,
            Context::new(computation, limits, counters, location),
        )?;
        check_key_bytes(self.key_bytes(), limits, location)
    }

    /// Admit one combined growth plan before reserving any sibling buffer.
    fn capacities(
        &self,
        additional: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        location: ProgramSite,
    ) -> Result<[usize; 3], FormulaFailure> {
        let rows = self
            .rows
            .len()
            .checked_add(1)
            .ok_or_else(|| overflow(location))?
            .max(self.rows.capacity());
        let outer = self
            .outer
            .len()
            .checked_add(additional)
            .ok_or_else(|| overflow(location))?
            .max(self.outer.capacity());
        let order = self
            .order
            .len()
            .checked_add(1)
            .ok_or_else(|| overflow(location))?
            .max(self.order.capacity());
        check_key_bytes(key_bytes(rows, outer, order), limits, location)?;
        let preferred = [
            growth_capacity(self.rows.capacity(), rows),
            growth_capacity(self.outer.capacity(), outer),
            growth_capacity(self.order.capacity(), order),
        ];
        let old = [
            (self.rows.capacity(), preferred[0], size_of::<Row<V>>()),
            (
                self.outer.capacity(),
                preferred[1],
                size_of::<Option<usize>>(),
            ),
            (self.order.capacity(), preferred[2], size_of::<usize>()),
        ]
        .into_iter()
        .filter(|&(capacity, target, _)| target > capacity)
        .map(|(capacity, _, width)| capacity as u128 * width as u128)
        .max()
        .unwrap_or(0);
        let maximum = computation.allowance(&self.lease, limits, location)?;
        // Plan all siblings together. Optional spare cells may use neither the
        // minimum key allowance nor the replacement space of a later buffer.
        // A conservative peak only selects spare capacity; exact growth retains
        // the ordinary sequential admission checks when that spare does not fit.
        Ok(
            if key_bytes(preferred[0], preferred[1], preferred[2])
                <= limits.max_aggregate_cache_key_bytes as u128
                && Self::storage_bytes(preferred) + old <= maximum as u128
            {
                preferred
            } else {
                [rows, outer, order]
            },
        )
    }

    fn storage_bytes([rows, outer, order]: [usize; 3]) -> u128 {
        size_of::<Self>() as u128
            + rows as u128 * size_of::<Row<V>>() as u128
            + outer as u128 * size_of::<Option<usize>>() as u128
            + order as u128 * size_of::<usize>() as u128
    }

    fn storage_without(
        &self,
        replaced: u128,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let bytes = Self::storage_bytes([
            self.rows.capacity(),
            self.outer.capacity(),
            self.order.capacity(),
        ]) - replaced;
        usize::try_from(bytes).map_err(|_| overflow(location))
    }

    fn search(
        &self,
        aggregate: usize,
        probe: &[Option<usize>],
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Result<usize, usize>, FormulaFailure> {
        let mut start = 0;
        let mut end = self.order.len();
        while start < end {
            counters.work(limits, location)?;
            let middle = start + (end - start) / 2;
            let key = &self.rows[self.order[middle]].key;
            let mut order = key.aggregate.cmp(&aggregate);
            if order == Ordering::Equal {
                order = compare_outer(
                    &self.outer[key.start..key.start + key.len],
                    probe,
                    limits,
                    counters,
                    location,
                )?;
            }
            match order {
                Ordering::Less => start = middle + 1,
                Ordering::Greater => end = middle,
                Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(start))
    }
}

fn compare_outer(
    left: &[Option<usize>],
    right: &[Option<usize>],
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Ordering, FormulaFailure> {
    let mut at = 0;
    while at < left.len().min(right.len()) {
        counters.work(limits, location)?;
        let order = left[at].cmp(&right[at]);
        if order != Ordering::Equal {
            return Ok(order);
        }
        at += 1;
    }
    counters.work(limits, location)?;
    Ok(left.len().cmp(&right.len()))
}

fn key_bytes(rows: usize, outer: usize, order: usize) -> u128 {
    rows as u128 * size_of::<Key>() as u128
        + outer as u128 * size_of::<Option<usize>>() as u128
        + order as u128 * size_of::<usize>() as u128
}

fn check_key_bytes(
    bytes: u128,
    limits: &FormulaLimits,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    ceiling(
        FormulaResource::AggregateCacheKeys,
        bytes,
        limits.max_aggregate_cache_key_bytes as u128,
        location,
    )
}

fn overflow(location: ProgramSite) -> FormulaFailure {
    crate::formula_binding::assignment(AssignmentError::Storage(Error::Overflow), location)
}

#[cfg(test)]
mod tests;
