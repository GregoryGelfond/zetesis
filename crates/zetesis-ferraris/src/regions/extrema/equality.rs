//! Exact operation identity for associative ordinal and Boolean witnesses.
//!
//! Fingerprints select preparation candidates; complete distinct-leaf equality
//! establishes each class. A class borrows existing chains and retains one
//! membership per chain, never expanded leaves or all member pairs. It keeps no
//! mutable truth: every original node retains its own counter and frozen gate.
//!
//! Retained class storage is O(C). Temporary authentication uses the existing
//! O(N+E+C) witness walker, one leaf vector and a packed membership mask. A bucket
//! of B unequal fingerprint collisions can require O(B²) exact comparisons, each
//! visiting the compared cones. These are constructor preparation costs, not
//! the logical N+occurrence indexing receipt. The constructor has no independent
//! cancellation contract; completed reads are exposed in `preparation_work`.

use std::ops::Range;

use zetesis_cpu::Stop;

use super::{Closure, Count, Narrower, Step, Work, bit, push, reserve};
use crate::regions::{flag_words, witnesses};

type Signature = (usize, usize, usize, usize);
type Entry = (Signature, usize);

#[derive(Clone, Debug, Default)]
pub(in crate::regions) struct EqualWitnesses {
    members: Vec<usize>,
    classes: Vec<Range<usize>>,
    by_chain: Vec<(usize, usize)>,
    preparation_work: u64,
}

impl EqualWitnesses {
    pub(in crate::regions) fn prepare(index: &Narrower) -> Result<Self, Stop> {
        let mut work = Work::new(u64::MAX);
        let mut result = Self::prepare_with_work(index, &mut work)?;
        result.preparation_work = work.spent;
        Ok(result)
    }

    fn prepare_with_work(index: &Narrower, work: &mut Work<'_>) -> Result<Self, Stop> {
        if index.extrema.cuts.is_empty() {
            return Ok(Self::default());
        }
        let mut scratch = Equality::default();
        let mut entries = scratch.signatures(index, work)?;
        entries.sort_unstable();
        let mut result = Self::default();
        let mut start = 0;
        while start < entries.len() {
            work.tick()?;
            let key = entries[start].0;
            let mut end = start + 1;
            while end < entries.len() {
                work.tick()?;
                if entries[end].0 != key {
                    break;
                }
                end += 1;
            }
            if end - start > 1 && has_ordinal(index, &entries[start..end], work)? {
                result.append_bucket(index, &mut entries[start..end], &mut scratch, work)?;
            }
            start = end;
        }
        result.by_chain = reserve(result.members.len())?;
        for (class, range) in result.classes.iter().enumerate() {
            for &chain in &result.members[range.clone()] {
                work.tick()?;
                result.by_chain.push((chain, class));
            }
        }
        result.by_chain.sort_unstable_by_key(|&(chain, _)| chain);
        Ok(result)
    }

    /// Partition a candidate bucket in place. The first unclassified chain is
    /// the next representative; only complete equalities move into its class.
    /// No relation is published when a comparison or reservation refuses.
    fn append_bucket(
        &mut self,
        index: &Narrower,
        entries: &mut [Entry],
        scratch: &mut Equality,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let mut start = 0;
        while start < entries.len() {
            work.tick()?;
            let representative = entries[start].1;
            let mut end = start + 1;
            for position in start + 1..entries.len() {
                work.tick()?;
                if scratch.equal(index, representative, entries[position].1, work)? {
                    work.tick()?;
                    entries.swap(position, end);
                    end += 1;
                }
            }
            if end - start > 1 {
                let first = self.members.len();
                self.members
                    .try_reserve(end - start)
                    .map_err(|_| Stop::Allocation)?;
                for &(_, chain) in &entries[start..end] {
                    work.tick()?;
                    self.members.push(chain);
                }
                // Partition swaps may disturb a later class's chain order.
                // Preserve the old sorted candidate bucket's notification order;
                // sorting remains constructor preparation, like candidate sorting.
                self.members[first..].sort_unstable();
                work.tick()?;
                push(&mut self.classes, first..self.members.len())?;
            }
            start = end;
        }
        Ok(())
    }

    pub(in crate::regions) fn retained_bytes(&self) -> u128 {
        self.members.capacity() as u128 * size_of::<usize>() as u128
            + self.classes.capacity() as u128 * size_of::<Range<usize>>() as u128
            + self.by_chain.capacity() as u128 * size_of::<(usize, usize)>() as u128
    }

