//! Search completion and logical interruption.

use std::fmt;
use zetesis_cpu::Stop;

/// Why a successful driver invocation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    /// Every relevant candidate was checked. With an objective, candidates
    /// worse than a verified incumbent may be omitted; all optimal ties remain
    /// covered. This is search coverage, independently of output delivery.
    /// UNSAT additionally requires no verified stable model.
    Exhausted,
    /// The requested number of models was returned; coverage remains partial.
    RequestedModels,
    /// Search or one oracle stopped with explicit incomplete coverage.
    Interrupted,
}

/// A typed reason why model enumeration could not establish complete coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interruption {
    /// Incremental candidate enumeration or a closure oracle stopped.
    Oracle(Stop),
    /// Reduct countermodel encoding, search or independent witness checking stopped.
    Countermodel(zetesis_sat::Incomplete),
    /// An objective could not be completely evaluated for a verified model.
    Objective(zetesis_objective::Error),
    /// Retaining complete incumbent models exceeded an explicit bound.
    Incumbent(crate::OptimizationStop),
}
impl fmt::Display for Interruption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oracle(error) => error.fmt(f),
            Self::Countermodel(error) => error.fmt(f),
            Self::Objective(error) => error.fmt(f),
            Self::Incumbent(error) => error.fmt(f),
        }
    }
}
