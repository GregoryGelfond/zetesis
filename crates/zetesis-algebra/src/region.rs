//! Regions of a candidate space and the outcome of narrowing them.
//!
//! A region decides some atoms in, some out, and leaves the rest open. Its
//! candidates are the sets agreeing with those decisions, the `Region` of
//! `Search.lean` indexed over a root's atoms. [`crate::walk::Walk`] covers
//! the region by applying the caller's narrowing and splitting open atoms.

use std::mem::size_of;

use crate::{Stop, words::Words};

/// A region of candidates: every atom held, cut or open. Its decisions are two
/// atom bitmasks, one for held and one for cut; a narrowing that carries
/// knowledge from the region's parent applies only the decisions the parent
/// had not yet seen, by diffing this region's decided mask against a snapshot.
#[derive(Debug)]
pub struct Region {
    /// Bit set iff the atom is held. Disjoint from `cut` word for word, and
    /// bits at `[atoms, 64*words)` are always zero and are not atoms.
    held: Words<u64, Box<[u64]>>,
    /// Bit set iff the atom is cut.
    cut: Words<u64, Box<[u64]>>,
    /// The atom universe; each mask is `words(atoms)` words long.
    atoms: usize,
}

impl Clone for Region {
    fn clone(&self) -> Self {
        Self {
            held: self.held.clone(),
            cut: self.cut.clone(),
            atoms: self.atoms,
        }
    }

    /// Reset a region in its existing mask buffers when the word counts
    /// match. Frozen probes reuse one such region for successive patterns.
    fn clone_from(&mut self, source: &Self) {
        self.held.clone_from(&source.held);
        self.cut.clone_from(&source.cut);
        self.atoms = source.atoms;
    }
}

/// The number of 64-bit words a mask over `atoms` atoms needs.
const fn words(atoms: usize) -> usize {
    atoms.div_ceil(64)
}

/// Two regions are the same when they decide the same atoms the same way over
/// the same universe. Decision order and a walk's split preference are history
/// and policy, not the interval's identity.
impl PartialEq for Region {
    fn eq(&self, other: &Self) -> bool {
        self.atoms == other.atoms && self.held == other.held && self.cut == other.cut
    }
}
impl Eq for Region {}

