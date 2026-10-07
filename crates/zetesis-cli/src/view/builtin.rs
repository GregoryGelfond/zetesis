//! Legacy option selection at the edge of the shared renderer contract.

use std::io::Write;
use zetesis_cpu::Cancellation;

use crate::{
    AnswerRenderer, AnswerView, ConfigurationView, HumanRenderer, JsonRenderer, Options,
    PublicationView, RunError, SummaryDelivery, SummaryStage,
};

pub(crate) enum Builtin<W> {
    Human(HumanRenderer<W>),
    Json(JsonRenderer<W>),
}

impl<W: Write> Builtin<W> {
    pub(crate) fn new(output: W, options: &Options) -> Self {
        if options.json {
            Self::Json(JsonRenderer::new(
                output,
                options.resources().json_record_bytes(),
                options.resources().formula_limits().theory.max_atoms,
            ))
        } else {
            Self::Human(HumanRenderer::new(
                output,
                options.color,
                options.resources().observation_limits().max_output_bytes,
            ))
        }
    }
}

impl<W: Write> AnswerRenderer for Builtin<W> {
    fn needs_stage_timings(&self) -> bool {
        match self {
            Self::Human(renderer) => renderer.needs_stage_timings(),
            Self::Json(renderer) => renderer.needs_stage_timings(),
        }
    }

    fn begin(&mut self) -> Result<(), RunError> {
        match self {
            Self::Human(renderer) => renderer.begin(),
            Self::Json(renderer) => renderer.begin(),
        }
    }
    fn configuration(&mut self, view: ConfigurationView<'_>) -> Result<(), RunError> {
        match self {
            Self::Human(renderer) => renderer.configuration(view),
            Self::Json(renderer) => renderer.configuration(view),
        }
    }
    fn summary_stage(&self) -> SummaryStage {
        match self {
            Self::Human(renderer) => renderer.summary_stage(),
            Self::Json(renderer) => renderer.summary_stage(),
        }
    }
    fn answer(
        &mut self,
        view: AnswerView<'_>,
        cancellation: &Cancellation,
    ) -> Result<(), RunError> {
        match self {
            Self::Human(renderer) => renderer.answer(view, cancellation),
            Self::Json(renderer) => renderer.answer(view, cancellation),
        }
    }
    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        match self {
            Self::Human(renderer) => renderer.finish(view),
            Self::Json(renderer) => renderer.finish(view),
        }
    }
}
