//! A sparse canonical-to-local equality translation beside semantic ordering.
//!
//! Published entries are exactly the append dictionary's representatives. The
//! bound source supplies their vocabulary; no map entry retains a scope or
//! payload. Hash operations admit one fixed-key container operation, not each
//! opaque internal probe. Semantic ordering remains the foreign-input fallback.

use std::collections::HashMap;

use crate::catalog::{CatalogRead, ReadError, TermRef, storage::TermId};

use super::{Failure, Resource, Work, ceiling};

#[derive(Default)]
pub(super) struct Append {
    pub order: Runs,
    pub identities: Inverse,
}

/// The catalog's value order, built in bulk only on demand.
///
/// Insertion never orders a value: the canonical inverse (`Inverse`) already
/// decides equality for every value this owner can name. Ordered lookup is
/// needed only for a value the read cannot name. Such a lookup searches the
/// sorted runs and scans the unindexed suffix `indexed..dictionary.len()`,
/// charging each comparison, and marks the order as demanded. The next
/// insertion then sorts that suffix as one batch and merges it into the runs
/// (binary-counter levels). The batch sort and each merge use fixed block
/// boundaries, so their comparisons — and their charged work — are identical
/// whether the blocks run on one worker or many.
#[derive(Default)]
pub(super) struct Runs {
    pub levels: Vec<Vec<u32>>,
    pub indexed: usize,
    pub demanded: std::sync::atomic::AtomicBool,
}

impl Runs {
    pub(super) fn clear(&mut self) {
        self.levels.clear();
        self.indexed = 0;
        *self.demanded.get_mut() = false;
    }

    /// Retained capacity of the run headers and their identities.
    pub(super) fn bytes(&self) -> usize {
        self.levels.capacity() * size_of::<Vec<u32>>()
            + self
                .levels
                .iter()
                .map(|run| run.capacity() * size_of::<u32>())
                .sum::<usize>()
    }

