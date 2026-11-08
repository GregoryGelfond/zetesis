//! Incremental exclusion by the immutable checked cardinality families.
//!
//! A lineage retains its last complete input snapshot and two sums per pair.
//! Each selected occurrence is reconstructed at that snapshot and the current
//! region. Replacing a prefix mixes bounds that all apply to the descendant;
//! their first contradiction excludes it. Prepared occurrences stay immutable.

use std::mem::size_of;

use zetesis_algebra::region::{Region, SubjectStep};
use zetesis_algebra::words::Words;
use zetesis_algebra::{Cancellation, Failure, Meter, Quota};

use crate::regions::{NarrowingScratch, NarrowingStatistics};

use super::cardinality::{Families, Prepared};
use super::runtime::{ArithmeticScratch, charge, copy_attempt, copy_values, push};
use super::snapshot::Snapshot;
use super::{ArithmeticAttempt, CardinalitySystem};

mod values;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PairSums {
    lower: u64,
    upper: u64,
}

/// Last complete input and pair sums for one prepared cardinality system.
///
/// Knowledge may be reused only with the same prepared system, atom universe
/// and a descendant region. It is not transferable merely because two systems
/// have the same theory. Its private snapshot detects width and descent faults,
/// but does not authenticate the identity of a different prepared system.
/// After a failed or refuted attempt, abandon the partial knowledge. A sibling
/// starts from its parent's complete copy, never the other sibling's progress.
#[derive(Clone, Debug)]
pub struct CardinalityKnowledge {
    snapshot: Snapshot,
    sums: Vec<PairSums>,
}

impl CardinalityKnowledge {
    fn empty() -> Self {
        Self {
            snapshot: Snapshot::default(),
            sums: Vec::new(),
        }
    }

    /// Copy initialized lineage values into an independent destination,
    /// reusing sufficient existing capacity. The source is never changed.
    /// On failure, `destination` is `None`; partial copied values are unusable.
    ///
    /// This helper polls at entry but neither refunds nor adds an exit poll.
    /// The enclosing copy attempt accounts the Meter delta and refunds its
    /// unused permits. Ordinary `Clone` has no metered-work guarantee.
    ///
    /// # Errors
    /// Preserves cancellation, allocation and the caller's typed quota error.
    pub fn copy_reserved<Q: Quota + ?Sized>(
        &self,
        destination: &mut Option<Self>,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        copy_attempt(
            self,
            destination,
            cancellation,
            work,
            |work| {
                charge(work, 7)?; // snapshot Vec/flag and sums Vec headers
                Ok(Self::empty())
            },
            |target, source, work| {
                target.snapshot.copy_from(&source.snapshot, work)?;
                copy_values(&mut target.sums, &source.sums, 2, work)
            },
        )
    }

    /// This record and actual owned capacities, excluding the immutable system
    /// and the per-worker scratch. Embedded vector headers are counted once.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.snapshot.allocated_bytes()
            + self.sums.capacity() as u128 * size_of::<PairSums>() as u128
    }
}

impl CardinalitySystem {
    /// Allocate no storage and inspect no rows. The first metered attempt
    /// initializes this system's input snapshot and prepared pair sums.
    #[must_use]
    pub fn knowledge(&self) -> CardinalityKnowledge {
        CardinalityKnowledge::empty()
    }

    /// Inspect the affected checked family occurrences once, in stored order.
    ///
    /// A complete nonrefuting pass returns `Unchanged`: updating private sums
    /// changes no region decision. Refutation returns at the first pair whose
    /// lower sum exceeds its upper sum. No optional allowance or new family is
    /// introduced, and no source recognition is repeated.
    ///
    /// `known` must belong to this exact system and a monotone ancestor of
    /// `region`; see [`CardinalityKnowledge`]. `scratch` is worker-owned and
    /// may be reused sequentially by either arithmetic subject.
    ///
    /// # Errors
    /// Preserves caller quota refusals and control/allocation stops. Detected
    /// width or descent violations are invariant failures, never refutations.
    /// Every outcome returns its performed work and refunds unused grants once.
    pub fn inspect_reserved<Q: Quota + ?Sized>(
        &self,
        region: &mut Region,
        known: &mut CardinalityKnowledge,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> ArithmeticAttempt<Failure<Q::Error>> {
        let start = work.charged();
        let result = cancellation
            .poll()
            .map_err(Failure::Stopped)
            .and_then(|()| self.inspect(region, known, &mut scratch.arithmetic, work));
        work.refund();
        ArithmeticAttempt {
            result,
            statistics: NarrowingStatistics {
                work: work.charged() - start,
                ..NarrowingStatistics::default()
            },
        }
    }

    fn inspect<Q: Quota + ?Sized>(
        &self,
        region: &Region,
        known: &mut CardinalityKnowledge,
        scratch: &mut ArithmeticScratch,
        work: &mut Meter<'_, Q>,
    ) -> Result<SubjectStep, Failure<Q::Error>> {
        known
            .snapshot
            .validate(region, self.owner.atom_count(), work)?;
        charge(work, 1)?;
        let Prepared::Families(families) = &self.prepared else {
            return Ok(SubjectStep::Refuted);
        };
        charge(work, 1)?;
        if !known.snapshot.initialized() {
            charge(work, 1)?; // clear the inherited vector length before initialization
            known.sums.clear();
            for pair in &families.pairs {
                push(
                    &mut known.sums,
                    PairSums {
                        lower: pair.lower_sum,
                        upper: pair.upper_sum,
                    },
                    2,
                    work,
                )?;
            }
        }
        charge(work, 1)?;
        if let Some(index) = &families.index {
            known.snapshot.select(region, index, scratch, work)?;
            if self.inspect_selected(families, region, known, &scratch.touched, work)? {
                return Ok(SubjectStep::Refuted);
            }
        } else {
            known.snapshot.prepare(region.mask_words(), work)?;
        }
        known.snapshot.publish(region, work)?;
        Ok(SubjectStep::Unchanged)
    }

    fn inspect_selected<Q: Quota + ?Sized>(
        &self,
        families: &Families,
        region: &Region,
        known: &mut CardinalityKnowledge,
        touched: &[u64],
        work: &mut Meter<'_, Q>,
    ) -> Result<bool, Failure<Q::Error>> {
        for (word_index, word) in touched.iter().enumerate() {
            charge(work, 1)?;
            let mut bits = Words::<u64>::bits(*word);
            while let Some(bit) = bits.next_with(|| charge(work, 5))? {
                let position = word_index * 64 + bit;
                let occurrence = &families.occurrences[position];
                charge(work, 1)?;
                let bound = &self.bounds[occurrence.row];
                let old = values::effective(bound, known.snapshot.old_masks(), work)?;
                let current = values::effective(bound, region.decision_words(), work)?;
                charge(work, 1)?;
                let sums = &mut known.sums[occurrence.pair];
                if values::replace(sums, occurrence.side, old, current, work)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests;
