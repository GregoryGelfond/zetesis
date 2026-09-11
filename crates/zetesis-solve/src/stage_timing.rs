//! Frontend callback boundaries for reusable stage and grounding measurements.

use std::cell::RefCell;
use zetesis_telemetry::{SolveStage, StageRecorder, StageSpan};

// The current frontend ground() boundary is non-reentrant. Nested callback
// boundaries would require retaining a stack of stage guards.
pub(crate) struct Observer<'a> {
    recorder: &'a StageRecorder,
    grounding: &'a crate::grounding_timing::Recorder,
    span: RefCell<Option<StageSpan<'a>>>,
    phase: RefCell<Option<crate::grounding_timing::Attempt>>,
}
impl<'a> Observer<'a> {
    pub(crate) const fn new(
        recorder: &'a StageRecorder,
        grounding: &'a crate::grounding_timing::Recorder,
    ) -> Self {
        Self {
            recorder,
            grounding,
            span: RefCell::new(None),
            phase: RefCell::new(None),
        }
    }
}
impl zetesis_themelios::GroundingObserver for Observer<'_> {
    fn enter(&self) {
        *self.span.borrow_mut() = Some(self.recorder.enter(SolveStage::Grounding));
    }
    fn exit(&self) {
        self.span.borrow_mut().take();
    }
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_enter(
        &self,
        phase: crate::GroundingPhase,
        _location: Option<zetesis_themelios::base::span::Location>,
    ) {
        assert!(
            self.phase
                .replace(Some(crate::grounding_timing::Attempt::start(phase)))
                .is_none(),
            "frontend grounding phases must not nest"
        );
    }
    fn phase_exit(
        &self,
        phase: crate::GroundingPhase,
        _location: Option<zetesis_themelios::base::span::Location>,
        outcome: crate::GroundingOutcome,
        work: crate::GroundingWork,
    ) {
        let attempt = self.phase.take().expect("phase exit follows its entry");
        self.grounding.exit(attempt, phase, outcome, work);
    }
}
