//! A scalar walk covering a region of candidates.
//!
//! A region decides some atoms in, some out, and leaves the rest open; its
//! candidates are the sets that agree with its decisions, the `Cube` of
//! `Search.lean` indexed over the atoms of a root. The traversal walks the
//! coverage tree of that chapter
//! with a stack: a region is narrowed by a caller-supplied narrowing, and
//! is then refuted, a leaf when every atom is decided, split on the open
//! atom its narrowing preferred or else its highest open atom, or counted
//! when its narrowing decided nothing beyond the split and the caller
//! counts such regions. By default the cut branch is visited before the held
//! one; [`Walk::with_held_first`] reverses that preference. With the default
//! branch order and no atom preference, leaves follow a binary counter over
//! the atoms with atom zero as its low bit (`Cube.split_partition` and
//! `Cube.split_disjoint` of `Search.lean`). The root is always split.
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

use crate::{
    Stop,
    region::{Narrowing, Region},
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
    regions: Vec<(Region, S)>,
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
        Self {
            regions: vec![(root, state)],
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
    pub fn with_narrowed_root(root: Region, counting: Counting, state: S) -> Self {
        Self {
            narrowed_root: true,
            ..Self::with_state(root, counting, state)
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
    /// among them, as a stop keeps it. The traversal is left with none, so its
    /// next visit ends it, and its statistics keep what it visited.
    pub fn surrender(&mut self) -> Vec<(Region, S)> {
        std::mem::take(&mut self.regions)
    }

    /// Narrow regions until one is a leaf or is counted; `None` once every
    /// region has been visited. The narrowing's failure type is the caller's;
    /// a failed reservation for the split is reported as `Stop::Allocation`.
    ///
    /// # Errors
    /// A stopped narrowing stops the traversal with its region kept on the
    /// stack, so a later call narrows the same region again.
    pub fn next<E: From<Stop>>(
        &mut self,
        mut narrow: impl FnMut(&mut Region, &mut S) -> Result<Narrowing, E>,
    ) -> Result<Option<Visit<S>>, E> {
        loop {
            let Some((mut region, mut state)) = self.regions.pop() else {
                return Ok(None);
            };
            let changed = if std::mem::take(&mut self.narrowed_root) {
                true
            } else {
                match narrow(&mut region, &mut state) {
                    Ok(Narrowing::Refuted) => {
                        self.statistics.regions += 1;
                        self.statistics.refuted += 1;
                        self.root = false;
                        continue;
                    }
                    Ok(Narrowing::Fixed { changed }) => changed,
                    Err(stop) => {
                        self.regions.push((region, state));
                        return Err(stop);
                    }
                }
            };
            self.statistics.regions += 1;
            let root = std::mem::replace(&mut self.root, false);
            let Some(atom) = region.split_atom() else {
                self.statistics.leaves += 1;
                return Ok(Some(Visit::Leaf(region, state)));
            };
            if self.counting == Counting::Unchanged && !root && !changed {
                self.statistics.counted += 1;
                return Ok(Some(Visit::Counted(region, state)));
            }
            let (cut, held) = region.split(atom);
            self.regions
                .try_reserve(2)
                .map_err(|_| E::from(Stop::Allocation))?;
            let (first, second) = if self.held_first {
                (held, cut)
            } else {
                (cut, held)
            };
            self.regions.push((second, state.clone()));
            self.regions.push((first, state));
        }
    }
}
