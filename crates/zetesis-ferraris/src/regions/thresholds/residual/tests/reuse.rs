//! Frozen narrowing discards original-only notifications before its first read.

use super::*;
use crate::{Knowledge, NarrowingAttempt};
use zetesis_algebra::{Countermodels, Failure, Quota};

/// The original interval has a longer weighted prefix neighbour, so both
/// requirement and residual notifications can remain after an actual refusal.
struct Fixture {
    theory: Theory,
    index: Narrower,
    truth: Vec<bool>,
}

impl Fixture {
    fn new() -> Self {
        let base = interval(5, &[(0, 6), (1, 10), (2, 15), (3, 15)], [16, 18]);
        let mut nodes = FormulaNodes::new(base.parts().try_clone().unwrap());
        nodes
            .push_threshold_family(
                &[0, 1, 2, 3, 4],
                &[6, 10, 15, 15, 1],
                &[16],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let theory = Theory::new(
            5,
            nodes.into_parts(),
            base.roots().to_vec(),
            AdmissionLimits::default(),
        )
        .unwrap();
        let index = Narrower::new(&theory);
        assert!(index.thresholds.has_relations());
        assert!(index.thresholds.has_residuals());
        let interpretation = Interpretation::new(&theory, [0, 1]).unwrap();
        let mut workspace = EvaluationWorkspace::default();
        let evaluated = workspace
            .evaluate(
                &interpretation,
                EvaluationLimits::default(),
                &Cancellation::default(),
            )
            .result
            .unwrap();
        let truth = (0..theory.nodes().len())
            .map(|node| evaluated.node_truth(node).unwrap())
            .collect();
        Self {
            theory,
            index,
            truth,
        }
    }

    fn original(
        &self,
        scratch: &mut NarrowingScratch,
        limit: u64,
        failure: Stop,
    ) -> NarrowingAttempt<Failure<Stop>> {
        self.index.narrow_known_reserved(
            OriginalSubject::new(&self.theory, None),
            &mut Region::all_open(5),
            &mut self.index.knowledge(),
            scratch,
            &Cancellation::default(),
            &mut Meter::reserved(&mut Allowance {
                remaining: limit,
                failure,
            }),
        )
    }

    fn queued_limit(&self) -> u64 {
        let completed = self.original(&mut NarrowingScratch::default(), u64::MAX, Stop::WorkLimit);
        assert!(matches!(completed.result, Ok(Narrowing::Fixed { .. })));
        for limit in 0..completed.statistics.work {
            let mut scratch = NarrowingScratch::default();
            let attempt = self.original(&mut scratch, limit, Stop::WorkLimit);
            assert_eq!(attempt.result, Err(Failure::Refused(Stop::WorkLimit)));
            assert_eq!(attempt.statistics.work, limit);
            if !scratch.common.threshold_rows.is_empty() && !scratch.common.residual_rows.is_empty()
            {
                return limit;
            }
        }
        panic!("the original refusal must retain both real notification queues");
    }

    fn refused(&self, limit: u64, failure: Stop) -> NarrowingScratch {
        let mut scratch = NarrowingScratch::default();
        let attempt = self.original(&mut scratch, limit, failure);
        assert_eq!(attempt.result, Err(Failure::Refused(failure)));
        assert_eq!(attempt.statistics.work, limit);
        assert!(!scratch.common.threshold_rows.is_empty());
        assert!(!scratch.common.residual_rows.is_empty());
        scratch
    }

    fn frozen(
        &self,
        scratch: &mut NarrowingScratch,
        limit: u64,
        batch: u64,
        failure: Stop,
        cancellation: &Cancellation,
    ) -> Run {
        let mut quota = Trace {
            remaining: limit,
            batch,
            failure,
            cancellation,
            events: Vec::new(),
        };
        let mut region = Region::all_open(5);
        let mut knowledge = self.index.frozen_knowledge();
        let attempt = self.index.narrow_frozen_known_reserved(
            FrozenSubject::new(&self.theory, &self.truth),
            &mut region,
            &mut knowledge,
            scratch,
            cancellation,
            &mut Meter::reserved(&mut quota),
        );
        Run {
            attempt,
            region,
            knowledge,
            events: quota.events,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Request(u64),
    Poll(Result<(), Stop>),
    Grant(Result<u64, Stop>),
    Refund(u64),
}

struct Trace<'a> {
    remaining: u64,
    batch: u64,
    failure: Stop,
    cancellation: &'a Cancellation,
    events: Vec<Event>,
}

impl Quota for Trace<'_> {
    type Error = Stop;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        self.events.push(Event::Request(wanted));
        let polled = self.cancellation.poll();
        self.events.push(Event::Poll(polled));
        polled?;
        let granted = wanted.min(self.batch).min(self.remaining);
        let result = if granted == 0 {
            Err(self.failure)
        } else {
            self.remaining -= granted;
            Ok(granted)
        };
        self.events.push(Event::Grant(result));
        result
    }

    fn refund(&mut self, unspent: u64) {
        self.events.push(Event::Refund(unspent));
        self.remaining += unspent;
    }
}

struct Run {
    attempt: NarrowingAttempt<Failure<Stop>>,
    region: Region,
    knowledge: Knowledge<Countermodels>,
    events: Vec<Event>,
}

