//! Owned native preparations, each borrowing one original canonical program.

use std::sync::Arc;

use themelios_program::program::Program;
use zetesis_cpu::Cancellation;
use zetesis_solve::PreparedInput;
use zetesis_themelios::{
    FormulaFailure, FormulaPurpose, PreparedRelational, ProgramFormulaOptions,
    ProgramRelationalOptions, SourceMetadata, prepare_program_formula_with,
    prepare_program_relational,
};

use crate::{Config, Grounder};

pub(crate) enum Prepared {
    Relational(Box<PreparedRelational>),
    /// Materialized by the library's grounder mapping, as the CLI's is.
    Formula(Box<zetesis_solve::GroundedFormula>),
}

impl Prepared {
    pub(crate) fn new(
        original: Arc<Program>,
        config: &Config,
        cancellation: &Cancellation,
    ) -> Result<Self, FormulaFailure> {
        let resources = config.resources();
        let formula = resources.formula_limits();
        if config.grounder == Grounder::Lazy {
            let relational = prepare_program_relational(
                Arc::clone(&original),
                ProgramRelationalOptions {
                    admission: resources.program_admission_options(),
                    expansion: resources.expansion_limits(),
                    observation: formula.observation,
                    metadata_storage: formula.metadata_storage,
                    purpose: FormulaPurpose::AnswerSets,
                    cancellation: Some(cancellation.clone()),
                },
            );
            match relational {
                Ok(relational) => return Ok(Self::Relational(Box::new(relational))),
                // Only a construct the relational profile lacks moves the
                // program to lazy formula grounding, as the CLI's does.
                Err(failure) if outside_relational_profile(&failure) => {}
                Err(failure) => return Err(failure),
            }
        }
        let prepared = prepare_program_formula_with(
            original,
            ProgramFormulaOptions {
                admission: resources.program_admission_options(),
                expansion: resources.expansion_limits(),
                formula,
                purpose: FormulaPurpose::AnswerSets,
                cancellation: Some(cancellation.clone()),
            },
        )?;
        let grounder = match config.grounder {
            Grounder::Auto => zetesis_solve::Grounder::Auto,
            Grounder::Eager => zetesis_solve::Grounder::Eager,
            Grounder::Lazy => zetesis_solve::Grounder::Lazy,
        };
        zetesis_solve::ground_formula(prepared, grounder, None)
            .map(|grounded| Self::Formula(Box::new(grounded)))
    }

    pub(crate) fn input(&self) -> PreparedInput<'_> {
        match self {
            Self::Relational(owner) => PreparedInput::relational(owner),
            Self::Formula(grounded) => grounded.input(),
        }
    }

    pub(crate) fn metadata(&self) -> &SourceMetadata {
        match self {
            Self::Relational(owner) => owner.metadata(),
            Self::Formula(grounded) => grounded.metadata(),
        }
    }
}

/// Whether relational preparation refused only a construct its profile lacks.
fn outside_relational_profile(failure: &FormulaFailure) -> bool {
    matches!(failure.cause(), FormulaFailure::Expansion(error) if error.needs_formula_admission())
}

#[cfg(test)]
mod tests;
