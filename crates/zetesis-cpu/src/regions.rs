//! Regions of a candidate space and the traversal that covers them.
//!
//! A region decides some atoms in, some out, and leaves the rest open; its
//! candidates are the sets that agree with its decisions, the `Cube` of
//! `Search.lean` indexed over the atoms of a root, as the oracle's `Cube`
//! is the same cube over atoms. The traversal walks the coverage tree of that chapter
//! with a stack: a region is narrowed by a caller-supplied narrowing, and
//! is then refuted, a leaf when every atom is decided, split on the open
//! atom its narrowing preferred or else its highest open atom, or counted
//! when its narrowing decided nothing beyond the split and the caller
//! counts such regions. The cut branch is visited before the held one, so
//! with no preference the leaves come in the order of a binary counter over
//! the atoms with atom zero as its low bit (`Cube.split_partition` and
//! `Cube.split_disjoint` of `Search.lean`). The root is always split.
//!
//! The narrowing is the caller's: on the closure route it is the program's
//! two closures under the region's gate readings, on the formula route the
//! theory's readings. The traversal only requires that a narrowing never
//! removes an accepted candidate from the region and never refutes a region
//! holding one; each visit then keeps every accepted candidate of the root
//! in exactly one region still to visit, visited as a leaf, or counted.
//!
//! Termination: a visit pops one region and pushes at most two, each with
//! one more atom decided, so the number of regions ever pushed is bounded by
//! twice the number of candidates of the root.

use crate::Stop;

/// A region of candidates: every atom held, cut or open, and, once
/// narrowed, the open atom its narrowing would split on. The region also
/// keeps the order its atoms were decided in, so a narrowing that carries
/// knowledge from the region's parent applies only the decisions made since.
#[derive(Clone, Debug)]
pub struct Region {
    decided: Vec<Option<bool>>,
    preferred: Option<usize>,
    /// The atoms decided, in the order they were decided.
    decisions: Vec<usize>,
}

/// Two regions are the same when they decide the same atoms the same way
/// and prefer the same split; the order the decisions were made in is
/// history, not identity.
impl PartialEq for Region {
    fn eq(&self, other: &Self) -> bool {
        self.decided == other.decided && self.preferred == other.preferred
    }
}
impl Eq for Region {}

