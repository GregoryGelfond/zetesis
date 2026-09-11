//! Opt-in host wall intervals, owned outside the fallible driver result.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use zetesis_sat::{PhaseMeasurement, SearchPhaseTimings};
use zetesis_telemetry::{SolveStage, StageRecorder, StageSpan, StageTimings};

/// Coarse ordinary-solve phases. Counts refer to measured calls,
/// including failed attempts, rather than candidates, models or instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolvePhase {
    /// Source admission and its finite materialization; includes automatic retries.
    AdmissionMaterialization,
    /// Execution setup, including static lowering, pools or device initialization.
    ExecutionSetup,
    /// Candidate generator initialization or initial classical formula encoding.
    CandidateSetup,
    /// Complete original-theory certificate construction, including refusals.
    CertificateSetup,
    /// Ranked-support membership checking, including interrupted attempts.
    CertifiedMembership,
    /// Candidate generation/projection and exact semantic blocking.
    CandidateGeneration,
    /// Independent original-formula validation, including residual prechecks.
    OriginalValidation,
    /// GPU host oracle call, including packing, waits, readback and failed calls.
    GpuHostOracle,
    /// Exact frozen-reduct encoding/search and countermodel validation on CPU.
    ExactReductMembership,
    /// CPU closure batch execution, including worker scheduling and collection.
    ClosureMembership,
    /// Objective evaluation, score comparison and bounded full-model retention.
    ObjectiveScoringRetention,
    /// Objective plan, bound compilation and candidate-only restriction application.
    ObjectiveFeedback,
    /// Observation/rendering/output and final result summaries, including failures.
    ObservationOutput,
}
impl SolvePhase {
    /// Finite phase catalog in report order.
    pub const ALL: [Self; 13] = [
        Self::AdmissionMaterialization,
        Self::ExecutionSetup,
        Self::CandidateSetup,
        Self::CertificateSetup,
        Self::CertifiedMembership,
        Self::CandidateGeneration,
        Self::OriginalValidation,
        Self::GpuHostOracle,
        Self::ExactReductMembership,
        Self::ClosureMembership,
        Self::ObjectiveScoringRetention,
        Self::ObjectiveFeedback,
        Self::ObservationOutput,
    ];

    /// Stable identifier in the appended phase-statistics section.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AdmissionMaterialization => "admission_materialization",
            Self::ExecutionSetup => "execution_setup",
            Self::CandidateSetup => "candidate_setup",
            Self::CertificateSetup => "certificate_setup",
            Self::CertifiedMembership => "certified_membership",
            Self::CandidateGeneration => "candidate_generation",
            Self::OriginalValidation => "original_validation",
            Self::GpuHostOracle => "gpu_host_oracle",
            Self::ExactReductMembership => "exact_reduct_membership",
            Self::ClosureMembership => "closure_membership",
            Self::ObjectiveScoringRetention => "objective_scoring_retention",
            Self::ObjectiveFeedback => "objective_feedback",
            Self::ObservationOutput => "observation_output",
        }
    }
}

/// A snapshot of attempted host intervals, independent of semantic completion.
/// Callers define the scope; phase intervals can overlap within or across threads.
/// Their sum is not a wall-time partition. Exclusive stages separately report
/// whether their nesting established such a partition.
/// These are not GPU kernel times, CPU utilization, RSS or instruction counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseTimings {
    /// Wall time since the measurement owner was created. A caller-shared scope
    /// can include admission, multiple sessions, time between pulls and publication.
    pub driver_elapsed: Duration,
    /// Exclusive high-level host stages; accessible without parsing diagnostics.
    pub stages: StageTimings,
    /// Optional attribution within eager formula grounding. Other grounders remain unmeasured here.
    pub grounding: crate::GroundingTimings,
    measurements: [Option<PhaseMeasurement>; 13],
}
impl PhaseTimings {
    /// An entered phase's attempted time; `None` means unentered or inapplicable.
    #[must_use]
    pub const fn get(&self, phase: SolvePhase) -> Option<PhaseMeasurement> {
        self.measurements[phase as usize]
    }
}

