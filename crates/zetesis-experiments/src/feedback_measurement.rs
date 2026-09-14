//! Bounded experimental conditional-countermodel restrictions.
//!
//! Eight tiny original theories are checked exhaustively. One arm replays every
//! interpretation, optionally avoiding native membership calls with guards learned
//! from subject-bound native countermodels. A separate arm installs pre-acquired
//! guards through the existing SAT restriction door. Neither arm changes ordinary
//! solving. See `Feedback.lean` for semantic laws, not a Rust refinement proof.

mod config;
mod fixtures;
mod guard;
mod replay;
mod run;
mod view;
#[cfg(test)]
mod tests;

use std::{fmt, io};
pub use config::{Configuration, ConstructionLimits, Options};
pub use fixtures::Case;
pub use run::{measure, measure_with_control};
pub use view::{Construction, Event, Native, NativeLimits, Progress, Route, Sample, StageTimes};

/// Independent experimental construction refusal dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Cumulative construction, deduplication and publication operations.
    Work,
    /// Nodes in one emitted guard DAG.
    Nodes,
    /// Nodes across successfully retained guards.
    TotalNodes,
    /// Distinct retained witness guards.
    Guards,
    /// A single construction's named vector capacity.
    BuildBytes,
    /// Retained guard vectors plus live construction capacity.
    LiveBytes,
}

/// An incomplete experiment. No completion event follows this error; preceding
/// samples and the failed observation's completed prefix retain their meaning.
#[derive(Debug)]
pub enum Error {
    /// The caller requested a population outside the finite study.
    Configuration(&'static str),
    /// Guard construction reached its own inclusive ceiling.
    Limit(Resource),
    /// Native control or reference evaluation stopped.
    Control(zetesis_cpu::Stop),
    /// Fallible experiment vector reservation failed.
    Allocation,
    /// Named capacity arithmetic cannot be represented.
    Overflow,
    /// A candidate or witness has a foreign original owner.
    Owner,
    /// Guard learning was not supplied a native countermodel record.
    Witness,
    /// The finite formula graph was not admitted.
    Admission(zetesis_ferraris::AdmissionError),
    /// Native membership, candidate enumeration or restriction did not complete.
    Native(zetesis_sat::Incomplete),
    /// The complete family or conditional restriction disagreed with its reference.
    Parity,
    /// The compiled finite DAG or native receipt violated an internal invariant.
    Invariant,
    /// The synchronous event writer refused a record.
    Output(io::Error),
    /// Reporting a prior failed prefix also failed; neither cause is discarded.
    FailureOutput {
        /// Original operation failure.
        original: Box<Error>,
        /// Secondary writer failure.
        output: io::Error,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => write!(f, "feedback experiment: {message}"),
            Self::Limit(resource) => write!(f, "feedback construction {resource:?} limit reached"),
            Self::Control(error) => error.fmt(f),
            Self::Allocation => f.write_str("feedback experiment allocation failed"),
            Self::Overflow => f.write_str("feedback experiment capacity overflow"),
            Self::Owner => f.write_str("feedback subject belongs to a different theory"),
            Self::Witness => f.write_str("feedback learning requires a checked native countermodel"),
            Self::Admission(error) => error.fmt(f),
            Self::Native(error) => error.fmt(f),
            Self::Parity => f.write_str("feedback restriction or complete family disagrees"),
            Self::Invariant => f.write_str("feedback experiment invariant violated"),
            Self::Output(error) => error.fmt(f),
            Self::FailureOutput { original, output } => write!(f, "{original}; failure publication: {output}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Control(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::FailureOutput { original, .. } => Some(original.as_ref()),
            _ => None,
        }
    }
}

fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| Error::Allocation)?;
    Ok(values)
}
fn reference_limits(max_work: u64) -> zetesis_ferraris::Limits {
    zetesis_ferraris::Limits { max_work, max_subsets: 64 }
}

/// Run the fixed study and serialize its synchronous event view as JSON lines.
///
/// # Errors
/// Returns invalid limits, incomplete native/reference/guard work, parity or
/// writer failure. A nonzero process exit does not denote a complete acquisition.
pub fn run(options: &Options, output: &mut impl io::Write) -> Result<(), Error> {
    measure(&options.configuration()?, |event| {
        serde_json::to_writer(&mut *output, event).map_err(io::Error::other)?;
        writeln!(output)
    })
}
