//! One publication invocation fixes its terminal callback stage before execution.

use super::{AnswerRenderer, AnswerView, PublicationView, RunError, SummaryDelivery, SummaryStage};
use zetesis_cpu::Control;

/// Owns invocation policy while borrowing the consumer's rendering state.
/// Changing that state during a callback cannot reschedule terminal publication.
pub(crate) struct Session<'a, R> {
    renderer: &'a mut R,
    stage: SummaryStage,
}

impl<'a, R: AnswerRenderer> Session<'a, R> {
    pub(crate) fn start(renderer: &'a mut R) -> Result<Self, RunError> {
        renderer.begin()?;
        let stage = renderer.summary_stage();
        Ok(Self { renderer, stage })
    }
}

impl<R: AnswerRenderer> AnswerRenderer for Session<'_, R> {
    fn summary_stage(&self) -> SummaryStage {
        self.stage
    }

    fn answer(&mut self, view: AnswerView<'_>, control: &Control) -> Result<(), RunError> {
        self.renderer.answer(view, control)
    }

    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        self.renderer.finish(view)
    }
}