impl Region {
    /// Header and owned vector capacities in bytes, including unused capacity.
    /// Excludes allocator bookkeeping; this is not a process-memory estimate.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + (self.held.len() + self.cut.len()) as u128 * size_of::<u64>() as u128
    }

    /// The region in which every atom is open: every candidate lies in it.
    #[must_use]
    pub fn all_open(atoms: usize) -> Self {
        Self::with_masks(
            atoms,
            &mut (),
            |(), length, _| Ok::<_, std::convert::Infallible>(vec![0; length].into_boxed_slice()),
            |(), _| Ok(()),
        )
        .unwrap_or_else(|never| match never {})
    }

    /// The all-open region, with fallible reservation of its two masks.
    /// No partially initialized region escapes an allocation refusal.
    ///
    /// # Errors
    /// Returns [`Stop::Allocation`] if either mask cannot be reserved.
    pub fn try_all_open(atoms: usize) -> Result<Self, Stop> {
        Self::try_all_open_with(atoms, &mut (), &mut |(), _| Ok(()), &mut |(), _| Ok(()))
    }

    /// Construct an all-open region with fallible work and storage admission.
    ///
    /// `before` admits each logical word operation before it executes. Each
    /// mask initializes a three-word vector header, two words per appended zero
    /// (the value and length), and its two-word boxed-slice header. The final
    /// atom-count field costs one. A complete construction therefore costs
    /// `11 + 4 * atoms.div_ceil(64)`; a refusal retains only its executed prefix.
    /// Allocator internals are outside this logical operation count.
    ///
    /// `admit` observes this constructor's total named storage, excluding any
    /// caller baseline: the output record, completed mask payloads, and the
    /// current vector header and requested or actual capacity. It runs before
    /// each reservation, after actual capacity is known, and before publishing
    /// the final region. No partial region escapes either callback's refusal.
    ///
    /// # Errors
    /// Preserves either callback's error. Failed mask reservation returns
    /// [`Stop::Allocation`] converted into `E`.
    pub fn try_all_open_with<C, E: From<Stop>>(
        atoms: usize,
        context: &mut C,
        before: &mut impl FnMut(&mut C, u64) -> Result<(), E>,
        admit: &mut impl FnMut(&mut C, u128) -> Result<(), E>,
    ) -> Result<Self, E> {
        Self::with_masks(
            atoms,
            &mut (context, before, admit),
            |(context, before, admit), length, held_bytes| {
                let headers = size_of::<Self>() as u128 + size_of::<Vec<u64>>() as u128;
                let requested = length as u128 * size_of::<u64>() as u128;
                admit(context, headers + held_bytes + requested)?;
                before(context, 3)?;
                let mut mask = Vec::new();
                mask.try_reserve_exact(length)
                    .map_err(|_| E::from(Stop::Allocation))?;
                let actual = mask.capacity() as u128 * size_of::<u64>() as u128;
                admit(context, headers + held_bytes + actual)?;
                for _ in 0..length {
                    before(context, 2)?;
                    mask.push(0);
                }
                before(context, 2)?;
                Ok(mask.into_boxed_slice())
            },
            |(context, before, admit), retained| {
                admit(context, retained)?;
                before(context, 1)
            },
        )
    }

    fn with_masks<C, E>(
        atoms: usize,
        context: &mut C,
        mut allocate: impl FnMut(&mut C, usize, u128) -> Result<Box<[u64]>, E>,
        mut publish: impl FnMut(&mut C, u128) -> Result<(), E>,
    ) -> Result<Self, E> {
        let held = allocate(context, words(atoms), 0)?;
        let held_bytes = held.len() as u128 * size_of::<u64>() as u128;
        let cut = allocate(context, words(atoms), held_bytes)?;
        let retained =
            size_of::<Self>() as u128 + held_bytes + cut.len() as u128 * size_of::<u64>() as u128;
        publish(context, retained)?;
        Ok(Self {
            held: held.into(),
            cut: cut.into(),
            atoms,
        })
    }
    /// The number of atoms the region decides over.
    #[must_use]
    pub fn len(&self) -> usize {
        self.atoms
    }
    /// Whether the region decides over no atom at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.atoms == 0
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
        if atom >= self.atoms {
            return false;
        }
        let (word, bit) = Words::<u64>::locate(atom);
        if self.held[word] & bit != 0 {
            return value;
        }
        if self.cut[word] & bit != 0 {
            return !value;
        }
        if value {
            self.held[word] |= bit;
        } else {
            self.cut[word] |= bit;
        }
        true
    }
    /// The atom's decision: held, cut, or open; `None` outside the region too.
    #[must_use]
    pub fn decision(&self, atom: usize) -> Option<bool> {
        if atom >= self.atoms {
            return None;
        }
        let (word, bit) = Words::<u64>::locate(atom);
        if self.held[word] & bit != 0 {
            Some(true)
        } else if self.cut[word] & bit != 0 {
            Some(false)
        } else {
            None
        }
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
        atom < self.atoms && {
            let (word, bit) = Words::<u64>::locate(atom);
            (self.held[word] | self.cut[word]) & bit == 0
        }
    }
    /// The highest open atom, the one a split decides when no atom is
    /// preferred.
    #[must_use]
    pub fn highest_open(&self) -> Option<usize> {
        for word in (0..self.mask_words()).rev() {
            let open = self.open_word(word, u64::MAX);
            if open != 0 {
                return Some(word * 64 + Words::<u64>::last_bit(open));
            }
        }
        None
    }
    /// The atoms every candidate holds, ascending.
    pub fn held(&self) -> impl Iterator<Item = usize> + '_ {
        self.held.positions().filter(|&atom| atom < self.atoms)
    }
    /// The open atoms, ascending.
    pub fn open(&self) -> impl Iterator<Item = usize> + '_ {
        self.open_words(std::iter::repeat(u64::MAX))
    }
    /// The number of 64-bit words in each of the region's atom masks:
    /// `self.len().div_ceil(64)`.
    #[must_use]
    pub fn mask_words(&self) -> usize {
        self.held.len()
    }
    /// The open atoms of mask word `word` that `selected` selects, as bit
    /// `atom % 64` for atom `atom` of that word; the bits past the last atom
    /// are clear. Constant time, for scans that read a word at a time.
    ///
    /// # Panics
    /// Panics if `word` is not below [`Self::mask_words`].
    #[must_use]
    pub fn open_word(&self, word: usize, selected: u64) -> u64 {
        let open = !(self.held[word] | self.cut[word]) & selected;
        // The unused high bits of the top partial word are not atoms, and
        // would otherwise read as open.
        let valid = self.atoms - word * 64;
        if valid < 64 {
            open & ((1u64 << valid) - 1)
        } else {
            open
        }
    }
    /// Open atoms selected by a borrowed bitmask, ascending. Missing words
    /// select nothing; excess words and unused tail bits are ignored. Bit
    /// `atom % 64` in word `atom / 64` selects that atom.
    ///
    /// Visits at most `min(selected.len(), self.len().div_ceil(64))` words
    /// plus the returned atoms, with constant space and no allocation.
    pub fn open_in<'a>(&'a self, selected: &'a [u64]) -> impl Iterator<Item = usize> + 'a {
        self.open_words(selected.iter().copied())
    }
    fn open_words<'a>(
        &'a self,
        selected: impl Iterator<Item = u64> + 'a,
    ) -> impl Iterator<Item = usize> + 'a {
        (0..self.mask_words())
            .zip(selected)
            .flat_map(move |(word, selected)| {
                let base = word * 64;
                Words::<u64>::bits(self.open_word(word, selected)).map(move |bit| base + bit)
            })
    }
    /// The atoms this region decides that are not set in `seen`, each paired
    /// with whether it is held, ascending. `seen` is a decided-mask snapshot
    /// over the same atoms; words missing from `seen` count as unseen. Reports
    /// only real atoms, never a tail bit.
    pub fn decided_since<'a>(
        &'a self,
        seen: &'a [u64],
    ) -> impl Iterator<Item = (usize, bool)> + 'a {
        (0..self.held.len()).flat_map(move |word| {
            let seen_word = seen.get(word).copied().unwrap_or(0);
            let fresh = (self.held[word] | self.cut[word]) & !seen_word;
            let held = self.held[word];
            let base = word * 64;
            Words::<u64>::bits(fresh)
                .map(move |bit| (base + bit, held & (1u64 << bit) != 0))
                .filter(move |&(atom, _)| atom < self.atoms)
        })
    }
    /// Copy this region's decided mask (`held | cut`) into the available prefix
    /// of `dst`. A complete snapshot needs `self.len().div_ceil(64)` words.
    /// A shorter destination receives only its prefix; any excess destination
    /// words remain unchanged. An empty destination is valid. Missing snapshot
    /// words count as unseen when passed to [`Self::decided_since`].
    pub fn snapshot_decided(&self, dst: &mut [u64]) {
        for (word, (&held, &cut)) in dst.iter_mut().zip(self.held.iter().zip(self.cut.iter())) {
            *word = held | cut;
        }
    }
    /// Every decided atom with whether it is held, ascending.
    pub fn decided(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
        self.decided_since(&[])
    }
    /// Borrow the held and cut masks, in that order, without copying.
    /// Both slices contain `self.len().div_ceil(64)` words. Bit `atom % 64`
    /// in word `atom / 64` represents that atom. The masks are disjoint;
    /// unused high bits of the last word are zero.
    ///
    /// Keeping both polarities permits exact comparison with an earlier region.
    /// Their union alone cannot distinguish removing or reversing a decision.
    /// The slices remain borrowed from this region; retaining a snapshot requires
    /// caller-owned storage and its own resource accounting.
    #[must_use]
    pub fn decision_words(&self) -> (&[u64], &[u64]) {
        (&self.held, &self.cut)
    }
    /// The two regions an open atom splits this one into: cut, then held.
    ///
    /// # Panics
    /// Panics unless `atom` is one of the region's open atoms.
    #[must_use]
    pub fn split(mut self, atom: usize) -> (Self, Self) {
        // A region is split on one of its OPEN atoms: an out-of-range or
        // already-decided atom fails fast, since deciding it would either touch
        // a non-atom bit or leave a narrowing's knowledge out of step with the
        // region's decision (a reader's seen-mask already covers a decided atom).
        assert!(
            self.is_open(atom),
            "a region is split on one of its open atoms"
        );
        let (word, bit) = Words::<u64>::locate(atom);
        // Clone once for the held child; reuse this region as the cut child
        // rather than cloning it a second time.
        let mut held = self.clone();
        held.held[word] |= bit;
        self.cut[word] |= bit;
        (self, held)
    }
}

