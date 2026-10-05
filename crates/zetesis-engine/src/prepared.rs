//! Owned native preparations, each borrowing one original canonical program.

use std::sync::Arc;

use themelios_program::program::Program;
use zetesis_cpu::Cancellation;
use zetesis_solve::PreparedInput;
use zetesis_themelios::{
    AdmittedFormula, FormulaFailure, FormulaMaterialization, FormulaPurpose, HybridFormula,
    PreparedRelational, ProgramFormulaOptions, ProgramRelationalOptions, SourceMetadata,
    TerminalFormula, prepare_program_formula_with, prepare_program_relational,
};

use crate::{Config, Grounder};

pub(crate) enum Prepared {
    Relational(Box<PreparedRelational>),
    Formula(Box<AdmittedFormula>),
    Hybrid(HybridFormula),
    Terminal(TerminalFormula),
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
        match config.grounder {
            Grounder::Auto => prepared
                .ground_adaptive()
                .map(|materialized| match materialized {
                    FormulaMaterialization::Complete(formula) => Self::Formula(Box::new(formula)),
                    FormulaMaterialization::Terminal(terminal) => Self::Terminal(terminal),
                }),
            Grounder::Eager => prepared
                .ground()
                .map(|formula| Self::Formula(Box::new(formula))),
            Grounder::Hybrid => prepared.ground_hybrid().map(Self::Hybrid),
            Grounder::Lazy => {
                unreachable!("relational preparation returned before formula compilation")
            }
        }
    }

    pub(crate) fn input(&self) -> PreparedInput<'_> {
        match self {
            Self::Relational(owner) => PreparedInput::relational(owner),
            Self::Formula(owner) => PreparedInput::formula(owner),
            Self::Hybrid(owner) => PreparedInput::hybrid(owner),
            Self::Terminal(owner) => PreparedInput::terminal(owner),
        }
    }

    pub(crate) fn metadata(&self) -> &SourceMetadata {
        match self {
            Self::Relational(owner) => owner.metadata(),
            Self::Formula(owner) => owner.metadata(),
            Self::Hybrid(owner) => owner.metadata(),
            Self::Terminal(owner) => owner.metadata(),
        }
    }
}

#[cfg(test)]
mod tests;
