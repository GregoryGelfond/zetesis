//! One parity inspection batch over checked original-coordinate equations.
//!
//! Inspect touched basis rows against one unchanged region, save that input,
//! then apply collected consequences in basis order. A decision made by this
//! batch remains new to the snapshot, so the next application revisits its
//! incident rows. A changed batch does not claim a local fixed point.

use std::mem::size_of;

use zetesis_algebra::{
    Cancellation, Failure, Meter, Quota,
    region::{Region, SubjectStep},
    words::{self, Words},
};

use super::super::{
    ParitySystem,
    rows::Equation,
    runtime::{self, ArithmeticAttempt, ArithmeticScratch, charge},
    snapshot::Snapshot,
};
use crate::regions::{NarrowingScratch, NarrowingStatistics};

/// The last completely inspected input of one original parity system.
///
/// Reuse requires the same system and a descendant region. Cloning is an exact
/// independent value copy; production search copies must use the charged
/// copying boundary rather than treating that copy as free work.
#[derive(Clone, Debug)]
pub struct ParityKnowledge {
    snapshot: Snapshot,
}

impl ParityKnowledge {
    fn empty() -> Self {
        Self {
            snapshot: Snapshot::default(),
        }
    }

    /// This record and its actual buffer capacity, excluding shared equations.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128 + self.snapshot.allocated_bytes()
    }

    /// Copy initialized knowledge with metered payload work and capacity reuse.
    ///
    /// Cancellation is polled before any semantic work. Any failure leaves the
    /// source unchanged and the destination unavailable. Invalidating a failed
    /// destination is attempt cleanup, not a semantic copy. This helper does
    /// not refund: its caller settles the encompassing copy attempt.
    ///
    /// # Errors
    /// Returns the original quota/control failure or typed allocation stop.
    pub fn copy_reserved<Q: Quota + ?Sized>(
        &self,
        destination: &mut Option<Self>,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        runtime::copy_attempt(
            self,
            destination,
            cancellation,
            work,
            |work| {
                charge(work, 4)?; // empty vector header3 and initialization flag1
                Ok(Self::empty())
            },
            |target, source, work| target.snapshot.copy_from(&source.snapshot, work),
        )
    }
}

impl ParitySystem {
    /// Create uninitialized knowledge without allocation or inspecting rows.
    #[must_use]
    pub fn knowledge(&self) -> ParityKnowledge {
        ParityKnowledge::empty()
    }

