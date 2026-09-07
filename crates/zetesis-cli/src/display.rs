//! Bounded complete observation records; original model identity stays outside.

mod record;

use std::io::Write;

use record::{Contents, Record};

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_objective::Score;
use zetesis_themelios::{OutputSelection, observation::ObservationProgram};

use crate::{Options, RunError};

pub(crate) struct Display<'a> {
    pub selection: &'a OutputSelection,
    pub observations: &'a ObservationProgram,
    pub options: &'a Options,
    pub control: &'a Control,
}
impl Display<'_> {
    pub fn write(
        &self,
        output: &mut impl Write,
        number: usize,
        model: &Model,
        score: Option<&Score>,
    ) -> Result<(), RunError> {
        if self.options.json {
            let view = self
                .observations
                .view(model, self.selection, score, self.limits(), self.control)
                .map_err(RunError::Observation)?;
            return crate::output::write_model_record(
                output,
                number,
                &view,
                self.options,
                self.control,
            );
        }
        let rendered;
        let contents = if self.observations.is_empty() {
            Contents::Atoms(model, self.selection)
        } else {
            rendered = self
                .observations
                .render(model, self.selection, self.limits(), self.control)
                .map_err(RunError::Observation)?;
            Contents::Observed(rendered.text())
        };
        let record = Record::prepare(
            number,
            contents,
            score,
            self.options.color,
            self.options.max_observation_bytes,
            self.control,
        )?;
        // Semantic, encoding, size and control refusals precede external emission.
        self.control.poll().map_err(RunError::PublicationStopped)?;
        output.write_all(record.bytes())?;
        Ok(())
    }

    fn limits(&self) -> zetesis_themelios::observation::Limits {
        zetesis_themelios::observation::Limits {
            max_work: self.options.max_observation_work,
            max_bindings: self.options.max_observation_bindings,
            max_terms: self.options.max_observation_terms,
            max_output_bytes: self.options.max_observation_bytes,
            ..Default::default()
        }
    }
}