    pub(in crate::regions) fn preparation_work(&self) -> u64 {
        self.preparation_work
    }

    fn members(&self, chain: usize, work: &mut Work<'_>) -> Result<&[usize], Stop> {
        let mut start = 0;
        let mut end = self.by_chain.len();
        while start < end {
            work.tick()?;
            let middle = start + (end - start) / 2;
            match self.by_chain[middle].0.cmp(&chain) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => {
                    let class = self.by_chain[middle].1;
                    return Ok(&self.members[self.classes[class].clone()]);
                }
            }
        }
        Ok(&[])
    }
}

fn has_ordinal(index: &Narrower, entries: &[Entry], work: &mut Work<'_>) -> Result<bool, Stop> {
    for &(_, chain) in entries {
        work.tick()?;
        if index.chains[chain].ordinal() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Authentication belongs to immutable preparation, not per-worker closure.
/// No expanded reference or equality mask survives publication of the classes.
#[derive(Default)]
struct Equality {
    walker: witnesses::Scratch,
    reference: Vec<usize>,
    membership: Vec<u64>,
}

impl Equality {
    fn signatures(&mut self, index: &Narrower, work: &mut Work<'_>) -> Result<Vec<Entry>, Stop> {
        let mut entries = Vec::new();
        for (chain, data) in index.chains.iter().enumerate() {
            work.tick()?;
            if !data.disjunction {
                continue;
            }
            let leaves = self.walker.read(index, chain, None, work)?;
            let mut signature = (leaves.len(), usize::MAX, 0usize, 0usize);
            for &leaf in leaves {
                work.tick()?;
                signature.1 = signature.1.min(leaf);
                // Deliberate fingerprint arithmetic, never ASP arithmetic.
                signature.2 = signature.2.wrapping_add(leaf);
                signature.3 ^= leaf;
            }
            work.tick()?;
            push(&mut entries, (signature, chain))?;
        }
        Ok(entries)
    }

    /// Distinct leaf cardinality plus complete membership establishes equality.
    /// Each set bit prepays cleanup, including a refused comparison. Cleanup
    /// changes no logical state and leaves the next authentication independent.
    fn equal(
        &mut self,
        index: &Narrower,
        left: usize,
        right: usize,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        self.reference.clear();
        let result = (|| {
            let words = flag_words(index.parents.len());
            self.membership
                .try_reserve(words.saturating_sub(self.membership.len()))
                .map_err(|_| Stop::Allocation)?;
            while self.membership.len() < words {
                work.tick()?;
                self.membership.push(0);
            }
            let leaves = self.walker.read(index, left, None, work)?;
            self.reference
                .try_reserve(leaves.len())
                .map_err(|_| Stop::Allocation)?;
            for &leaf in leaves {
                work.tick()?;
                work.tick()?;
                self.membership[leaf / 64] |= 1u64 << (leaf % 64);
                self.reference.push(leaf);
            }
            let leaves = self.walker.read(index, right, None, work)?;
            if leaves.len() != self.reference.len() {
                return Ok(false);
            }
            for &leaf in leaves {
                work.tick()?;
                if self.membership[leaf / 64] & (1u64 << (leaf % 64)) == 0 {
                    return Ok(false);
                }
            }
            Ok(true)
        })();
        for &leaf in &self.reference {
            self.membership[leaf / 64] &= !(1u64 << (leaf % 64));
        }
        result
    }
}

impl<C: Count> Closure<'_, C> {
    /// Exact class membership was established once against this admitted owner.
    /// A masked node never contributes a requirement to the common operation;
    /// every target still receives its own original-truth gate.
    pub(in crate::regions) fn equal_extremum_witnesses(
        &mut self,
        index: &Narrower,
        chain: usize,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let source = &index.chains[chain];
        if !source.disjunction || frozen.is_some_and(|truth| !truth[source.root]) {
            return Ok(Step::Unchanged);
        }
        let value = bit(self.known.sure, source.root);
        let mut step = Step::Unchanged;
        for &target in index.equal_extrema.members(chain, work)? {
            work.tick()?;
            let root = index.chains[target].root;
            if target == chain || frozen.is_some_and(|truth| !truth[root]) {
                continue;
            }
            let already = if value {
                bit(self.known.sure, root)
            } else {
                bit(self.known.never, root)
            };
            if !already {
                step = step.join(if value {
                    self.sure(root)
                } else {
                    self.never(root)
                });
                if step == Step::Contradiction {
                    break;
                }
            }
        }
        Ok(step)
    }
}

#[cfg(test)]
mod tests;
