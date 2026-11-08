//! A walker's scratch carries nothing from one narrowing into the next,
//! however the earlier narrowing ended, and keeps its capacity for them.

use zetesis_algebra::{Cancellation, Stop};

use crate::{
    Narrower, Narrowing, NarrowingScratch, NarrowingStatistics, Region, RegionLimits, Theory,
};

use super::implication_chain;

/// Outcome, receipt and region of one narrowing.
type Run = (
    Result<Narrowing, Stop>,
    Option<NarrowingStatistics>,
    Region,
    Option<usize>,
);

/// Narrow the all-open region of `theory` from fresh knowledge.
fn narrow_open(theory: &Theory, narrower: &Narrower, scratch: &mut NarrowingScratch) -> Run {
    let mut knowledge = narrower.knowledge();
    let mut region = Region::all_open(theory.atom_count());
    let result = narrower.narrow_known(
        crate::OriginalSubject::new(theory, None),
        &mut region,
        &mut knowledge,
        scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    match result.result {
        Ok(narrowing) => (
            Ok(narrowing),
            Some(result.statistics),
            region,
            result.preferred,
        ),
        Err(stop) => (Err(stop), None, region, result.preferred),
    }
}

/// Whether a narrowing left entries in the scratch.
fn holds_entries(scratch: &NarrowingScratch) -> bool {
    !(scratch.common.learned.is_empty()
        && scratch.common.nodes.is_empty()
        && scratch.common.heads.is_empty())
}

/// The next region's narrowing with `scratch`, which an earlier failure left
/// holding entries, equals a fresh scratch's.
fn assert_clean_after(theory: &Theory, narrower: &Narrower, mut scratch: NarrowingScratch) {
    assert!(holds_entries(&scratch), "the failure left nothing to leak");
    let reused = narrow_open(theory, narrower, &mut scratch);
    let fresh = narrow_open(theory, narrower, &mut NarrowingScratch::default());
    assert_eq!(reused, fresh);
}

#[test]
fn a_refuted_narrowing_leaves_nothing_for_the_next_region() {
    let theory = implication_chain(40);
    let narrower = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    // The held root forces the last atom, which the region cuts.
    let mut region = Region::all_open(theory.atom_count());
    region.cut(39);
    let narrowing = narrower
        .narrow_known(
            crate::OriginalSubject::new(&theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    assert_eq!(narrowing, Narrowing::Refuted);
    assert_clean_after(&theory, &narrower, scratch);
}

#[test]
fn a_work_limit_stop_leaves_nothing_for_the_next_region() {
    let theory = implication_chain(40);
    let narrower = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    let stopped = narrower.narrow_known(
        crate::OriginalSubject::new(&theory, None),
        &mut Region::all_open(theory.atom_count()),
        &mut narrower.knowledge(),
        &mut scratch,
        RegionLimits { max_work: 5 },
        &Cancellation::default(),
    );
    assert_eq!(stopped.result.unwrap_err(), Stop::WorkLimit);
    assert_clean_after(&theory, &narrower, scratch);
}

/// Grants single permits, then reports cancellation.
struct CancelledAfter(u64);

impl zetesis_algebra::Quota for CancelledAfter {
    type Error = Stop;
    fn reserve(&mut self, _wanted: u64) -> Result<u64, Stop> {
        if self.0 == 0 {
            return Err(Stop::Cancelled);
        }
        self.0 -= 1;
        Ok(1)
    }
    fn refund(&mut self, _unspent: u64) {}
}

#[test]
fn a_cancelled_narrowing_leaves_nothing_for_the_next_region() {
    let theory = implication_chain(40);
    let narrower = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    let attempt = crate::test_support::stop_narrowing(&narrower.narrow_known_reserved(
        crate::OriginalSubject::new(&theory, None),
        &mut Region::all_open(theory.atom_count()),
        &mut narrower.knowledge(),
        &mut scratch,
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut CancelledAfter(5)),
    ));
    assert_eq!(attempt.result.unwrap_err(), Stop::Cancelled);
    assert_clean_after(&theory, &narrower, scratch);
}

#[test]
fn knowledge_retains_no_worklist_capacity() {
    let theory = implication_chain(40);
    let narrower = Narrower::new(&theory);
    let mut knowledge = narrower.knowledge();
    let before = knowledge.retained_bytes();
    let mut scratch = NarrowingScratch::default();
    narrower
        .narrow_known(
            crate::OriginalSubject::new(&theory, None),
            &mut Region::all_open(theory.atom_count()),
            &mut knowledge,
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    assert_eq!(knowledge.retained_bytes(), before);
    // The worklists the narrowing grew stay with the walker's scratch.
    assert!(scratch.common.nodes.capacity() > 0);
}

#[test]
fn pending_masks_reuse_storage_across_word_boundaries() {
    let mut scratch = NarrowingScratch::default();
    scratch.common.prepare(130);
    scratch.common.prepare_thresholds(130).unwrap();
    scratch.common.prepare_residuals(130).unwrap();
    let pointers = (
        scratch.common.pending.as_ptr(),
        scratch.common.pending_thresholds.as_ptr(),
        scratch.common.pending_residuals.as_ptr(),
    );
    let bytes = scratch.retained_bytes();
    for atom in [0, 63, 64, 129, 64] {
        scratch.common.queue_recheck(atom);
        scratch.common.queue_threshold(atom);
        scratch.common.queue_residual(atom);
    }
    assert_eq!(scratch.common.heads, [0, 63, 64, 129]);
    assert_eq!(scratch.common.threshold_rows, scratch.common.heads);
    assert_eq!(scratch.common.residual_rows, scratch.common.heads);
    assert_eq!(scratch.common.next_recheck(), Some(129));
    assert_eq!(scratch.common.next_threshold(), Some(129));
    assert_eq!(scratch.common.next_residual(), Some(129));
    scratch.common.queue_recheck(129);
    scratch.common.queue_threshold(129);
    scratch.common.queue_residual(129);
    // Resetting for a smaller subject must clear the still-queued high bits,
    // retaining the existing capacity rather than truncating their storage.
    scratch.common.prepare(1);
    assert!(scratch.common.heads.is_empty());
    assert!(scratch.common.threshold_rows.is_empty());
    assert!(scratch.common.residual_rows.is_empty());
    assert_eq!(&*scratch.common.pending, [0, 0, 0]);
    assert_eq!(&*scratch.common.pending_thresholds, [0, 0, 0]);
    assert_eq!(&*scratch.common.pending_residuals, [0, 0, 0]);
    assert_eq!(
        pointers,
        (
            scratch.common.pending.as_ptr(),
            scratch.common.pending_thresholds.as_ptr(),
            scratch.common.pending_residuals.as_ptr(),
        )
    );
    // Only the head list can grow here: row lists were fully reserved above.
    assert_eq!(
        scratch.retained_bytes(),
        bytes + scratch.common.heads.capacity() as u128 * std::mem::size_of::<usize>() as u128
    );
    scratch.common.queue_recheck(0);
    assert_eq!(scratch.common.next_recheck(), Some(0));
    assert_eq!(scratch.common.next_recheck(), None);
}
