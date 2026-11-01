//! Shared entry classification and a scalar walk covering a candidate region.
//!
//! A region decides some atoms in, some out, and leaves the rest open; its
//! candidates are the sets that agree with its decisions, the `Region` of
//! `Search.lean` indexed over the atoms of a root. The traversal walks the
//! coverage tree of that chapter
//! with a stack: a region is narrowed by a caller-supplied narrowing, and
//! is then refuted, a leaf when every atom is decided, split on the open
//! atom its narrowing preferred or else its highest open atom, or counted
//! when its narrowing decided nothing beyond the split and the caller
//! counts such regions. By default the cut branch is visited before the held
//! one; [`Walk::with_held_first`] reverses that preference. With the default
//! branch order and no atom preference, leaves follow a binary counter over
//! the atoms with atom zero as its low bit (`Region.split_partition` and
//! `Region.split_disjoint` of `Search.lean`). The root is always split.
//!
//! The narrowing is the caller's: on the formula route it is the theory's
//! readings. The traversal only requires that a narrowing never
//! removes an accepted candidate from the region and never refutes a region
//! holding one; each visit then keeps every accepted candidate of the root
//! in exactly one region still to visit, visited as a leaf, or counted.
//!
//! Termination: a visit pops one region and pushes at most two, each with
//! one more atom decided, so the number of regions ever pushed is bounded by
//! twice the number of candidates of the root.

pub mod frontier;

#[cfg(test)]
mod tests;

use crate::{
    Stop,
    region::{Entry, Narrowing, Region},
};

/// When a region is counted instead of split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counting {
    /// Every region is split until decided: the traversal visits only leaves.
    Never,
    /// A region below the root whose narrowing decided nothing is counted:
    /// its open atoms are offered to a flat counter by the caller.
    Unchanged,
}

/// The next action of an inspected, narrowed entry.
///
/// A leaf is a fully decided region, not an accepted answer set. A counted
/// region is handed to the caller's counter; it is not declared semantically
/// accepted by this classification either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The narrowing proved the region empty for its class.
    Refuted,
    /// Every atom is decided.
    Leaf,
    /// An unchanged non-root region is left to the caller's counter.
    Counted,
    /// An open atom partitions the region into cut and held children.
    Split(usize),
}

/// Inspect an entry after its caller has completed and accounted narrowing.
///
/// Refutation takes precedence over every shape of region. Otherwise a decided
/// region is a leaf, even when unchanged counting is selected. An unchanged
/// open region is counted only below the root; all other open regions split on
/// the entry's existing preference or highest-open fallback. `Counting::Never`
/// cannot return `Step::Counted`.
///
/// This is the allocation-free classification shared by the scalar, parallel,
/// slate and published-query schedules. It reads no theory, invokes no semantic
/// checker, consumes no entry, and performs no resource admission. The caller
/// retains its narrowing, receipt-publication and split-admission order. A
/// returned split atom is open in the inspected entry; mutating that interval
/// afterwards requires inspection again before consuming the split.
#[must_use]
pub fn inspect<S>(entry: &Entry<S>, narrowing: Narrowing, counting: Counting, root: bool) -> Step {
    let Narrowing::Fixed { changed } = narrowing else {
        return Step::Refuted;
    };
    let Some(atom) = entry.split_atom() else {
        return Step::Leaf;
    };
    if counting == Counting::Unchanged && !root && !changed {
        Step::Counted
    } else {
        Step::Split(atom)
    }
}

/// One region the traversal hands to the caller, with what the caller
/// carries alongside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Visit<S = ()> {
    /// Every atom is decided: the region is one candidate.
    Leaf(Region, S),
    /// The region's open atoms are the caller's to count.
    Counted(Region, S),
}

/// The regions a traversal visited, by outcome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionStatistics {
    /// Regions popped and narrowed, the root included.
    pub regions: usize,
    /// Regions whose narrowing refuted them.
    pub refuted: usize,
    /// Regions visited as leaves: every atom decided, one candidate each.
    pub leaves: usize,
    /// Regions handed to the caller to count.
    pub counted: usize,
}

impl RegionStatistics {
    /// The regions split since `before`: every region visited that was
    /// neither refuted nor a leaf nor counted.
    #[must_use]
    pub fn splits_since(self, before: Self) -> usize {
        (self.regions - before.regions)
            - (self.refuted - before.refuted)
            - (self.leaves - before.leaves)
            - (self.counted - before.counted)
    }
}

/// The coverage tree of a root region, walked depth first. Each region
/// carries the caller's state `S`, what the narrowing knows about the
/// region; a split clones it into both children, so that a child starts
/// from its parent's knowledge and the regions share nothing.
#[derive(Clone, Debug)]
pub struct Walk<S = ()> {
    /// The regions still to visit with their state, the next on top.
    regions: Vec<Entry<S>>,
    counting: Counting,
    /// Visit the held child first at future splits; queued entries keep their order.
    held_first: bool,
    /// The root has not been visited yet; it is split whatever its narrowing.
    root: bool,
    /// The caller narrowed the root before the traversal began, so it is
    /// not narrowed again.
    narrowed_root: bool,
    statistics: RegionStatistics,
}

impl Walk {
    /// A traversal of every candidate of the root, carrying nothing.
    #[must_use]
    pub fn new(root: Region, counting: Counting) -> Self {
        Self::with_state(root, counting, ())
    }
}

