//! Shared ownership of one optional host-instrumentation scope.

use std::sync::Arc;

use crate::phase_timing::{Recorder, Span};
use crate::{PhaseTimings, SolvePhase, SolveStage};
use zetesis_telemetry::StageSpan;

/// Caller-owned host measurements shared across operations and threads.
///
/// Cloning shares one recorder and its original start time. It does not copy or
/// restart measurements. The last owner releases the recorder; guards and
/// grounding observers borrow an owner and cannot outlive it. Independent calls
/// to [`Self::new`] or [`Self::stages_only`] create independent scopes. Clones
/// are `Send` and `Sync`; brief internal locks protect bookkeeping without
/// enclosing application work, frontend calls or observer callbacks.
///
/// Each owner retains a fixed set of phase, stage and grounding slots. Creating
/// an owner allocates one fixed-size shared record; cloning and all recording
/// operations take constant space. Snapshots and recording visit only those
/// fixed slots; concurrent bookkeeping may briefly wait for a lock. Enabled
/// stages read the host clock. Detailed measurements additionally time phase
/// attempts and collect frontend work. A stages-only scope leaves those
/// detailed measurements absent. Disabled operations read no clocks, and
/// snapshots remain absent.
///
/// Callers define the measured scope and may record arbitrary attempted work.
/// Neither a measurement nor its label establishes admission, membership,
/// completion, publication or recorder provenance. Measurements do not impose
/// resource limits or poll cancellation. Nested stage guards must be dropped in
/// stack order on their creating thread across every clone. Overlapping stage
/// scopes from different threads, transferred live stage guards, invalid
/// nesting or poisoned bookkeeping make attribution unavailable without
/// changing the application result. Independent phase durations sum attempted
/// intervals and can overlap across threads.
#[derive(Clone)]
pub struct SolveMeasurements {
    recorder: Arc<Recorder>,
}

impl SolveMeasurements {
    /// Create one fixed-size recorder with optional detailed host measurements.
    /// `true` enables stages, phase attempts and frontend work attribution;
    /// `false` disables all measurement clocks and returns no snapshots.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            recorder: Arc::new(Recorder::new(enabled)),
        }
    }

    /// Record exclusive host stages without detailed phase or grounding work.
    ///
    /// Snapshots retain their stage intervals and driver elapsed time, while
    /// every detailed phase and grounding measurement remains absent. Passing
    /// this scope to a session disables its optional detailed statistics.
    /// Admission and observation phase guards still enter their coarse stages;
    /// other phase guards read no clocks. Each scope has independent ownership.
    #[must_use]
    pub fn stages_only() -> Self {
        Self {
            recorder: Arc::new(Recorder::stages_only()),
        }
    }

    /// Whether this scope records host-clock measurements.
    /// This inspection reads no clock.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.recorder().enabled()
    }

    /// Whether detailed phase clocks and frontend work attribution are enabled.
    /// A stages-only scope returns `false`. This inspection reads no clock.
    #[must_use]
    pub fn details_enabled(&self) -> bool {
        self.recorder().details_enabled()
    }

    /// Enter one phase attempt, recording its duration when the guard is dropped.
    ///
    /// Phase intervals are nonexclusive. Admission and observation phases also
    /// enter their corresponding exclusive host stage, including in a stages-only
    /// scope. Other phase guards read no clock when detailed timing is disabled.
    /// Keep those stage guards in stack order; overlapping phase intervals
    /// cannot be added as a total.
    pub fn enter(&self, phase: SolvePhase) -> MeasurementSpan<'_> {
        MeasurementSpan {
            _span: self.recorder().start(phase),
        }
    }

    /// Measure an attempted operation and return its value unchanged.
    /// Returned errors and unwinding still close the phase attempt.
    pub fn measure<T>(&self, phase: SolvePhase, action: impl FnOnce() -> T) -> T {
        let _span = self.enter(phase);
        action()
    }

    /// Enter an exclusive host stage, suspending its parent until guard drop.
    /// Enter and drop guards in stack order on their creating thread across all
    /// clones. Cross-thread overlap or transferring a live guard makes the
    /// exclusive partition unavailable.
    pub fn stage(&self, stage: SolveStage) -> StageSpan<'_> {
        self.recorder().stage(stage)
    }

    /// Borrow optional attribution for themelios's non-reentrant grounding calls.
    ///
    /// Pass this observer to the frontend's grounding-observer API. Each observer
    /// owns its active callback token; phase callbacks must be paired in order
    /// without nesting on that observer. Separate observers can record concurrent
    /// attempts, whose durations and work are summed. The frontend's callbacks
    /// establish measurement boundaries, not evidence that admission or
    /// grounding succeeded.
    /// A stages-only observer requests no detailed counters and ignores phase
    /// callbacks, but still records the coarse grounding interval and route.
    /// A disabled scope returns `None` without reading a clock.
    #[must_use]
    pub fn grounding_observer(&self) -> Option<impl zetesis_themelios::GroundingObserver + '_> {
        self.recorder().grounding_observer()
    }

    /// Copy the currently available measurements without resetting this scope.
    ///
    /// Active exclusive stages contribute their elapsed prefix. Phase and
    /// grounding measurements include only closed attempts. The returned value
    /// can be inspected or edited without changing the shared recorder.
    /// A stages-only snapshot has no detailed phase or grounding measurements.
    #[must_use]
    pub fn snapshot(&self) -> Option<PhaseTimings> {
        self.recorder().snapshot()
    }

    pub(crate) fn recorder(&self) -> &Recorder {
        &self.recorder
    }
}

/// Borrowed phase attempt closed on return, explicit drop or unwinding.
/// Retain this guard for the full attempted interval. It carries no semantic
/// evidence and cannot outlive the measurement owner it borrows.
#[must_use = "retain the guard until the measured phase ends"]
pub struct MeasurementSpan<'a> {
    _span: Span<'a>,
}
