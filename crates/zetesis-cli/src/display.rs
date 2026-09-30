//! Bounded complete observation records; original model identity stays outside.

pub(crate) mod record;

use std::ops::ControlFlow;

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_themelios::{
    OutputSelection, PreparedSelection,
    observation::{self, ObservationProgram},
};

use crate::{AnswerRenderer, AnswerView, PublicationStop, RunError};

pub(crate) struct Display<'a> {
    selection: &'a OutputSelection,
    observations: &'a ObservationProgram,
    limits: zetesis_themelios::observation::Limits,
    cancellation: &'a Cancellation,
    /// The selection prepared at the first answer and reused for the rest.
    prepared: Option<PreparedSelection<'a>>,
}
impl<'a> Display<'a> {
    pub fn new(
        selection: &'a OutputSelection,
        observations: &'a ObservationProgram,
        limits: zetesis_themelios::observation::Limits,
        cancellation: &'a Cancellation,
    ) -> Self {
        Self {
            selection,
            observations,
            limits,
            cancellation,
            prepared: None,
        }
    }

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
        if self.prepared.is_none() {
            // Preparation only saves repeated work. A stopped preparation
            // leaves the selection unprepared; cancellation persists, so the
            // view below reports the stop in its usual publication phase.
            let prepared = observation::prepare_selection(
                self.selection,
                model.catalog().read(),
                self.limits,
                self.cancellation,
            )
            .unwrap_or_else(|_| PreparedSelection::from(self.selection));
            self.prepared = Some(prepared);
        }
        let selection = self
            .prepared
            .as_ref()
            .expect("the selection is prepared at the first answer");
        let view = self
            .observations
            .view_prepared(model, selection, score, self.limits, self.cancellation)
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
