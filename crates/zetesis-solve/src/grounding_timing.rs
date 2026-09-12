//! Fixed-space aggregation of optional formula-grounding observations.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use zetesis_themelios::{GroundingOutcome, GroundingPhase, GroundingWork};

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroundingTimings {
    invalid: bool,
    measurements: [Option<GroundingMeasurement>; GroundingPhase::ALL.len()],
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
    ) {
        let elapsed = attempt.finish(phase);
        self.lock_timings().measurements[phase_index(phase)]
            .get_or_insert_default()
            .record(elapsed, outcome, work);
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
#[path = "../tests/support/grounding_timing_contracts.rs"]
mod tests;
