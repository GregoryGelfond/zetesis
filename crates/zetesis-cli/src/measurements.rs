//! Shared ownership of one optional host-instrumentation scope.

use std::rc::Rc;

use crate::phase_timing::{Recorder, Span};
use crate::{PhaseTimings, SolvePhase, SolveStage};
use zetesis_telemetry::StageSpan;

/// Caller-owned host measurements shared by operations on one thread.
///
/// Cloning shares one recorder and its original start time. It does not copy or
/// restart measurements. The last owner releases the recorder; guards and
/// grounding observers borrow an owner and cannot outlive it. Independent calls
/// to [`Self::new`] create independent scopes. This handle is neither `Send` nor
/// `Sync`; it does not coordinate concurrent work.
///
/// Each owner retains a fixed set of phase, stage and grounding slots. Creating
/// an owner allocates one fixed-size shared record; cloning and all recording
/// operations take constant space. Snapshots and recording take constant time
/// over those fixed slots. Enabled operations read the host clock. Disabled
/// operations read no clocks, and snapshots remain absent.
///
/// Callers define the measured scope and may record arbitrary attempted work.
/// Neither a measurement nor its label establishes admission, membership,
/// completion, publication or recorder provenance. Measurements do not impose
/// resource limits or poll cancellation. Nested stage guards must be dropped in
/// stack order across every clone; invalid nesting makes timing attribution
/// unavailable without changing the application result.
#[derive(Clone)]
pub struct SolveMeasurements {
    recorder: Rc<Recorder>,
}

impl SolveMeasurements {
    /// Create one fixed-size recorder with optional host-clock measurements.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            recorder: Rc::new(Recorder::new(enabled)),
        }
    }

    /// Whether this scope records host-clock measurements.
    /// This inspection reads no clock.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.recorder().enabled()
    }

    /// Enter one phase attempt, recording its duration when the guard is dropped.
    ///
    /// Phase intervals are nonexclusive. Admission and observation phases also
    /// enter their corresponding exclusive host stage. Keep those stage guards
    /// in stack order; overlapping phase intervals cannot be added as a total.
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
    /// Keep guards in stack order across all clones of this owner.
    pub fn stage(&self, stage: SolveStage) -> StageSpan<'_> {
        self.recorder().stage(stage)
    }

    /// Borrow optional attribution for themelios's non-reentrant grounding calls.
    ///
    /// Pass this observer to the frontend's grounding-observer API. At most one
    /// grounding call may be active across clones of this owner; phase callbacks
    /// must be paired in order. The frontend's callbacks establish measurement
    /// boundaries, not evidence that admission or grounding succeeded.
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