    /// `compare(id)` orders the query against value `id`. Runs are searched
    /// by bisection, then the unindexed suffix up to `len` linearly.
    pub(super) fn search<E>(
        &self,
        len: usize,
        mut compare: impl FnMut(usize) -> Result<std::cmp::Ordering, E>,
    ) -> Result<Option<usize>, E> {
        for run in &self.levels {
            let (mut start, mut end) = (0, run.len());
            while start < end {
                let middle = start + (end - start) / 2;
                let id = run[middle] as usize;
                match compare(id)? {
                    std::cmp::Ordering::Less => end = middle,
                    std::cmp::Ordering::Equal => return Ok(Some(id)),
                    std::cmp::Ordering::Greater => start = middle + 1,
                }
            }
        }
        if self.indexed < len {
            self.demanded
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        for id in self.indexed..len {
            if compare(id)?.is_eq() {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }
}

/// Stable bottom-up merge sort of `ids` by a fallible order, with fixed
/// block boundaries (width 1, 2, 4, ...): the comparisons performed depend
/// only on the input, so independent blocks of one width may run on separate
/// workers with the same charged work.
pub(super) fn sort_ids<E>(
    ids: &mut Vec<u32>,
    scratch: &mut Vec<u32>,
    compare: &mut impl FnMut(u32, u32) -> Result<std::cmp::Ordering, E>,
) -> Result<(), E> {
    let len = ids.len();
    let mut width = 1;
    while width < len {
        scratch.clear();
        let mut start = 0;
        while start < len {
            let middle = (start + width).min(len);
            let end = (start + 2 * width).min(len);
            merge_into(&ids[start..middle], &ids[middle..end], scratch, compare)?;
            start = end;
        }
        std::mem::swap(ids, scratch);
        width *= 2;
    }
    Ok(())
}

/// Append the stable merge of two sorted runs to `out`.
pub(super) fn merge_into<E>(
    left: &[u32],
    right: &[u32],
    out: &mut Vec<u32>,
    compare: &mut impl FnMut(u32, u32) -> Result<std::cmp::Ordering, E>,
) -> Result<(), E> {
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if compare(right[j], left[i])?.is_lt() {
            out.push(right[j]);
            j += 1;
        } else {
            out.push(left[i]);
            i += 1;
        }
    }
    out.extend_from_slice(&left[i..]);
    out.extend_from_slice(&right[j..]);
    Ok(())
}

#[derive(Default)]
pub(super) struct Inverse {
    entries: HashMap<TermId, u32>,
}

pub(super) enum Probe {
    /// Semantic lookup remains available when this read cannot name the term.
    Unavailable(ReadError),
    Local {
        term: TermId,
        equality: Option<u32>,
    },
}

impl Inverse {
    pub(super) fn probe_with<E>(
        &self,
        read: CatalogRead<'_>,
        value: TermRef<'_>,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Probe, E> {
        before()?;
        let term = match read.selected_term(value) {
            Ok(term) => term,
            Err(error) => return Ok(Probe::Unavailable(error)),
        };
        before()?;
        Ok(Probe::Local {
            term,
            equality: self.entries.get(&term).copied(),
        })
    }

    /// Addressable key/value capacity, not opaque bucket/control allocation.
    pub(super) fn bytes(&self) -> usize {
        self.entries.capacity() * size_of::<(TermId, u32)>()
    }

    pub(super) fn reserve(&mut self, additional: usize, work: &mut Work) -> Result<(), Failure> {
        let needed = self
            .entries
            .len()
            .checked_add(additional)
            .ok_or(Failure::Overflow)?;
        if needed <= self.entries.capacity() {
            return Ok(());
        }
        let previous = self.bytes();
        let proposed = needed.max(self.entries.capacity().saturating_mul(2));
        ceiling(
            Resource::Bytes,
            work.live as u128 + proposed as u128 * size_of::<(TermId, u32)>() as u128,
            work.limits.max_bytes as u128,
        )?;
        // Relocation and the container reservation precede any publication.
        work.tick(self.entries.len() as u128 + 1)?;
        self.entries
            .try_reserve(proposed - self.entries.len())
            .map_err(|_| Failure::Allocation)?;
        work.replacement(previous, self.bytes())
    }

    /// All entries and writes were reserved/admitted by the surrounding tuple
    /// transaction. Standard fixed-ID hash/equality has no caller code or error.
    pub(super) fn publish(&mut self, term: TermId, equality: u32) {
        let previous = self.entries.insert(term, equality);
        debug_assert!(
            previous.is_none(),
            "each dictionary identity publishes once"
        );
    }

    /// Reset visits retained hash storage even when its logical extent is small.
    pub(super) fn clear_work(&self) -> u128 {
        self.entries.capacity() as u128
    }

    /// The complete reset is admitted before any owner extent changes.
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod runs_tests {
    use super::{Runs, merge_into, sort_ids};
    use std::sync::atomic::Ordering::Relaxed;

    fn sorted(values: &[i32]) -> (Vec<u32>, usize) {
        let mut ids: Vec<u32> = (0..u32::try_from(values.len()).unwrap()).collect();
        let mut scratch = Vec::new();
        let mut comparisons = 0;
        sort_ids(&mut ids, &mut scratch, &mut |l: u32, r: u32| {
            comparisons += 1;
            Ok::<_, ()>(values[l as usize].cmp(&values[r as usize]))
        })
        .unwrap();
        (ids, comparisons)
    }

    #[test]
    fn batch_sort_is_stable_and_bounded() {
        let values: Vec<i32> = (0..1000).map(|i| (i * 7919) % 257).collect();
        let (ids, comparisons) = sorted(&values);
        let mut expected: Vec<u32> = (0..1000).collect();
        expected.sort_by_key(|&id| (values[id as usize], id));
        assert_eq!(ids, expected);
        // At most n * ceil(log2 n) comparisons, independent of worker count.
        assert!(comparisons <= 1000 * 10, "{comparisons}");
    }

    #[test]
    fn merge_is_stable() {
        let values = [1, 3, 3, 5, 2, 3, 6];
        let mut out = Vec::new();
        merge_into(
            &[0, 1, 2, 3],
            &[4, 5, 6],
            &mut out,
            &mut |l: u32, r: u32| Ok::<_, ()>(values[l as usize].cmp(&values[r as usize])),
        )
        .unwrap();
        assert_eq!(out, [0, 4, 1, 2, 5, 3, 6]);
    }

    #[test]
    fn search_reads_runs_then_the_unindexed_suffix_and_demands_order() {
        let values = [10, 20, 30, 40, 5];
        let runs = Runs {
            levels: vec![vec![0, 1, 2]],
            indexed: 3,
            demanded: std::sync::atomic::AtomicBool::default(),
        };
        let find = |query: i32| {
            runs.search(values.len(), |id| Ok::<_, ()>(query.cmp(&values[id])))
                .unwrap()
        };
        assert_eq!(find(20), Some(1));
        assert!(
            !runs.demanded.load(Relaxed),
            "an indexed hit demands nothing"
        );
        assert_eq!(find(5), Some(4));
        assert!(runs.demanded.load(Relaxed));
        assert_eq!(find(7), None);
    }
}
