//! Opt-in host wall intervals, owned outside the fallible driver result.

use std::cell::Cell;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use zetesis_sat::{PhaseMeasurement, SearchPhaseTimings};
use zetesis_telemetry::{SolveStage, StageRecorder, StageSpan, StageTimings};

/// Coarse, nonnested ordinary-solve phases. Counts refer to measured calls,
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
    pub(crate) const ALL: [Self; 13] = [
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
/// Source loading and statistics emission are excluded. Phases do not overlap,
/// but unmeasured orchestration/timer overhead prevents a sum-to-total guarantee.
/// These are not GPU kernel times, CPU utilization, RSS or instruction counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseTimings {
    /// Whole driver scope from admission entry through result/output return.
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
    measurements: Cell<[Option<PhaseMeasurement>; 13]>,
}
impl Recorder {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            stages: StageRecorder::new(enabled),
            grounding: crate::grounding_timing::Recorder::default(),
            measurements: Cell::new([None; 13]),
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

    pub(crate) fn search(&self, timing: Option<SearchPhaseTimings>) {
        if let Some(timing) = timing.filter(|_| self.stages.enabled()) {
            let mut values = self.measurements.get();
            for (phase, value) in [
                (SolvePhase::CandidateGeneration, timing.candidates),
                (SolvePhase::OriginalValidation, timing.original_validation),
                (SolvePhase::ExactReductMembership, timing.reduct),
                (SolvePhase::CertifiedMembership, timing.certified),
            ] {
                values[phase as usize] = (value.calls != 0 || value.overflowed).then_some(value);
            }
            self.measurements.set(values);
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

    pub(crate) fn snapshot(&self) -> Option<PhaseTimings> {
        self.stages.snapshot().map(|stages| PhaseTimings {
            driver_elapsed: stages.driver_elapsed,
            stages,
            grounding: self.grounding.snapshot(),
            measurements: self.measurements.get(),
        })
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
            let mut values = self.recorder.measurements.get();
            values[self.phase as usize]
                .get_or_insert_default()
                .record(elapsed);
            self.recorder.measurements.set(values);
        }
    }
}

pub(crate) fn write(sink: &mut impl Write, timings: &PhaseTimings) -> io::Result<()> {
    writeln!(
        sink,
        "Phase timings: clock=host-monotonic; scope=driver; failed_attempts=included; schema=2"
    )?;
    writeln!(
        sink,
        "  phase driver: elapsed_ns={}",
        timings.driver_elapsed.as_nanos()
    )?;
    for phase in SolvePhase::ALL {
        if let Some(value) = timings.get(phase) {
            writeln!(
                sink,
                "  phase {}: calls={}; elapsed_ns={}; complete={}",
                phase.label(),
                value.calls,
                value.elapsed.as_nanos(),
                !value.overflowed
            )?;
        } else {
            writeln!(sink, "  phase {}: unmeasured", phase.label())?;
        }
    }
    writeln!(
        sink,
        "  phase scope: source_loading=excluded; statistics_output=excluded; timer_overhead=unattributed; kernel_time=unmeasured"
    )
}

#[cfg(test)]
#[path = "../tests/support/phase_timing_contracts.rs"]
mod contracts;