/// An independent grant/refund ledger for the unchanged per-read contract.
fn expected_events(limit: u64, batch: u64, needed: u64, failure: Stop) -> Vec<Event> {
    let performed = limit.min(needed);
    let mut remaining = limit;
    let mut admitted = 0;
    let mut events = Vec::new();
    while admitted < performed {
        let grant = remaining.min(batch).min(crate::regions::NARROWING_BATCH);
        assert!(grant > 0);
        events.extend([
            Event::Request(crate::regions::NARROWING_BATCH),
            Event::Poll(Ok(())),
            Event::Grant(Ok(grant)),
        ]);
        admitted += grant;
        remaining -= grant;
    }
    if limit < needed {
        events.extend([
            Event::Request(crate::regions::NARROWING_BATCH),
            Event::Poll(Ok(())),
            Event::Grant(Err(failure)),
        ]);
    }
    events.push(Event::Refund(admitted - performed));
    events
}

fn same_run(actual: &Run, expected: &Run) {
    assert_eq!(actual.attempt.result, expected.attempt.result);
    assert_eq!(actual.attempt.statistics, expected.attempt.statistics);
    assert_eq!(actual.attempt.preferred, expected.attempt.preferred);
    assert_eq!(actual.region, expected.region);
    assert_eq!(actual.events, expected.events);
    let (Width::Compact16(actual), Width::Compact16(expected)) =
        (&actual.knowledge.width, &expected.knowledge.width)
    else {
        panic!("the five-atom fixture uses compact counters");
    };
    assert_eq!(actual.masks.slices(), expected.masks.slices());
    for (left, right) in [
        (&actual.neutral_operands, &expected.neutral_operands),
        (&actual.unknown, &expected.unknown),
    ] {
        assert_eq!(left.len(), right.len());
        for index in 0..left.len() {
            assert_eq!(left.get(index), right.get(index));
        }
    }
    assert_eq!(actual.thresholds, expected.thresholds);
    assert_eq!(actual.seeded, expected.seeded);
    assert_eq!(actual.founded_closed, expected.founded_closed);
    assert_eq!(actual.support_progress, expected.support_progress);
}

fn original_capacities(scratch: &NarrowingScratch) -> [usize; 6] {
    [
        scratch.common.threshold_rows.capacity(),
        scratch.common.pending_thresholds.capacity(),
        scratch.common.residual_rows.capacity(),
        scratch.common.pending_residuals.capacity(),
        scratch.common.residual_suffix.capacity(),
        scratch.common.possible_sums.capacity(),
    ]
}

fn queues_cleared(scratch: &NarrowingScratch) {
    assert!(scratch.common.threshold_rows.is_empty());
    assert!(scratch.common.residual_rows.is_empty());
    for pending in [
        &scratch.common.pending_thresholds,
        &scratch.common.pending_residuals,
    ] {
        assert!(pending.iter().all(|&word| word == 0));
    }
}

#[test]
fn refused_original_queues_do_not_change_a_frozen_prefix() {
    let fixture = Fixture::new();
    let queued = fixture.queued_limit();
    let cancellation = Cancellation::default();
    let completed = fixture.frozen(
        &mut NarrowingScratch::default(),
        u64::MAX,
        1,
        Stop::WorkLimit,
        &cancellation,
    );
    assert!(matches!(
        completed.attempt.result,
        Ok(Narrowing::Fixed { .. })
    ));
    let needed = completed.attempt.statistics.work;
    assert!(needed > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for batch in [1, 7, crate::regions::NARROWING_BATCH] {
            for limit in 0..=needed + 7 {
                let mut reused = fixture.refused(queued, failure);
                let retained = original_capacities(&reused);
                let mut fresh = NarrowingScratch::default();
                let actual = fixture.frozen(&mut reused, limit, batch, failure, &cancellation);
                let expected = fixture.frozen(&mut fresh, limit, batch, failure, &cancellation);
                same_run(&actual, &expected);
                assert_eq!(actual.attempt.statistics.work, limit.min(needed));
                assert_eq!(
                    actual.attempt.result,
                    if limit < needed {
                        Err(Failure::Refused(failure))
                    } else {
                        completed.attempt.result
                    }
                );
                assert_eq!(
                    actual.events,
                    expected_events(limit, batch, needed, failure)
                );
                assert_eq!(original_capacities(&reused), retained);
                assert_eq!(original_capacities(&fresh), [0; 6]);
                queues_cleared(&reused);
                queues_cleared(&fresh);
                assert_eq!(reused.common.nodes, fresh.common.nodes);
                assert_eq!(reused.common.learned, fresh.common.learned);
                assert_eq!(reused.common.heads, fresh.common.heads);
            }
        }
    }
}

#[test]
fn frozen_entry_cancellation_precedes_original_queue_clearing() {
    let fixture = Fixture::new();
    let mut scratch = fixture.refused(fixture.queued_limit(), Stop::WorkLimit);
    let before = (
        scratch.common.threshold_rows.clone(),
        scratch.common.residual_rows.clone(),
        scratch.common.pending_thresholds.to_vec(),
        scratch.common.pending_residuals.to_vec(),
    );
    let retained = scratch.retained_bytes();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let actual = fixture.frozen(&mut scratch, 0, 1, Stop::Deadline, &cancellation);
    let fresh = fixture.frozen(
        &mut NarrowingScratch::default(),
        0,
        1,
        Stop::Deadline,
        &cancellation,
    );
    same_run(&actual, &fresh);
    assert_eq!(
        actual.attempt.result,
        Err(Failure::Stopped(Stop::Cancelled))
    );
    assert_eq!(actual.attempt.statistics, NarrowingStatistics::default());
    assert_eq!(actual.events, [Event::Refund(0)]);
    assert_eq!(actual.region, Region::all_open(5));
    assert_eq!(scratch.retained_bytes(), retained);
    assert_eq!(
        (
            scratch.common.threshold_rows,
            scratch.common.residual_rows,
            scratch.common.pending_thresholds.to_vec(),
            scratch.common.pending_residuals.to_vec(),
        ),
        before
    );
}
