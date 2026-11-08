//! Depths of the inline, freshly rooted proper-subset walk. Completion keeps
//! one workspace per worker; its bins accumulate across that worker's queries.

use std::mem::size_of;
use zetesis_algebra::{MeterBytes, region::Narrowing, walk::RegionStatistics};

/// Measurements for one entered query. A missing deepest visit means no region
/// was visited, or its optional depth storage was unavailable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::ferraris) struct QueryDepth {
    pub(in crate::ferraris) attempted_splits: usize,
    pub(in crate::ferraris) published_splits: usize,
    pub(in crate::ferraris) deepest: Option<usize>,
}

/// The outer completion owner accounts for this header. Both vector capacities
/// are reserved before use; no read, stop or recording event allocates.
#[derive(Debug)]
pub(in crate::ferraris) struct DepthWorkspace {
    pending: Vec<usize>,
    levels: Vec<u64>,
    incomplete: bool,
}

impl DepthWorkspace {
    #[cfg(test)]
    pub(in crate::ferraris) fn new(atoms: usize, max_payload: u128) -> Option<Self> {
        Self::with_peak(atoms, max_payload, &mut 0)
    }

    /// Retain the peak of admitted vector capacities even if the second
    /// reservation fails. The ceiling excludes the caller-owned header.
    pub(in crate::ferraris) fn with_peak(
        atoms: usize,
        max_payload: u128,
        peak: &mut u128,
    ) -> Option<Self> {
        // Each split decides one open atom. Depth and the pending sibling count
        // therefore each require at most one slot per atom, plus the root.
        let slots = atoms.checked_add(1)?;
        let bin_bytes = slots as u128 * size_of::<u64>() as u128;
        MeterBytes::check(
            slots as u128 * size_of::<usize>() as u128 + bin_bytes,
            max_payload,
        )
        .ok()?;
        let mut workspace = Self {
            pending: Vec::new(),
            levels: Vec::new(),
            incomplete: false,
        };
        workspace.pending.try_reserve_exact(slots).ok()?;
        workspace.reserve_levels(slots, max_payload, peak)
    }

    fn reserve_levels(mut self, slots: usize, limit: u128, peak: &mut u128) -> Option<Self> {
        let pending = self.payload_bytes();
        MeterBytes::check(pending, limit).ok()?;
        *peak = (*peak).max(pending);
        MeterBytes::check(pending + slots as u128 * size_of::<u64>() as u128, limit).ok()?;
        self.levels.try_reserve_exact(slots).ok()?;
        let actual = self.payload_bytes();
        MeterBytes::check(actual, limit).ok()?;
        *peak = (*peak).max(actual);
        Some(self)
    }

    pub(in crate::ferraris) fn payload_bytes(&self) -> u128 {
        self.pending.capacity() as u128 * size_of::<usize>() as u128
            + self.levels.capacity() as u128 * size_of::<u64>() as u128
    }

    /// Exact visited bins accumulated so far; absent when the storage could not represent a query.
    pub(in crate::ferraris) fn levels(&self) -> Option<&[u64]> {
        (!self.incomplete).then_some(&self.levels)
    }
}

/// Statically absent on ordinary CPU checks and queries offered for publication.
pub(super) trait Trace {
    fn enter(&mut self) {}
    fn narrowed(&mut self, _result: Narrowing) {}
    fn returned(&mut self, _counts: RegionStatistics, _failed: bool) {}
}

impl Trace for () {}

pub(super) struct Recording<'a> {
    counts: &'a mut QueryDepth,
    workspace: Option<&'a mut DepthWorkspace>,
    fixed: bool,
}

impl<'a> Recording<'a> {
    pub(super) fn new(
        counts: &'a mut QueryDepth,
        mut workspace: Option<&'a mut DepthWorkspace>,
    ) -> Self {
        *counts = QueryDepth::default();
        if let Some(levels) = &mut workspace {
            levels.pending.clear();
            levels.pending.push(0);
        }
        Self {
            counts,
            workspace,
            fixed: false,
        }
    }
}

impl Trace for Recording<'_> {
    fn enter(&mut self) {
        // A leaf returns from next(), as does a refused split. Only a
        // successfully published pair can re-enter after a fixed parent.
        if self.fixed {
            if let Some(worker) = &mut self.workspace {
                let pending_space = worker.pending.len() < worker.pending.capacity();
                let parent = worker.pending.last_mut().expect("fixed parent has a depth");
                let child = *parent + 1;
                // The supplying chunk sized these arrays from its actual roots.
                // A mismatched diagnostic workspace must never grow in the walk.
                if child < worker.levels.capacity() && pending_space {
                    *parent = child;
                    worker.pending.push(child);
                } else {
                    worker.incomplete = true;
                    self.workspace = None;
                    self.counts.deepest = None;
                }
            }
            self.fixed = false;
        }
    }

    fn narrowed(&mut self, result: Narrowing) {
        self.fixed = matches!(result, Narrowing::Fixed { .. });
        if let Some(worker) = &mut self.workspace {
            let &depth = worker
                .pending
                .last()
                .expect("the narrowed region has a depth");
            self.counts.deepest = Some(self.counts.deepest.unwrap_or(0).max(depth));
            if depth == worker.levels.len() {
                worker.levels.push(1);
            } else if let Some(next) = worker.levels[depth].checked_add(1) {
                worker.levels[depth] = next;
            } else {
                worker.incomplete = true;
            }
            if !self.fixed {
                worker.pending.pop();
            }
        }
    }

    fn returned(&mut self, counts: RegionStatistics, failed: bool) {
        self.counts.attempted_splits = counts.splits_since(RegionStatistics::default());
        // An unclassified fixed parent left by a failure was counted as an
        // attempted split. Its children were never published.
        self.counts.published_splits =
            self.counts.attempted_splits - usize::from(failed && self.fixed);
        if self.fixed
            && !failed
            && let Some(worker) = &mut self.workspace
        {
            worker.pending.pop();
        }
        self.fixed = false;
    }
}

#[cfg(test)]
mod tests;