impl Region {
    /// The region in which every atom is open: every candidate lies in it.
    #[must_use]
    pub fn all_open(atoms: usize) -> Self {
        Self {
            decided: vec![None; atoms],
            preferred: None,
            decisions: Vec::new(),
        }
    }
    /// The number of atoms the region decides over.
    #[must_use]
    pub fn len(&self) -> usize {
        self.decided.len()
    }
    /// Whether the region decides over no atom at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.decided.is_empty()
    }
    /// Hold an atom in every candidate; `false` when the atom is cut or
    /// outside the region.
    pub fn hold(&mut self, atom: usize) -> bool {
        self.decide(atom, true)
    }
    /// Cut an atom from every candidate; `false` when the atom is held or
    /// outside the region.
    pub fn cut(&mut self, atom: usize) -> bool {
        self.decide(atom, false)
    }
    fn decide(&mut self, atom: usize, value: bool) -> bool {
        match self.decided.get(atom) {
            Some(None) => {
                self.decided[atom] = Some(value);
                self.decisions.push(atom);
                true
            }
            Some(Some(decided)) => *decided == value,
            None => false,
        }
    }
    /// The atoms decided so far, in the order they were decided: a reader
    /// that saw the first `n` takes up at `decisions()[n..]`.
    #[must_use]
    pub fn decisions(&self) -> &[usize] {
        &self.decisions
    }
    /// The atom's decision: held, cut, or open; `None` outside the region too.
    #[must_use]
    pub fn decision(&self, atom: usize) -> Option<bool> {
        self.decided.get(atom).copied().flatten()
    }
    /// Whether every candidate holds the atom.
    #[must_use]
    pub fn is_held(&self, atom: usize) -> bool {
        self.decision(atom) == Some(true)
    }
    /// Whether no candidate holds the atom.
    #[must_use]
    pub fn is_cut(&self, atom: usize) -> bool {
        self.decision(atom) == Some(false)
    }
    /// Whether the atom is open: some candidates hold it and some do not.
    #[must_use]
    pub fn is_open(&self, atom: usize) -> bool {
        self.decided.get(atom).is_some_and(Option::is_none)
    }
    /// The highest open atom, the one a split decides when no atom is
    /// preferred.
    #[must_use]
    pub fn highest_open(&self) -> Option<usize> {
        self.decided.iter().rposition(Option::is_none)
    }
    /// Prefer an open atom for the next split, as a narrowing may after
    /// reading the region; a decided atom is not retained.
    pub fn prefer(&mut self, atom: usize) {
        self.preferred = Some(atom).filter(|&atom| self.is_open(atom));
    }
    /// The atom the next split decides: the preferred open atom, else the
    /// highest open one.
    #[must_use]
    pub fn split_atom(&self) -> Option<usize> {
        self.preferred
            .filter(|&atom| self.is_open(atom))
            .or_else(|| self.highest_open())
    }
    /// The atoms every candidate holds, ascending.
    pub fn held(&self) -> impl Iterator<Item = usize> + '_ {
        self.with_decision(Some(true))
    }
    /// The open atoms, ascending.
    pub fn open(&self) -> impl Iterator<Item = usize> + '_ {
        self.with_decision(None)
    }
    fn with_decision(&self, wanted: Option<bool>) -> impl Iterator<Item = usize> + '_ {
        self.decided
            .iter()
            .enumerate()
            .filter(move |(_, decision)| **decision == wanted)
            .map(|(atom, _)| atom)
    }
    /// The two regions an open atom splits this one into: cut, then held.
    /// Neither inherits a preference; their narrowing sets their own.
    #[must_use]
    pub fn split(&self, atom: usize) -> (Self, Self) {
        let mut cut = self.clone();
        cut.decided[atom] = Some(false);
        cut.decisions.push(atom);
        cut.preferred = None;
        let mut held = self.clone();
        held.decided[atom] = Some(true);
        held.decisions.push(atom);
        held.preferred = None;
        (cut, held)
    }
}

/// The outcome of narrowing one region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Narrowing {
    /// No accepted candidate lies in the region.
    Refuted,
    /// The fixed point, and whether the narrowing decided an atom.
    Fixed {
        /// Whether any atom was decided by the narrowing.
        changed: bool,
    },
}

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
    /// Regions visited as leaves.
    pub decided: usize,
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
            - (self.decided - before.decided)
            - (self.counted - before.counted)
    }
}

/// The coverage tree of a root region, walked depth first. Each region
/// carries the caller's state `S`, what the narrowing knows about the
/// region; a split clones it into both children, so that a child starts
/// from its parent's knowledge and the regions share nothing.
#[derive(Clone, Debug)]
pub struct Traversal<S = ()> {
    /// The regions still to visit with their state, the next on top.
    regions: Vec<(Region, S)>,
    counting: Counting,
    /// The root has not been visited yet; it is split whatever its narrowing.
    root: bool,
    /// The caller narrowed the root before the traversal began, so it is
    /// not narrowed again.
    narrowed_root: bool,
    statistics: RegionStatistics,
}

impl Traversal {
    /// A traversal of every candidate of the root, carrying nothing.
    #[must_use]
    pub fn new(root: Region, counting: Counting) -> Self {
        Self::with_state(root, counting, ())
    }
}

impl<S: Clone> Traversal<S> {
    /// A traversal of every candidate of the root, carrying `state` with the
    /// root and a clone of a region's state with each of its children.
    #[must_use]
    pub fn with_state(root: Region, counting: Counting, state: S) -> Self {
        Self {
            regions: vec![(root, state)],
            counting,
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

    /// The regions visited so far, by outcome.
    #[must_use]
    pub const fn statistics(&self) -> RegionStatistics {
        self.statistics
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
                self.statistics.decided += 1;
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
            self.regions.push((held, state.clone()));
            self.regions.push((cut, state));
        }
    }
}