impl<S: Clone> Walk<S> {
    /// A traversal of every candidate of the root, carrying `state` with the
    /// root and a clone of a region's state with each of its children.
    #[must_use]
    pub fn with_state(root: Region, counting: Counting, state: S) -> Self {
        Self::with_entry(Entry::new(root, state), counting)
    }

    /// Begin with an owned pending entry, retaining its knowledge and preference.
    #[must_use]
    pub fn with_entry(root: Entry<S>, counting: Counting) -> Self {
        Self {
            regions: vec![root],
            counting,
            held_first: false,
            root: true,
            narrowed_root: false,
            statistics: self::RegionStatistics::default(),
        }
    }

    /// A traversal of every candidate of a root the caller has narrowed
    /// already: the root is visited without a narrowing and split whatever
    /// that narrowing reported, and every region below it is narrowed by
    /// the caller's narrowing.
    #[must_use]
    pub fn with_narrowed_root(root: Entry<S>, counting: Counting) -> Self {
        Self {
            narrowed_root: true,
            ..Self::with_entry(root, counting)
        }
    }

    /// Visit the held child before the cut child at subsequent splits.
    ///
    /// The default visits cut first. Both children retain the same independent
    /// copies of their parent's state and cover the same disjoint partition.
    /// This changes no narrowing, counting, work limit or failure behavior.
    /// Already queued regions keep their order; choose this before the first
    /// pull for a consistent held-first traversal. The extra state is one flag.
    #[must_use]
    pub const fn with_held_first(mut self) -> Self {
        self.held_first = true;
        self
    }

    /// The regions visited so far, by outcome.
    #[must_use]
    pub const fn statistics(&self) -> RegionStatistics {
        self.statistics
    }

    /// The regions still to visit.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.regions.len()
    }

    /// Surrender the regions still to visit with their state, the oldest
    /// first, so the last is the one the next visit would narrow. They and the
    /// regions already visited partition the root, so a walker that visits
    /// every surrendered region, by any narrowing the traversal could have
    /// used, covers the rest of the tree. A region whose narrowing stopped is
    /// among them, as is a narrowed parent whose split reservation failed.
    /// The traversal is left with none, so its next visit ends it, and its
    /// statistics keep what it visited.
    pub fn surrender(&mut self) -> Vec<Entry<S>> {
        std::mem::take(&mut self.regions)
    }

    /// Narrow regions until one is a leaf or is counted; `None` once every
    /// region has been visited. The narrowing's failure type is the caller's;
    /// a failed reservation for the split is reported as `Stop::Allocation`.
    ///
    /// # Errors
    /// A stopped narrowing stops the traversal with its region kept on the
    /// stack, so a later call narrows the same region again. A failed split
    /// reservation also keeps the narrowed parent and its state, before any
    /// child is formed. The completed narrowing remains in the visit receipt;
    /// retrying or surrendering does not refund it.
    pub fn next<E: From<Stop>>(
        &mut self,
        narrow: impl FnMut(&mut Entry<S>) -> Result<Narrowing, E>,
    ) -> Result<Option<Visit<S>>, E> {
        self.next_with_reservation(narrow, |pending, additional| {
            pending
                .try_reserve(additional)
                .map_err(|_| E::from(Stop::Allocation))
        })
    }

    // The schedule admits pending capacity before consuming a split's parent.
    // The production callback reserves real Vec capacity; keeping that boundary
    // explicit also lets the stop contract be tested without a global allocator.
    fn next_with_reservation<E: From<Stop>>(
        &mut self,
        mut narrow: impl FnMut(&mut Entry<S>) -> Result<Narrowing, E>,
        mut reserve: impl FnMut(&mut Vec<Entry<S>>, usize) -> Result<(), E>,
    ) -> Result<Option<Visit<S>>, E> {
        loop {
            let Some(mut entry) = self.regions.pop() else {
                return Ok(None);
            };
            let narrowed_root = std::mem::take(&mut self.narrowed_root);
            let narrowing = if narrowed_root {
                Narrowing::Fixed { changed: true }
            } else {
                match narrow(&mut entry) {
                    Ok(narrowing) => narrowing,
                    Err(stop) => {
                        self.regions.push(entry);
                        return Err(stop);
                    }
                }
            };
            self.statistics.regions += 1;
            let root = std::mem::replace(&mut self.root, false);
            let atom = match inspect(&entry, narrowing, self.counting, root) {
                Step::Refuted => {
                    self.statistics.refuted += 1;
                    continue;
                }
                Step::Leaf => {
                    self.statistics.leaves += 1;
                    let (region, state) = entry.into_parts();
                    return Ok(Some(Visit::Leaf(region, state)));
                }
                Step::Counted => {
                    self.statistics.counted += 1;
                    let (region, state) = entry.into_parts();
                    return Ok(Some(Visit::Counted(region, state)));
                }
                Step::Split(atom) => atom,
            };
            if let Err(stop) = reserve(&mut self.regions, 2) {
                // pop vacated one slot, so restoring its parent cannot allocate,
                // even when the reservation left the capacity unchanged.
                self.regions.push(entry);
                self.root = root;
                self.narrowed_root = narrowed_root;
                return Err(stop);
            }
            let (cut, held) = entry.split_with(atom, |state| {
                if self.held_first {
                    (state.clone(), state)
                } else {
                    let second = state.clone();
                    (state, second)
                }
            });
            let (first, second) = if self.held_first {
                (held, cut)
            } else {
                (cut, held)
            };
            self.regions.push(second);
            self.regions.push(first);
        }
    }
}
