//! Two-polarity knowledge of the last completely inspected input region.
//!
//! System identity and ancestor correspondence are caller preconditions.
//! The metered ancestry predicate supplies only the region relation. The
//! adapter maps detected shape or lineage faults to `InvalidProgram`; same-system
//! authentication remains a caller obligation.

use std::mem::size_of;

use zetesis_algebra::{
    Failure, Meter, Quota, Stop,
    region::Region,
    words::{self, Words},
};

use super::{
    parity::Index,
    runtime::{self, ArithmeticScratch},
};

#[derive(Clone, Debug, Default)]
pub(super) struct Snapshot {
    words: Words<u64>,
    initialized: bool,
}

impl Snapshot {
    pub(super) fn initialized(&self) -> bool {
        self.initialized
    }

    /// Owned payload only; a containing knowledge record counts the header.
    pub(super) fn allocated_bytes(&self) -> u128 {
        self.words.capacity() as u128 * size_of::<u64>() as u128
    }

    /// Preserve a prepared snapshot; lazily create all-open prior masks.
    pub(super) fn prepare<Q: Quota + ?Sized>(
        &mut self,
        atom_words: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        runtime::charge(work, 2)?;
        let length = atom_words
            .checked_mul(2)
            .ok_or(Failure::Stopped(Stop::Allocation))?;
        if self.words.len() != length {
            assert!(
                !self.initialized,
                "a live snapshot retains its coordinate universe"
            );
            runtime::prepare_words(&mut self.words, length, work)?;
        }
        Ok(())
    }

    pub(super) fn old_masks(&self) -> (&[u64], &[u64]) {
        self.words.split_at(self.words.len() / 2)
    }

    /// Check same polarity containment through existing counted Words kernels.
    /// This does not authenticate the prepared system or map false to an error.
    pub(super) fn is_ancestor<Q: Quota + ?Sized>(
        &self,
        region: &Region,
        work: &mut Meter<'_, Q>,
    ) -> Result<bool, Failure<Q::Error>> {
        runtime::charge(work, 1)?;
        if !self.initialized {
            return Ok(true);
        }
        let (held, cut) = self.old_masks();
        let (current_held, current_cut) = region.decision_words();
        runtime::charge(work, 1)?;
        if held.len() != current_held.len() {
            return Ok(false);
        }
        for (old, current) in [(held, current_held), (cut, current_cut)] {
            let count = words::count(old, &mut |amount| runtime::charge(work, amount))?;
            let retained = runtime::counted_intersection(old, current, work)?;
            runtime::charge(work, 1)?;
            if count != retained {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Common adapter for detected width and ancestry violations.
    /// This validates neither Theory identity nor prepared-system identity.
    /// `InvalidProgram` denotes the failed invocation invariant, never a
    /// semantic refutation.
    pub(super) fn validate<Q: Quota + ?Sized>(
        &self,
        region: &Region,
        expected_atoms: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        runtime::charge(work, 1)?;
        if region.len() != expected_atoms {
            return Err(Failure::Stopped(Stop::InvalidProgram));
        }
        let ancestor = self.is_ancestor(region, work)?;
        runtime::charge(work, 1)?;
        if !ancestor {
            return Err(Failure::Stopped(Stop::InvalidProgram));
        }
        Ok(())
    }

    /// Select first-pass rows or columns touched by newly decided atoms.
    /// The caller establishes monotone descent and exact system correspondence.
    /// `open` is temporary fresh-decision scratch and is overwritten by P1 later.
    pub(super) fn select<Q: Quota + ?Sized>(
        &mut self,
        region: &Region,
        index: &Index,
        scratch: &mut ArithmeticScratch,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        self.prepare(region.mask_words(), work)?;
        runtime::charge(work, 1)?;
        let logical = index.rows.div_ceil(64);
        runtime::prepare_words(&mut scratch.touched, logical, work)?;
        runtime::charge(work, 1)?;
        if !self.initialized {
            for row in 0..index.rows {
                runtime::charge(work, 4)?; // locate (3), OR (1)
                scratch.touched.insert(row);
            }
            return Ok(());
        }
        runtime::prepare_words(&mut scratch.open, region.mask_words(), work)?;
        let (held, cut) = region.decision_words();
        let (old_held, old_cut) = self.old_masks();
        runtime::copy_words(&mut scratch.open, held, work)?;
        scratch
            .open
            .union_assign(Words::new(cut), || runtime::charge(work, 1))?;
        scratch
            .open
            .xor_assign(Words::new(old_held), || runtime::charge(work, 1))?;
        scratch
            .open
            .xor_assign(Words::new(old_cut), || runtime::charge(work, 1))?;
        words::visit_set_bits(
            &scratch.open,
            &mut (work, &mut scratch.touched),
            &mut |(work, _), amount| runtime::charge(work, amount),
            &mut |(work, touched), atom| {
                runtime::charge(work, 2)?; // column multiplication and end addition
                let start = atom * index.row_words;
                let column = &index.words[start..start + logical];
                touched.union_assign(Words::new(column), || runtime::charge(work, 1))
            },
        )
    }

    /// Publish exactly the inspected pre-batch input, never resulting decisions.
    /// On failure the caller must abandon the partially overwritten snapshot.
    pub(super) fn publish<Q: Quota + ?Sized>(
        &mut self,
        region: &Region,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let (held, cut) = region.decision_words();
        assert_eq!(
            self.words.len(),
            held.len() * 2,
            "prepare precedes publication"
        );
        let (target_held, target_cut) = self.words.split_at_mut(held.len());
        runtime::copy_words(&mut Words::new(target_held), held, work)?;
        runtime::copy_words(&mut Words::new(target_cut), cut, work)?;
        runtime::charge(work, 1)?;
        self.initialized = true;
        Ok(())
    }

    /// Copy actual initialized values and flag, reusing sufficient capacity.
    /// This helper does not poll or refund. The caller owns destination=None
    /// on failure and must never publish this target after a refused prefix.
    pub(super) fn copy_from<Q: Quota + ?Sized>(
        &mut self,
        source: &Self,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        runtime::copy_values(self.words.storage_mut(), &source.words, 1, work)?;
        runtime::charge(work, 1)?;
        self.initialized = source.initialized;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
