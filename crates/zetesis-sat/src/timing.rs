//! Optional coarse host timings; independent of semantic work accounting.

use std::time::{Duration, Instant};

/// Host time spent in attempted calls, including calls that return an error.
/// Zero calls means the phase was not entered, not measured zero work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhaseMeasurement {
    /// Number of attempted timed calls that returned to the recorder.
    pub calls: u64,
    /// Sum of host elapsed intervals; neither CPU time nor instruction count.
    pub elapsed: Duration,
    /// Counter/duration overflow made this measurement incomplete. Timing
    /// overflow never changes the solver's semantic result or resource budgets.
    pub overflowed: bool,
}
impl PhaseMeasurement {
    /// Merge a joined worker's timing without turning measurement overflow
    /// into a semantic stop.
    pub(crate) fn merge(&mut self, other: Self) {
        match (
            self.calls.checked_add(other.calls),
            self.elapsed.checked_add(other.elapsed),
        ) {
            (Some(calls), Some(elapsed)) if !self.overflowed && !other.overflowed => {
                self.calls = calls;
                self.elapsed = elapsed;
            }
            _ => self.overflowed = true,
        }
    }

    /// Add one returned attempt without changing solver completion on overflow.
    pub fn record(&mut self, elapsed: Duration) {
        match (self.calls.checked_add(1), self.elapsed.checked_add(elapsed)) {
            (Some(calls), Some(total)) if !self.overflowed => {
                self.calls = calls;
                self.elapsed = total;
            }
            _ => self.overflowed = true,
        }
    }
}

/// Opt-in native search wall intervals. Original-region filter checks nest
/// inside candidate traversal; phase sums therefore need not be exclusive.
/// Initial CNF construction and candidate restriction application are excluded;
/// their caller can measure these operations independently. Driver setup,
/// external batch/device execution, publication and timer overhead are not included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchPhaseTimings {
    /// Candidate query, projection and exact blocking, including failed attempts.
    pub candidates: PhaseMeasurement,
    /// Independent original-model and original-region checks, including
    /// repeated residual prechecks, filter preparation and failed attempts.
    pub original_validation: PhaseMeasurement,
    /// Cold immutable reduct construction, including failed preparation.
    pub reduct_preparation: PhaseMeasurement,
    /// Parameter installation, exact search and returned-countermodel validation.
    pub reduct: PhaseMeasurement,
    /// Selected class membership checking, including failed attempts.
    pub certified: PhaseMeasurement,
}

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Candidates,
    OriginalValidation,
    Reduct,
    ReductPreparation,
    Certified,
}

pub(crate) fn start(timing: Option<&SearchPhaseTimings>) -> Option<Instant> {
    timing.is_some().then(Instant::now)
}

pub(crate) fn finish(
    timing: &mut Option<SearchPhaseTimings>,
    phase: Phase,
    started: Option<Instant>,
) {
    if let (Some(timing), Some(started)) = (timing, started) {
        let measurement = match phase {
            Phase::Candidates => &mut timing.candidates,
            Phase::OriginalValidation => &mut timing.original_validation,
            Phase::Reduct => &mut timing.reduct,
            Phase::ReductPreparation => &mut timing.reduct_preparation,
            Phase::Certified => &mut timing.certified,
        };
        measurement.record(started.elapsed());
    }
}