/// A region with its walk policy and caller-owned lineage state.
///
/// Preference is not part of the region's interval identity. It is accepted
/// only for an open atom, checked again after narrowing, and reset in both
/// children. Pending entries retain this policy when handed to another schedule.
#[derive(Clone, Debug)]
pub struct Entry<S = ()> {
    /// The interval this entry still represents.
    pub region: Region,
    /// Knowledge or other state carried along this entry's lineage.
    pub state: S,
    preferred: Option<usize>,
}

impl<S> Entry<S> {
    /// Start an entry with no preference, using the highest open atom by default.
    #[must_use]
    pub const fn new(region: Region, state: S) -> Self {
        Self {
            region,
            state,
            preferred: None,
        }
    }

    /// Borrow the semantic interval and lineage state without discarding policy.
    pub fn parts_mut(&mut self) -> (&mut Region, &mut S) {
        (&mut self.region, &mut self.state)
    }

    /// Consume a terminal entry, releasing its no-longer-needed walk policy.
    /// Pending entries should be transferred as entries instead.
    #[must_use]
    pub fn into_parts(self) -> (Region, S) {
        (self.region, self.state)
    }

    /// Prefer an open atom for the next split; a decided atom is not retained.
    pub fn prefer(&mut self, atom: usize) {
        self.preferred = Some(atom).filter(|&atom| self.region.is_open(atom));
    }

