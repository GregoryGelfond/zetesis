//! Bounded complete observation records; original model identity stays outside.

use std::io::Write;

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
            return crate::output::model(output, number, &view, self.options, self.control);
        }
        if self.observations.is_empty() {
            crate::driver::write_model(output, number, model, self.selection)?;
            if let Some(score) = score {
                write!(output, "Optimization:")?;
                for &(_, cost) in score.costs() {
                    write!(output, " {cost}")?;
                }
                writeln!(output)?;
            }
            return Ok(());
        }
        let rendered = self
            .observations
            .render(
                model,
                self.selection,
                zetesis_themelios::observation::Limits {
                    max_work: self.options.max_observation_work,
                    max_bindings: self.options.max_observation_bindings,
                    max_terms: self.options.max_observation_terms,
                    max_output_bytes: self.options.max_observation_bytes,
                    ..Default::default()
                },
                self.control,
            )
            .map_err(RunError::Observation)?;
        let number = number.to_string();
        let mut size = 10_u128 + number.len() as u128 + rendered.text().len() as u128;
        if let Some(score) = score {
            size += 14;
            for &(_, cost) in score.costs() {
                size += 1 + cost.to_string().len() as u128;
            }
        }
        if size > self.options.max_observation_bytes as u128 {
            return Err(RunError::ObservationOutputLimit {
                observed: size,
                limit: self.options.max_observation_bytes,
            });
        }
        let mut record = Vec::new();
        record
            .try_reserve_exact(usize::try_from(size).expect("checked usize output limit"))
            .map_err(|error| RunError::Output(std::io::Error::other(error)))?;
        writeln!(&mut record, "Answer: {number}\n{}", rendered.text())?;
        if let Some(score) = score {
            write!(&mut record, "Optimization:")?;
            for &(_, cost) in score.costs() {
                write!(&mut record, " {cost}")?;
            }
            writeln!(&mut record)?;
        }
        // Control failure before external emission cannot expose half an Answer.
        self.control
            .poll()
            .map_err(|error| RunError::Output(std::io::Error::other(error)))?;
        output.write_all(&record)?;
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
