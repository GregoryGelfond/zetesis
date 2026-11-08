//! The real candidate adapter refuses cumulative overflow before publishing a
//! validated source consequence or asking for its search-work permit.

use std::cell::Cell;

use super::*;
use crate::budget::Quota;
use crate::region_filter::{self, RegionFeasibility, Source, tests::injection};
use crate::streamed_constraints::ConstraintConsequence;
use crate::subject::Pass;
use zetesis_themelios::ProgramSite;

#[derive(Clone, Copy, Debug)]
enum Reply {
    Fail,
    Hold,
    Cut,
}

#[derive(Debug)]
struct SourceFixture {
    theory: Theory,
    reply: Reply,
}

struct SourceWorker<'a>(&'a SourceFixture);

impl injection::Source for SourceFixture {
    fn worker(
        &self,
        theory: &Theory,
        _: &Cancellation,
    ) -> Result<Box<dyn injection::Worker + '_>, Incomplete> {
        assert!(theory.same_instance(&self.theory));
        Ok(Box::new(SourceWorker(self)))
    }
}

impl injection::Worker for SourceWorker<'_> {
    fn check(
        &mut self,
        _: &Theory,
        _: &Region,
        _: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        panic!("candidate narrowing must request a consequence")
    }

    fn consequence(
        &mut self,
        theory: &Theory,
        region: &Region,
        _: &Cancellation,
        pass: Pass,
    ) -> Result<ConstraintConsequence, Incomplete> {
        assert!(theory.same_instance(&self.0.theory));
        assert_eq!(pass, Pass::First);
        assert!(region.is_open(0));
        let site = ProgramSite::program();
        match self.0.reply {
            Reply::Fail => Err(Incomplete::RegionFilter),
            Reply::Hold => Ok(ConstraintConsequence::Hold { atom: 0, site }),
            Reply::Cut => Ok(ConstraintConsequence::Cut { atom: 0, site }),
        }
    }
}

#[derive(Default)]
struct RecordingQuota {
    work_calls: Cell<usize>,
    decision_calls: Cell<usize>,
}

impl Quota for RecordingQuota {
    type Reservation<'call> = <LocalQuota as Quota>::Reservation<'call>;

    fn work(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        self.work_calls.set(self.work_calls.get() + 1);
        LocalQuota.work(spent, ceiling)
    }

    fn decision(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        self.decision_calls.set(self.decision_calls.get() + 1);
        LocalQuota.decision(spent, ceiling)
    }

    fn charge(&self, spent: u64, ceiling: u64, amount: u64) -> Result<(), Incomplete> {
        LocalQuota.charge(spent, ceiling, amount)
    }

    fn reserve_up_to(&self, spent: u64, ceiling: u64, wanted: u64) -> Result<u64, Incomplete> {
        LocalQuota.reserve_up_to(spent, ceiling, wanted)
    }

    fn reserve_kernel(
        &mut self,
        spent: u64,
        ceiling: u64,
        wanted: u64,
    ) -> Result<Self::Reservation<'_>, Incomplete> {
        LocalQuota.reserve_kernel(spent, ceiling, wanted)
    }

    fn refund(&self, unspent: u64) {
        LocalQuota.refund(unspent);
    }
}

struct Attempt {
    result: Result<Narrowing, Incomplete>,
    counts: RegionCounts,
    budget: SearchStatistics,
    region: Region,
    work_calls: usize,
    decision_calls: usize,
    source: crate::RegionFilterStatistics,
}

fn run(theory: &Theory, reply: Reply, mut counts: RegionCounts) -> Attempt {
    let cancellation = Cancellation::default();
    let narrower = Narrower::new(theory);
    let mut entry = Entry::new(Region::all_open(1), CandidateKnowledge::default());
    let mut budget = Budget {
        quota: RecordingQuota::default(),
        limits: SearchLimits {
            max_work: u64::MAX,
            max_decisions: u64::MAX,
        },
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let filter = region_filter::Filter::new(Source::Fixture(Arc::new(SourceFixture {
        theory: theory.clone(),
        reply,
    })));
    let mut worker = None;
    let mut timings = None;
    let mut check = region_filter::Check {
        filter: &filter,
        worker: &mut worker,
        timings: &mut timings,
    };
    let result = narrow(
        CandidateReading {
            original: OriginalSubject::new(theory, None),
            narrower: &narrower,
            arithmetic: None,
        },
        &Conditions::<(Theory, Narrower)>::default(),
        &mut entry,
        &mut NarrowingScratch::default(),
        &mut budget,
        &mut counts,
        Some(&mut check),
    );
    Attempt {
        result,
        counts,
        budget: budget.statistics,
        region: entry.region.clone(),
        work_calls: budget.quota.work_calls.get(),
        decision_calls: budget.quota.decision_calls.get(),
        source: filter.statistics(),
    }
}

#[test]
fn late_cumulative_overflow_precedes_the_publication_debit_and_mutation() {
    let theory = Theory::new(
        1,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::falsum(),
                Node::implies(0, 1),
                Node::or_pair([0, 2]),
            ],
            vec![],
        )
        .unwrap(),
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap();
    // Measure the exact eager prefix through this same candidate route. The
    // intentional source failure stops before a propagation debit or ranking.
    let prefix = run(&theory, Reply::Fail, RegionCounts::default());
    assert_eq!(prefix.result, Err(Incomplete::RegionFilter));
    assert!(prefix.counts.work > 0);
    assert_eq!(prefix.counts.held, 0);
    assert_eq!(prefix.counts.cut, 0);
    assert_eq!(prefix.work_calls, 0);
    assert_eq!(prefix.decision_calls, 0);
    assert_eq!(prefix.source.failed, 1);
    for (reply, seed) in [
        (
            Reply::Hold,
            RegionCounts {
                work: u64::MAX - prefix.counts.work,
                ..Default::default()
            },
        ),
        (
            Reply::Hold,
            RegionCounts {
                held: u64::MAX,
                ..Default::default()
            },
        ),
        (
            Reply::Cut,
            RegionCounts {
                cut: u64::MAX,
                ..Default::default()
            },
        ),
        (
            Reply::Hold,
            RegionCounts {
                propagations: u64::MAX - prefix.counts.propagations,
                ..Default::default()
            },
        ),
    ] {
        let mut expected = seed;
        expected.add(prefix.counts).unwrap();
        let attempt = run(&theory, reply, seed);
        assert_eq!(attempt.result, Err(Incomplete::CounterOverflow));
        assert_eq!(attempt.counts, expected);
        assert_eq!(attempt.budget, prefix.budget);
        assert_eq!(
            attempt.work_calls, 0,
            "no source publication permit requested"
        );
        assert_eq!(attempt.decision_calls, 0, "a consequence is not a split");
        assert_eq!(attempt.region, prefix.region);
        assert!(attempt.region.is_open(0));
        assert_eq!(attempt.source.preparations, 1);
        assert_eq!(attempt.source.checks, 1);
        assert_eq!(attempt.source.refuted, 0);
        assert_eq!(
            attempt.source.failed, 0,
            "source validation succeeded before receipt refusal"
        );
        assert!(!attempt.source.overflowed);
    }
}
