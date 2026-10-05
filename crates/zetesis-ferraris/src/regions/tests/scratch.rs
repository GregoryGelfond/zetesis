//! A walker's scratch carries nothing from one narrowing into the next,
//! however the earlier narrowing ended, and keeps its capacity for them.

use zetesis_cpu::{Cancellation, Stop};

use crate::{
    Narrower, Narrowing, NarrowingQuota, NarrowingScratch, NarrowingStatistics, Region,
    RegionLimits, Theory,
};

use super::implication_chain;

/// Outcome, receipt and region of one narrowing.
type Run = (Result<Narrowing, Stop>, Option<NarrowingStatistics>, Region);

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
    match result {
        Ok((narrowing, statistics)) => (Ok(narrowing), Some(statistics), region),
        Err(stop) => (Err(stop), None, region),
    }
}

/// The next region's narrowing with `scratch` equals a fresh scratch's.
fn assert_clean_after(theory: &Theory, narrower: &Narrower, mut scratch: NarrowingScratch) {
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
    let (narrowing, _) = narrower
        .narrow_known(
            crate::OriginalSubject::new(&theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
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
    assert_eq!(stopped.unwrap_err(), Stop::WorkLimit);
    assert_clean_after(&theory, &narrower, scratch);
}

/// Grants single permits, then reports cancellation.
struct CancelledAfter(u64);

impl NarrowingQuota for CancelledAfter {
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
    let attempt = narrower.narrow_known_reserved(
        crate::OriginalSubject::new(&theory, None),
        &mut Region::all_open(theory.atom_count()),
        &mut narrower.knowledge(),
        &mut scratch,
        &Cancellation::default(),
        &mut CancelledAfter(5),
    );
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
        .unwrap();
    assert_eq!(knowledge.retained_bytes(), before);
    // The worklists the narrowing grew stay with the walker's scratch.
    assert!(scratch.nodes.capacity() > 0);
}