    /// The preferred atom if still open, otherwise the highest open atom.
    #[must_use]
    pub fn split_atom(&self) -> Option<usize> {
        self.preferred
            .filter(|&atom| self.region.is_open(atom))
            .or_else(|| self.region.highest_open())
    }

    /// Split an admitted open atom, carrying each returned state in its child.
    ///
    /// The callback returns the cut child's state followed by the held child's.
    /// It lets each schedule preserve its existing cloning or allocation-reuse
    /// policy. To retain the parent on a pending-capacity refusal, the schedule
    /// admits that capacity before this consuming operation. This method does
    /// not reserve queue storage. Both children start without a preference.
    ///
    /// # Panics
    /// Panics unless `atom` is one of the region's open atoms.
    #[must_use]
    pub fn split_with(self, atom: usize, states: impl FnOnce(S) -> (S, S)) -> (Self, Self) {
        let (cut, held) = self.region.split(atom);
        let (cut_state, held_state) = states(self.state);
        (Self::new(cut, cut_state), Self::new(held, held_state))
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

/// Progress from one application of a region subject, not joint agreement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubjectStep {
    /// The applicable inspection completed without a new region decision.
    Unchanged,
    /// At least one sound decision was applied; further closure may be needed.
    Changed,
    /// The region contains no member of the subject's class.
    Refuted,
}

impl SubjectStep {
    /// Convert a subject whose complete local closure has been established.
    ///
    /// The caller must establish that closure separately: `Changed` alone is
    /// not evidence of a fixed point. Partial late passes must not use this
    /// conversion; their joint driver first re-establishes eager agreement.
    #[must_use]
    pub fn narrowing(self) -> Narrowing {
        match self {
            Self::Unchanged => Narrowing::Fixed { changed: false },
            Self::Changed => Narrowing::Fixed { changed: true },
            Self::Refuted => Narrowing::Refuted,
        }
    }
}

#[cfg(test)]
mod tests;
