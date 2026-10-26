//! Owned native preparations, each borrowing one original canonical program.

use std::sync::Arc;

use themelios_program::program::Program;
use zetesis_cpu::Cancellation;
use zetesis_solve::PreparedInput;
use zetesis_themelios::{
    FormulaFailure, FormulaPurpose, ProgramFormulaOptions, SourceMetadata,
    prepare_program_formula_with,
};

use crate::Config;

/// Prepared by the shared execution policy, as the CLI's input is.
pub(crate) struct Prepared(zetesis_solve::PreparedExecution);

impl Prepared {
    pub(crate) fn new(
        original: Arc<Program>,
        config: &Config,
        cancellation: &Cancellation,
    ) -> Result<Self, FormulaFailure> {
        let resources = config.resources();
        let options = ProgramFormulaOptions {
            admission: resources.program_admission_options(),
            expansion: resources.expansion_limits(),
            formula: resources.formula_limits(),
            purpose: FormulaPurpose::AnswerSets,
            cancellation: Some(cancellation.clone()),
        };
        let prepared = prepare_program_formula_with(original, options)?;
        zetesis_solve::prepare_formula_execution(prepared, &config.session(), None).map(Self)
    }

    pub(crate) fn input(&self) -> PreparedInput<'_> {
        self.0.input()
    }

    pub(crate) fn metadata(&self) -> &SourceMetadata {
        self.0.metadata()
    }
}

#[cfg(test)]
mod tests;