pub(crate) struct Recorder {
    stages: StageRecorder,
    grounding: crate::grounding_timing::Recorder,
    measurements: Mutex<[Option<PhaseMeasurement>; 13]>,
}
impl Recorder {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            stages: StageRecorder::new(enabled),
            grounding: crate::grounding_timing::Recorder::default(),
            measurements: Mutex::new([None; 13]),
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.stages.enabled()
    }

    pub(crate) fn start(&self, phase: SolvePhase) -> Span<'_> {
        Span {
            recorder: self,
            phase,
            started: self.stages.enabled().then(Instant::now),
            _stage: match phase {
                SolvePhase::AdmissionMaterialization => {
                    Some(self.stage(SolveStage::SourcePreparation))
                }
                SolvePhase::ObservationOutput => Some(self.stage(SolveStage::ObservationOutput)),
                _ => None,
            },
        }
    }

    pub(crate) fn measure<T>(&self, phase: SolvePhase, action: impl FnOnce() -> T) -> T {
        let _span = self.start(phase);
        action()
    }

    pub(crate) fn search(&self, previous: SearchPhaseTimings, current: SearchPhaseTimings) {
        if self.stages.enabled() {
            let mut values = self.lock_measurements();
            for (phase, before, after) in [
                (
                    SolvePhase::CandidateGeneration,
                    previous.candidates,
                    current.candidates,
                ),
                (
                    SolvePhase::OriginalValidation,
                    previous.original_validation,
                    current.original_validation,
                ),
                (
                    SolvePhase::ExactReductMembership,
                    previous.reduct,
                    current.reduct,
                ),
                (
                    SolvePhase::CertifiedMembership,
                    previous.certified,
                    current.certified,
                ),
            ] {
                if self.measurements.is_poisoned() && before != after {
                    values[phase as usize].get_or_insert_default().overflowed = true;
                } else {
                    append_difference(&mut values[phase as usize], before, after);
                }
            }
        }
    }

    pub(crate) fn stage(&self, stage: SolveStage) -> StageSpan<'_> {
        self.stages.enter(stage)
    }

    pub(crate) fn lazy_grounding(&self) {
        self.stages.mark_lazy_grounding();
    }

    pub(crate) fn grounding_observer(&self) -> Option<crate::stage_timing::Observer<'_>> {
        self.stages
            .enabled()
            .then(|| crate::stage_timing::Observer::new(&self.stages, &self.grounding))
    }

    // No caller code runs under this lock. A poisoned accumulator retains its
    // known prefix as incomplete; it cannot turn instrumentation into a solve fault.
    fn lock_measurements(&self) -> MutexGuard<'_, [Option<PhaseMeasurement>; 13]> {
        self.measurements.lock().unwrap_or_else(|poisoned| {
            let mut values = poisoned.into_inner();
            for measurement in values.iter_mut().flatten() {
                measurement.overflowed = true;
            }
            values
        })
    }

    pub(crate) fn snapshot(&self) -> Option<PhaseTimings> {
        self.stages.snapshot().map(|stages| PhaseTimings {
            driver_elapsed: stages.driver_elapsed,
            stages,
            grounding: self.grounding.snapshot(),
            measurements: *self.lock_measurements(),
        })
    }
}

// Each session supplies its last imported snapshot and current cumulative one.
// Import only the new prefix, preserving other sessions and caller-measured work.
// Missing/overflowed arithmetic retains the known aggregate prefix and marks it
// incomplete. Once incomplete, the aggregate freezes like PhaseMeasurement.
fn append_difference(
    measurement: &mut Option<PhaseMeasurement>,
    previous: PhaseMeasurement,
    current: PhaseMeasurement,
) {
    if current == previous {
        return;
    }
    let aggregate = measurement.get_or_insert_default();
    if aggregate.overflowed {
        return;
    }
    let total = current
        .calls
        .checked_sub(previous.calls)
        .zip(current.elapsed.checked_sub(previous.elapsed))
        .and_then(|(calls, elapsed)| {
            aggregate
                .calls
                .checked_add(calls)
                .zip(aggregate.elapsed.checked_add(elapsed))
        });
    if let Some((calls, elapsed)) = total.filter(|_| !previous.overflowed) {
        aggregate.calls = calls;
        aggregate.elapsed = elapsed;
        aggregate.overflowed = current.overflowed;
    } else {
        aggregate.overflowed = true;
    }
}

pub(crate) struct Span<'a> {
    recorder: &'a Recorder,
    phase: SolvePhase,
    started: Option<Instant>,
    _stage: Option<StageSpan<'a>>,
}
impl Drop for Span<'_> {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed();
            let mut values = self.recorder.lock_measurements();
            let measurement = values[self.phase as usize].get_or_insert_default();
            if self.recorder.measurements.is_poisoned() {
                measurement.overflowed = true;
            } else {
                measurement.record(elapsed);
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/phase_timing_contracts.rs"]
mod contracts;
