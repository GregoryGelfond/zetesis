//! Source closure lifetime is independent of arithmetic restart progress.

use std::sync::{Arc, Mutex};

use super::*;
use crate::region_filter::{self, Source, tests::injection};
use crate::{RegionFeasibility, StreamedConstraints};
use zetesis_themelios::{ConstraintAllowance, ConstraintCheckLimits, ProgramSite};

#[derive(Debug)]
struct TraceSource {
    theory: Theory,
    force: Option<usize>,
    passes: Mutex<Vec<Pass>>,
}

struct TraceWorker<'a>(&'a TraceSource);

impl injection::Source for TraceSource {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn injection::Worker + '_>, Incomplete> {
        cancellation.poll()?;
        assert!(theory.same_instance(&self.theory));
        Ok(Box::new(TraceWorker(self)))
    }
}

impl injection::Worker for TraceWorker<'_> {
    fn check(
        &mut self,
        _: &Theory,
        _: &Region,
        _: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        panic!("the subject requests a consequence")
    }

    fn consequence(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        pass: Pass,
    ) -> Result<ConstraintConsequence, Incomplete> {
        cancellation.poll()?;
        assert!(theory.same_instance(&self.0.theory));
        self.0.passes.lock().unwrap().push(pass);
        Ok(match self.0.force.filter(|&atom| region.is_open(atom)) {
            Some(atom) => ConstraintConsequence::Hold {
                atom,
                site: ProgramSite::program(),
            },
            None => ConstraintConsequence::NoConsequence,
        })
    }
}

fn source(theory: &Theory, force: Option<usize>) -> (Arc<TraceSource>, region_filter::Filter) {
    let trace = Arc::new(TraceSource {
        theory: theory.clone(),
        force,
        passes: Mutex::new(Vec::new()),
    });
    let filter = region_filter::Filter::new(Source::Fixture(trace.clone()));
    (trace, filter)
}

/// Isolate actual checked arithmetic and source adapters in the common driver.
/// Empty eager input makes the first parity change observable in this test.
fn close_late(
    reading: &CandidateReading<'_>,
    check: &mut Check<'_, '_>,
    region: &mut Region,
    budget: &mut Budget<'_>,
) -> (
    Result<zetesis_algebra::region::Narrowing, Incomplete>,
    RegionCounts,
) {
    let mut subjects = late(reading, Some(check));
    let mut counts = RegionCounts::default();
    let result = crate::subject::joint(
        std::iter::empty,
        &mut subjects,
        region,
        &mut CandidateKnowledge::default(),
        &mut NarrowingScratch::default(),
        budget,
        &mut counts,
    );
    (result, counts)
}

#[test]
fn arithmetic_progress_does_not_start_a_source_allowance() {
    let (theory, prepared) = fixture();
    let narrower = Narrower::new(&theory);
    let reading = CandidateReading {
        original: OriginalSubject::new(&theory, None),
        narrower: &narrower,
        arithmetic: Some(&prepared),
    };
    let (trace, filter) = source(&theory, None);
    let mut worker = None;
    let mut timings = None;
    let cancellation = Cancellation::default();
    // Retain the same source worker across two different candidate closures.
    for _ in 0..2 {
        let mut check = Check {
            filter: &filter,
            worker: &mut worker,
            timings: &mut timings,
        };
        let mut region = Region::all_open(2);
        assert!(region.hold(0));
        let mut budget = budget(&cancellation, u64::MAX);
        let (result, counts) = close_late(&reading, &mut check, &mut region, &mut budget);
        assert_eq!(
            result,
            Ok(zetesis_algebra::region::Narrowing::Fixed { changed: true })
        );
        assert!(
            region.is_cut(1),
            "the real parity adapter made the earlier late change"
        );
        assert_eq!(counts.cut, 1);
        assert_eq!(counts.held, 0);
        assert_eq!(counts.work, budget.statistics.work);
        assert_eq!(budget.statistics.decisions, 0);
    }
    assert_eq!(*trace.passes.lock().unwrap(), [Pass::First, Pass::First]);
    assert_eq!(filter.statistics().preparations, 1);
    assert_eq!(filter.statistics().checks, 2);
    assert_eq!(filter.statistics().failed, 0);
}

