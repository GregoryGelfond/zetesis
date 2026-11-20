//! Fixed-space aggregation of optional formula-grounding observations.

use std::cell::{Cell, RefCell};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use zetesis_themelios::{GroundingOperator, GroundingOutcome, GroundingPhase, GroundingWork};

/// Host intervals and work for all attempted occurrences of one grounding phase.
/// Counts include failed and unwinding attempts; they are not semantic verdicts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundingMeasurement {
    /// Sum of observed host intervals, or `None` on duration overflow.
    pub elapsed: Option<Duration>,
    /// Combined frontend work counts and maximum support-capacity peak.
    pub work: GroundingWork,
    outcomes: [Option<u64>; GroundingOutcome::ALL.len()],
}

impl GroundingMeasurement {
    /// Number of phase attempts ending with this outcome, or `None` on overflow.
    #[must_use]
    pub fn count(&self, outcome: GroundingOutcome) -> Option<u64> {
        self.outcomes[outcome_index(outcome)]
    }

    fn record(&mut self, elapsed: Duration, outcome: GroundingOutcome, work: &GroundingWork) {
        self.elapsed = self.elapsed.and_then(|value| value.checked_add(elapsed));
        let count = &mut self.outcomes[outcome_index(outcome)];
        *count = count.and_then(|value| value.checked_add(1));
        self.work = self.work.checked_sum(*work);
    }
}

impl Default for GroundingMeasurement {
    fn default() -> Self {
        Self {
            elapsed: Some(Duration::ZERO),
            work: GroundingWork::default(),
            outcomes: [Some(0); GroundingOutcome::ALL.len()],
        }
    }
}

/// Optional eager-formula attribution aggregated by phase in constant space.
/// Rule locations are available to custom frontend observers but are not retained
/// here. Durations include counter/timer overhead and are not GPU kernel times.
/// This view does not attribute relational eager or interleaved lazy grounding.
/// An unrequested frontend domain-analysis phase is absent. When requested by a
/// library preparation, its actual work is aggregated through the same observer;
/// a completed optional attempt need not have reached an analysis fixed point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundingTimings {
    invalid: bool,
    measurements: [Option<GroundingMeasurement>; GroundingPhase::ALL.len()],
    operators: [Option<Duration>; GroundingOperator::ALL.len()],
}

impl GroundingTimings {
    /// Recorded attempts of a phase; `None` means no completed callback pair or
    /// unavailable bookkeeping after poisoning. It is not zero work.
    #[must_use]
    pub fn get(&self, phase: GroundingPhase) -> Option<&GroundingMeasurement> {
        (!self.invalid)
            .then(|| self.measurements[phase_index(phase)].as_ref())
            .flatten()
    }
}

impl Default for GroundingTimings {
    fn default() -> Self {
        Self {
            invalid: false,
            measurements: [None; GroundingPhase::ALL.len()],
            operators: [Some(Duration::ZERO); GroundingOperator::ALL.len()],
        }
    }
}

impl GroundingTimings {
    /// Summed host intervals of one operator inside the recorded phases, on
    /// the coordinating thread, so a parallel operator counts its wall time.
    /// `None` on overflow or unavailable bookkeeping; zero when never entered.
    #[must_use]
    pub fn operator(&self, operator: GroundingOperator) -> Option<Duration> {
        (!self.invalid)
            .then(|| self.operators[operator_index(operator)])
            .flatten()
    }
}

fn operator_index(operator: GroundingOperator) -> usize {
    GroundingOperator::ALL
        .iter()
        .position(|&entry| entry == operator)
        .expect("frontend operator catalog covers its variants")
}

/// Times the operators of one observed attempt. Operators are sequential on
/// the coordinating thread; their totals are folded into the recorder at each
/// phase exit.
pub(crate) struct Operators {
    active: Cell<Option<(GroundingOperator, Instant)>>,
    totals: RefCell<[Option<Duration>; GroundingOperator::ALL.len()]>,
}

impl Operators {
    pub(crate) fn new() -> Self {
        Self {
            active: Cell::new(None),
            totals: RefCell::new([Some(Duration::ZERO); GroundingOperator::ALL.len()]),
        }
    }

    fn take(&self) -> [Option<Duration>; GroundingOperator::ALL.len()] {
        self.totals
            .replace([Some(Duration::ZERO); GroundingOperator::ALL.len()])
    }
}

impl zetesis_themelios::OperatorObserver for Operators {
    fn operator_enter(&self, operator: GroundingOperator) {
        let previous = self.active.replace(Some((operator, Instant::now())));
        assert!(
            previous.is_none(),
            "frontend grounding operators must not nest"
        );
    }

    fn operator_exit(&self, operator: GroundingOperator) {
        let (entered, started) = self.active.take().expect("operator exit follows its entry");
        assert_eq!(entered, operator, "operator exit identifies its entry");
        let total = &mut self.totals.borrow_mut()[operator_index(operator)];
        *total = total.and_then(|value| value.checked_add(started.elapsed()));
    }
}

fn phase_index(phase: GroundingPhase) -> usize {
    GroundingPhase::ALL
        .iter()
        .position(|&entry| entry == phase)
        .expect("frontend phase catalog covers its variants")
}

fn outcome_index(outcome: GroundingOutcome) -> usize {
    GroundingOutcome::ALL
        .iter()
        .position(|&entry| entry == outcome)
        .expect("frontend outcome catalog covers its variants")
}

#[derive(Default)]
pub(crate) struct Recorder {
    timings: Mutex<GroundingTimings>,
}

// A callback observer owns its own active interval. Concurrent observers never
// replace another observer's entry, and no lock spans frontend grounding work.
#[must_use = "retain the attempt until its matching phase exit"]
pub(crate) struct Attempt {
    phase: GroundingPhase,
    started: Instant,
}

impl Attempt {
    pub(crate) fn start(phase: GroundingPhase) -> Self {
        Self {
            phase,
            started: Instant::now(),
        }
    }

    fn finish(self, phase: GroundingPhase) -> Duration {
        assert_eq!(phase, self.phase, "phase exit identifies its entry");
        self.started.elapsed()
    }
}

impl Recorder {
    pub(crate) fn exit(
        &self,
        attempt: Attempt,
        phase: GroundingPhase,
        outcome: GroundingOutcome,
        work: &GroundingWork,
        operators: Option<&Operators>,
    ) {
        let elapsed = attempt.finish(phase);
        let operators = operators.map(Operators::take);
        let mut timings = self.lock_timings();
        timings.measurements[phase_index(phase)]
            .get_or_insert_default()
            .record(elapsed, outcome, work);
        for (total, add) in timings
            .operators
            .iter_mut()
            .zip(operators.into_iter().flatten())
        {
            *total = total.and_then(|value| add.and_then(|add| value.checked_add(add)));
        }
    }

    fn lock_timings(&self) -> MutexGuard<'_, GroundingTimings> {
        self.timings.lock().unwrap_or_else(|poisoned| {
            let mut timings = poisoned.into_inner();
            timings.invalid = true;
            timings
        })
    }

    pub(crate) fn snapshot(&self) -> GroundingTimings {
        *self.lock_timings()
    }
}

#[cfg(test)]
mod tests;
