//! One publication invocation fixes its terminal callback stage before execution.

use super::{
    AnswerRenderer, AnswerView, ConfigurationView, PublicationView, RunError, SummaryDelivery,
    SummaryStage,
};
use zetesis_cpu::Cancellation;

/// Owns invocation policy while borrowing the consumer's rendering state.
/// Changing that state during a callback cannot reschedule terminal publication.
pub(crate) struct Session<'a, R> {
    renderer: &'a mut R,
    stage: SummaryStage,
    stage_timings: bool,
}

impl<'a, R: AnswerRenderer> Session<'a, R> {
    pub(crate) fn start(renderer: &'a mut R) -> Result<Self, RunError> {
        renderer.begin()?;
        let stage = renderer.summary_stage();
        let stage_timings = renderer.needs_stage_timings();
        Ok(Self {
            renderer,
            stage,
            stage_timings,
        })
    }
}

impl<R: AnswerRenderer> AnswerRenderer for Session<'_, R> {
    fn needs_stage_timings(&self) -> bool {
        self.stage_timings
    }

    fn configuration(&mut self, view: ConfigurationView<'_>) -> Result<(), RunError> {
        self.renderer.configuration(view)
    }

    fn summary_stage(&self) -> SummaryStage {
        self.stage
    }

    fn answer(
        &mut self,
        view: AnswerView<'_>,
        cancellation: &Cancellation,
    ) -> Result<(), RunError> {
        self.renderer.answer(view, cancellation)
    }

    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        self.renderer.finish(view)
    }
}