#[test]
fn later_arithmetic_progress_does_not_renew_the_source_allowance() {
    let (theory, prepared) = fixture();
    let narrower = Narrower::new(&theory);
    let reading = CandidateReading {
        original: OriginalSubject::new(&theory, None),
        narrower: &narrower,
        arithmetic: Some(&prepared),
    };
    let (trace, filter) = source(&theory, Some(0));
    let mut worker = None;
    let mut timings = None;
    let mut check = Check {
        filter: &filter,
        worker: &mut worker,
        timings: &mut timings,
    };
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let mut region = Region::all_open(2);
    let (result, counts) = close_late(&reading, &mut check, &mut region, &mut budget);
    assert_eq!(
        result,
        Ok(zetesis_algebra::region::Narrowing::Fixed { changed: true })
    );
    assert!(region.is_held(0) && region.is_cut(1));
    assert_eq!((counts.held, counts.cut), (1, 1));
    assert_eq!(*trace.passes.lock().unwrap(), [Pass::First, Pass::Continue]);
    assert_eq!(filter.statistics().preparations, 1);
    assert_eq!(filter.statistics().checks, 2);
    assert_eq!(counts.work, budget.statistics.work);
    assert_eq!(budget.statistics.decisions, 0);
}

#[test]
fn a_native_source_starts_even_when_global_progress_has_continued() {
    let owner = zetesis_reference_support::hybrid("{p(1..4)}. :-p(X),p(Y),X<Y,not not p(X).");
    assert_eq!(owner.streamed_templates(), 1);
    let theory = owner.core_theory();
    let narrower = Narrower::new(theory);
    let reading = CandidateReading {
        original: OriginalSubject::new(theory, None),
        narrower: &narrower,
        arithmetic: None,
    };
    let source = Arc::new(StreamedConstraints::new(
        owner.core().clone(),
        ConstraintAllowance::new(ConstraintCheckLimits::default()),
    ));
    let filter = region_filter::Filter::new(Source::Streamed(source.clone()));
    let mut worker = None;
    let mut timings = None;
    let cancellation = Cancellation::default();
    for _ in 0..2 {
        let mut check = Check {
            filter: &filter,
            worker: &mut worker,
            timings: &mut timings,
        };
        let mut subjects = late(&reading, Some(&mut check));
        let mut budget = budget(&cancellation, u64::MAX);
        let counts = RegionCounts::default();
        let attempt = subjects[2].as_mut().unwrap().narrow(
            &mut Region::all_open(theory.atom_count()),
            &mut CandidateKnowledge::default(),
            &mut NarrowingScratch::default(),
            Pass::Continue,
            &mut SubjectContext {
                budget: &mut budget,
                counts: &counts,
            },
        );
        assert!(
            attempt.result.is_ok(),
            "the native checker must not report NoActiveRegion"
        );
        assert!(source.take_failure().is_none());
        assert_eq!(budget.statistics.decisions, 0);
    }
    assert_eq!(filter.statistics().preparations, 1);
    assert_eq!(filter.statistics().checks, 2);
    assert_eq!(filter.statistics().failed, 0);
}

#[test]
fn cancellation_before_source_entry_keeps_its_original_failure_boundary() {
    let (theory, _) = fixture();
    let narrower = Narrower::new(&theory);
    let reading = CandidateReading {
        original: OriginalSubject::new(&theory, None),
        narrower: &narrower,
        arithmetic: None,
    };
    let (trace, filter) = source(&theory, Some(0));
    let mut worker = None;
    let mut timings = None;
    let mut check = Check {
        filter: &filter,
        worker: &mut worker,
        timings: &mut timings,
    };
    let mut subjects = late(&reading, Some(&mut check));
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut budget = budget(&cancellation, 0);
    let mut region = Region::all_open(2);
    let counts = RegionCounts::default();
    let attempt = subjects[2].as_mut().unwrap().narrow(
        &mut region,
        &mut CandidateKnowledge::default(),
        &mut NarrowingScratch::default(),
        Pass::Continue,
        &mut SubjectContext {
            budget: &mut budget,
            counts: &counts,
        },
    );
    assert_eq!(attempt.result, Err(Incomplete::Cancelled));
    assert_eq!(attempt.counts, RegionCounts::default());
    assert_eq!(budget.statistics, SearchStatistics::default());
    assert_eq!(region, Region::all_open(2));
    assert!(trace.passes.lock().unwrap().is_empty());
    assert!(worker.is_none());
    assert_eq!(filter.statistics().preparations, 1);
    assert_eq!(filter.statistics().checks, 0);
    assert_eq!(filter.statistics().failed, 1);
}
