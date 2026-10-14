//! Owned native preparations, each borrowing one original canonical program.

use std::sync::Arc;

use themelios_program::program::Program;
use zetesis_cpu::Cancellation;
use zetesis_solve::PreparedInput;
use zetesis_themelios::{
    AutomaticAdmission, AutomaticAdmissionFailure, FormulaFailure, FormulaPurpose,
    PreparedRelational, ProgramFormulaOptions, SourceMetadata, prepare_program_automatic,
    prepare_program_formula_with,
};

use crate::{Config, Grounder};

pub(crate) enum Prepared {
    Relational(Box<PreparedRelational>),
    /// Prepared by the shared execution policy, as the CLI's input is.
    Execution(zetesis_solve::PreparedExecution),
}

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
        let prepared = if config.grounder == Grounder::Lazy {
            match prepare_program_automatic(original, options) {
                Ok(AutomaticAdmission::Extended(relational)) => {
                    return Ok(Self::Relational(Box::new(relational)));
                }
                Ok(AutomaticAdmission::Formula(prepared)) => prepared,
                Err(
                    AutomaticAdmissionFailure::Extended(error)
                    | AutomaticAdmissionFailure::Formula(error),
                ) => return Err(error),
            }
        } else {
            prepare_program_formula_with(original, options)?
        };
        zetesis_solve::prepare_formula_execution(prepared, &config.session(), None)
            .map(Self::Execution)
    }

    pub(crate) fn input(&self) -> PreparedInput<'_> {
        match self {
            Self::Relational(owner) => PreparedInput::relational(owner),
            Self::Execution(prepared) => prepared.input(),
        }
    }

    pub(crate) fn metadata(&self) -> &SourceMetadata {
        match self {
            Self::Relational(owner) => owner.metadata(),
            Self::Execution(prepared) => prepared.metadata(),
        }
    }
}

#[cfg(test)]
mod tests;
