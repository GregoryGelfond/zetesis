//! Bounded complete observation records; original model identity stays outside.

pub(crate) mod record;

use std::ops::ControlFlow;

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_themelios::{OutputSelection, observation::ObservationProgram};

use crate::{AnswerRenderer, AnswerView, PublicationStop, RunError};

pub(crate) struct Display<'a> {
    pub selection: &'a OutputSelection,
    pub observations: &'a ObservationProgram,
    pub limits: zetesis_themelios::observation::Limits,
    pub cancellation: &'a Cancellation,
}
impl Display<'_> {
    pub fn write(
        &mut self,
        renderer: &mut impl AnswerRenderer,
        number: usize,
        model: &Model,
        score: Option<&Score>,
    ) -> Result<ControlFlow<PublicationStop>, RunError> {
        match self.write_record(renderer, number, model, score) {
            Ok(()) => Ok(ControlFlow::Continue(())),
            Err(error) => PublicationStop::classify(error).map(ControlFlow::Break),
        }
    }

    fn write_record(
        &mut self,
        renderer: &mut impl AnswerRenderer,
        number: usize,
        model: &Model,
        score: Option<&Score>,
    ) -> Result<(), RunError> {
        let view = self
            .observations
            .view(model, self.selection, score, self.limits, self.cancellation)
            .map_err(RunError::Observation)?;
        self.cancellation
            .poll()
            .map_err(RunError::PublicationStopped)?;
        renderer.answer(
            AnswerView {
                number,
                model: &view,
                limits: self.limits,
                observations: !self.observations.is_empty(),
            },
            self.cancellation,
        )
    }
}
