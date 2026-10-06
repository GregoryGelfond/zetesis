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
        if config.grounder == Grounder::Lazy {
            return prepare_program_relational(
                original,
                ProgramRelationalOptions {
                    admission: config.admission,
                    expansion: config.expansion,
                    observation: config.formula.observation,
                    metadata_storage: config.formula.metadata_storage,
                    purpose: FormulaPurpose::AnswerSets,
                    cancellation: Some(cancellation.clone()),
                },
            )
            .map(|relational| Self::Relational(Box::new(relational)));
        }
        let prepared = prepare_program_formula_with(
            original,
            ProgramFormulaOptions {
                admission: config.admission,
                expansion: config.expansion,
                formula: config.formula,
                purpose: FormulaPurpose::AnswerSets,
                cancellation: Some(cancellation.clone()),
            },
        )?;
        // The facade's `Hybrid` is the lazy formula materialization.
        let grounder = match config.grounder {
            Grounder::Auto => zetesis_solve::Grounder::Auto,
            Grounder::Eager => zetesis_solve::Grounder::Eager,
            Grounder::Hybrid => zetesis_solve::Grounder::Lazy,
            Grounder::Lazy => {
                unreachable!("relational preparation returned before formula compilation")
            }
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

#[cfg(test)]
mod tests;