    /// Inspect applicable rows once and apply their consequences in row order.
    ///
    /// The region uses this system's original atom coordinates. Initialized
    /// knowledge belongs to this exact system and an ancestor of the region;
    /// the same-system requirement is a caller precondition, not an identity
    /// token checked by this operation. Detected width or lineage violations
    /// retain an internal-instantiation failure, never a semantic refutation.
    ///
    /// Cancellation is checked before mutation. The operation refunds unused
    /// permits once and reports all performed work and fresh decisions on
    /// every outcome. After a failure, abandon both region and knowledge.
    /// Worker scratch can be reused: its next invocation resets live contents.
    /// `Changed` means progress, not a fixed point of this or other subjects.
    pub fn inspect_reserved<Q: Quota + ?Sized>(
        &self,
        region: &mut Region,
        known: &mut ParityKnowledge,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> ArithmeticAttempt<Failure<Q::Error>> {
        let before = work.charged();
        let mut statistics = NarrowingStatistics::default();
        let result = cancellation.poll().map_err(Failure::from).and_then(|()| {
            known
                .snapshot
                .validate(region, self.recognized.facts.owner().atom_count(), work)?;
            inspect(
                self,
                region,
                &mut known.snapshot,
                &mut scratch.arithmetic,
                work,
                &mut statistics,
            )
        });
        work.refund();
        statistics.work = work.charged() - before;
        ArithmeticAttempt { result, statistics }
    }
}

fn inspect<Q: Quota + ?Sized>(
    system: &ParitySystem,
    region: &mut Region,
    snapshot: &mut Snapshot,
    scratch: &mut ArithmeticScratch,
    work: &mut Meter<'_, Q>,
    statistics: &mut NarrowingStatistics,
) -> Result<SubjectStep, Failure<Q::Error>> {
    charge(work, 1)?;
    scratch.consequences.clear();
    charge(work, 1)?;
    if system.prepared.root_refuted {
        return Ok(SubjectStep::Refuted);
    }
    charge(work, 1)?;
    if system.prepared.basis.is_empty() {
        return Ok(SubjectStep::Unchanged);
    }
    charge(work, 1)?;
    let index = system
        .prepared
        .index
        .as_ref()
        .expect("complete nonrefuting preparation has its index");
    snapshot.select(region, index, scratch, work)?;
    if words::first(&scratch.touched, &mut |amount| charge(work, amount))?.is_none() {
        // No row can read an open mask or collect a consequence. Retain this
        // inspected input even when its new decisions touch no parity row.
        snapshot.publish(region, work)?;
        charge(work, 1)?; // the reached unchanged result
        return Ok(SubjectStep::Unchanged);
    }
    prepare_open(region, &mut scratch.open, work)?;
    let (held, _) = region.decision_words();
    let ArithmeticScratch {
        touched,
        open,
        consequences,
    } = scratch;
    for (word, mask) in touched.iter().enumerate() {
        charge(work, 1)?;
        let mut remaining = Words::<u64>::bits(*mask);
        while let Some(bit) = remaining.next_with(|| charge(work, 3))? {
            charge(work, 2)?;
            let ordinal = word * 64 + bit;
            charge(work, 2)?;
            let source = system.prepared.basis[ordinal];
            let row = &system.recognized.equations[source];
            let consistent = collect(row, held, open, consequences, work)?;
            charge(work, 1)?;
            if !consistent {
                return Ok(SubjectStep::Refuted);
            }
        }
    }
    // This input is the basis of every row just inspected. Saving the region
    // after publication instead would lose triggers created by this batch.
    snapshot.publish(region, work)?;
    apply(consequences, region, work, statistics)
}

fn prepare_open<Q: Quota + ?Sized>(
    region: &Region,
    open: &mut Words<u64>,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    runtime::prepare_words(open, region.mask_words(), work)?;
    let (held, cut) = region.decision_words();
    for ((output, held), cut) in open.iter_mut().zip(held).zip(cut) {
        charge(work, 2)?;
        *output = !(*held | *cut);
    }
    charge(work, 2)?;
    let tail = region.len() % 64;
    if tail != 0 {
        charge(work, 3)?;
        *open
            .last_mut()
            .expect("a partial word belongs to a nonempty region") &= u64::MAX >> (64 - tail);
    }
    Ok(())
}

fn collect<Q: Quota + ?Sized>(
    row: &Equation,
    held: &[u64],
    open: &[u64],
    consequences: &mut Vec<(usize, bool)>,
    work: &mut Meter<'_, Q>,
) -> Result<bool, Failure<Q::Error>> {
    // Admitted rows have the same universe with zero 128-bit padding. Region
    // masks use the shorter 64-bit extent; do not read the extra padding word.
    let coefficients = &row.words[..held.len()];
    let held_count = runtime::counted_intersection(coefficients, held, work)?;
    let open_count = runtime::counted_intersection(coefficients, open, work)?;
    charge(work, 3)?; // remainder, Boolean conversion and zero-open test
    let odd = held_count % 2 != 0;
    if open_count == 0 {
        charge(work, 1)?;
        return Ok(odd == row.constant);
    }
    charge(work, 1)?;
    if open_count == 1 {
        let atom = first_common(coefficients, open, work)?;
        charge(work, 1)?;
        let value = row.constant ^ odd;
        runtime::push(consequences, (atom, value), 2, work)?;
    }
    Ok(true)
}

/// Read only as far as the established unique common bit. The singleton test
/// belongs to the preceding immutable intersection count, not this locator.
fn first_common<Q: Quota + ?Sized>(
    left: &[u64],
    right: &[u64],
    work: &mut Meter<'_, Q>,
) -> Result<usize, Failure<Q::Error>> {
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        charge(work, 1)?;
        let common = *left & *right;
        if let Some(bit) = words::first(&[common], &mut |amount| charge(work, amount))? {
            charge(work, 2)?;
            return Ok(index * 64 + bit);
        }
    }
    unreachable!("the unchanged intersection has exactly one bit")
}

fn apply<Q: Quota + ?Sized>(
    consequences: &[(usize, bool)],
    region: &mut Region,
    work: &mut Meter<'_, Q>,
    statistics: &mut NarrowingStatistics,
) -> Result<SubjectStep, Failure<Q::Error>> {
    charge(work, 1)?;
    let mut changed = false;
    for consequence in consequences {
        charge(work, 2)?; // the two initialized consequence fields
        let (atom, value) = *consequence;
        let decision = work.bounded(9, || {
            let decision = region.decision(atom);
            // Admitted coordinate: bound test1, locate3, held AND/test2;
            // optionally cut AND/test2; then the returned decision tag1.
            (decision, if decision == Some(true) { 7 } else { 9 })
        })?;
        if let Some(previous) = decision {
            charge(work, 1)?;
            if previous != value {
                return Ok(SubjectStep::Refuted);
            }
        } else {
            publish(region, atom, value, statistics, work)?;
            charge(work, 1)?;
            changed = true;
        }
    }
    charge(work, 1)?; // the reached final progress selection
    Ok(if changed {
        SubjectStep::Changed
    } else {
        SubjectStep::Unchanged
    })
}

fn publish<Q: Quota + ?Sized>(
    region: &mut Region,
    atom: usize,
    value: bool,
    statistics: &mut NarrowingStatistics,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    charge(work, 5)?; // counter selection, two additions and overflow tests
    let decisions = if value {
        &mut statistics.held
    } else {
        &mut statistics.cut
    };
    let propagated = statistics
        .propagations
        .checked_add(1)
        .expect("each original atom changes at most once");
    let decided = decisions
        .checked_add(1)
        .expect("a polarity count is bounded by original atoms");
    // The open-coordinate Region mutation costs 11: bounds1, locate3, two
    // AND/tests4, polarity choice1, OR1 and result tag1. The caller selects
    // hold/cut1 and writes the two already checked receipt fields2.
    charge(work, 14)?;
    let accepted = if value {
        region.hold(atom)
    } else {
        region.cut(atom)
    };
    assert!(
        accepted,
        "an admitted open coordinate accepts either decision"
    );
    statistics.propagations = propagated;
    *decisions = decided;
    Ok(())
}

#[cfg(test)]
mod tests;
