//! Bounded CPU finite-table experiments with complete row/domain outputs.
//!
//! A direct typed scan, prepared scalar bitsets and shared Rayon bitsets receive
//! identical immutable rows and ordered domains. Preparation and per-query
//! conversion are measured separately. This optional primitive experiment neither
//! grounds a source program nor establishes ASP support or answer-set truth.

mod config;
mod fixture;
mod run;
mod view;

use std::{fmt, io};

pub use config::{Case, Configuration, Options};
pub use fixture::{Subject, TypedValue};
pub use run::measure;
pub use view::{Event, Outcome, Output, Phase, Preparation, Receipt, Route};

/// An invalid command, failed acquisition or interrupted experiment.
/// Finite-table applicability refusals are retained as events instead.
#[derive(Debug)]
pub enum Error {
    /// A requested dimension or schedule exceeds the declared finite population.
    Configuration(&'static str),
    /// A source atom could not be constructed.
    Atom(zetesis_core::ConstructionError),
    /// A structural fixture value could not be constructed.
    Value(zetesis_core::ValueError),
    /// The authoritative relation could not be prepared.
    Relation(zetesis_core::relation::Failure),
    /// A table invariant, allocation or non-limit operation failed.
    Table(zetesis_cpu::table::Failure),
    /// Cancellation or the caller's deadline interrupted acquisition/execution.
    Stopped(zetesis_cpu::Stop),
    /// A bounded fixture or result allocation could not be reserved.
    Allocation,
    /// The independently owned Rayon pool could not be constructed.
    Pool(rayon::ThreadPoolBuildError),
    /// Complete row/domain results differ from the independent source reference.
    Parity,
    /// Report serialization or its sink failed; prior bytes remain.
    Output(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(f, "table experiment: {reason}"),
            Self::Atom(error) => error.fmt(f),
            Self::Value(error) => error.fmt(f),
            Self::Relation(error) => error.fmt(f),
            Self::Table(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
            Self::Allocation => f.write_str("table experiment allocation refused"),
            Self::Pool(error) => error.fmt(f),
            Self::Parity => f.write_str("table projection disagrees with complete source rows"),
            Self::Output(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Atom(error) => Some(error),
            Self::Value(error) => Some(error),
            Self::Relation(error) => Some(error),
            Self::Table(error) => Some(error),
            Self::Stopped(error) => Some(error),
            Self::Pool(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Configuration(_) | Self::Allocation | Self::Parity => None,
        }
    }
}

/// Run a bounded command and stream complete JSON-lines events.
///
/// # Errors
/// Invalid configuration, stopped acquisition or failed output returns an error.
/// A completed schedule containing applicability refusals returns `Ok(false)`;
/// it is not silently reclassified as a successful comparison.
pub fn run(options: &Options, output: &mut impl io::Write) -> Result<bool, Error> {
    use io::Write as _;
    let configuration = options.configuration()?;
    let mut writer = view::BoundedWriter::new(output, config::MAX_REPORT_BYTES);
    measure(configuration, &zetesis_cpu::Control::default(), |event| {
        serde_json::to_writer(&mut writer, event).map_err(io::Error::other)?;
        writeln!(writer)
    })
}

#[cfg(test)]
mod tests;
