//! Application dispatch for the compatibility experiment command.
//!
//! Each measurement owns its typed configuration and event API. This adapter
//! retains their legacy views and exit distinctions without process-global I/O.

use std::{fmt, io};

use crate::{CommandOptions, Experiment};

/// Whether the complete requested schedule qualified every operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    /// Every requested observation passed its comparison contract.
    Passed,
    /// A complete table schedule retained an applicability refusal.
    Refused,
}

/// Typed failure from the selected experiment; an output prefix may remain.
#[derive(Debug)]
pub enum Error {
    /// Static reduct closure measurement failed.
    Static(crate::BenchmarkError),
    /// General formula measurement failed.
    Formula(crate::FormulaBenchmarkError),
    /// Finite-table measurement failed.
    Table(crate::table_measurement::Error),
    /// Conditional guard measurement failed.
    Feedback(crate::feedback_measurement::Error),
    /// Relation-mask measurement failed.
    Relation(crate::relation_measurement::Error),
    /// Aggregate measurement failed.
    Aggregate(crate::aggregate_measurement::Error),
    /// Tight certificate measurement failed.
    Tight(crate::tight_measurement::Error),
    /// Lazy source measurement failed.
    Lazy(crate::lazy_measurement::Error),
    /// Source admission measurement failed.
    Grounding(crate::grounding::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(error) => error.fmt(formatter),
            Self::Formula(error) => error.fmt(formatter),
            Self::Table(error) => error.fmt(formatter),
            Self::Feedback(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Aggregate(error) => error.fmt(formatter),
            Self::Tight(error) => error.fmt(formatter),
            Self::Lazy(error) => error.fmt(formatter),
            Self::Grounding(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Static(error) => error,
            Self::Formula(error) => error,
            Self::Table(error) => error,
            Self::Feedback(error) => error,
            Self::Relation(error) => error,
            Self::Aggregate(error) => error,
            Self::Tight(error) => error,
            Self::Lazy(error) => error,
            Self::Grounding(error) => error,
        })
    }
}

/// Execute the selected legacy view through its library measurement owner.
///
/// # Errors
/// Returns the original typed experiment failure. Incomplete work and writer
/// failure never become a successful completion; already emitted bytes remain.
pub fn execute(options: &CommandOptions, output: &mut impl io::Write) -> Result<Completion, Error> {
    match &options.command {
        Some(Experiment::Table(options)) => {
            return crate::table_measurement::run(options, output)
                .map(|passed| {
                    if passed {
                        Completion::Passed
                    } else {
                        Completion::Refused
                    }
                })
                .map_err(Error::Table);
        }
        Some(Experiment::Feedback(options)) => {
            crate::feedback_measurement::run(options, output).map_err(Error::Feedback)?;
        }
        Some(Experiment::Relation(options)) => {
            crate::relation_measurement::run(options, output).map_err(Error::Relation)?;
        }
        Some(Experiment::Aggregate(options)) => {
            crate::aggregate_measurement::run(options, output).map_err(Error::Aggregate)?;
        }
        Some(Experiment::Tight(options)) => {
            crate::tight_measurement::run(options, output).map_err(Error::Tight)?;
        }
        Some(Experiment::Lazy(options)) => {
            crate::lazy_measurement::run(options, output).map_err(Error::Lazy)?;
        }
        Some(Experiment::FormulaProjection(options)) => {
            crate::run_formula_projection(options, output).map_err(Error::Formula)?;
        }
        Some(Experiment::Grounding(options)) => {
            crate::grounding::run(options, output).map_err(Error::Grounding)?;
        }
        Some(Experiment::Formula(options)) => {
            crate::run_formula(options, output).map_err(Error::Formula)?;
        }
        None => crate::run(&options.static_options, output).map_err(Error::Static)?,
    }
    Ok(Completion::Passed)
}
