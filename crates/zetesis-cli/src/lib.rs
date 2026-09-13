//! Source loading, presentation and command-line adapters for zetesis.
//!
//! [`Session`] is re-exported from the canonical `zetesis-solve` library.
//! [`run_finalized`] consumes that library and retains semantic evidence separately
//! from publication in [`PublicationReport`] or [`PublicationFailure`].
//! [`run`] accepts source and an injected output sink. Parsing, exact oracles,
//! and candidate enumeration remain reusable libraries. [`entry`] adapts these
//! operations to process arguments, standard streams, and exit codes.
#![forbid(unsafe_code)]

mod options;
mod driver;
mod failure;
mod display;
mod admission;
mod devices;
mod process;
mod presentation;
mod publication;
mod statistics;
mod phase_timing;
mod stage_timing;
mod output;

#[cfg(test)]
#[path = "../tests/support/bounded_writer.rs"]
mod test_writer;

pub use devices::devices;
pub use driver::{
    Report, RunError, run, run_bundle_detailed_with_diagnostics,
    run_bundle_finalized_with_diagnostics, run_bundle_with_diagnostics, run_detailed,
    run_detailed_with_diagnostics, run_finalized, run_finalized_with_diagnostics,
    run_with_diagnostics,
};
pub use failure::{PartialReport, RunFailure};
pub use options::{Command, Options};
pub use presentation::ColorMode;
pub use process::entry;

mod grounding_timing;
pub use zetesis_themelios::{GroundingOutcome, GroundingPhase, GroundingWork};

pub use zetesis_telemetry::{GroundingMode, SolveStage, StageMeasurement, StageTimings};

mod finalized;
pub use finalized::{Publication, PublicationFailure, PublicationReport};

// Compatibility exports preserve the canonical solver types, not another implementation.
pub use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, CompletionAccounting, ExecutionObservation,
    ExecutionObserver, ExecutionResources, FormulaDeviceLimits, FormulaExecutionStatistics,
    Grounder, GroundingMeasurement, GroundingTimings, Interruption, LazyBufferUsage,
    LazyExecutionStatistics, LazyTransportReplacements, LazyTransportUsage, MeasurementSpan,
    Optimization, OptimizationStop, Oracle, PhaseTimings, PreparedInput, PreparedProfile,
    SearchState, SemanticOutcome, Session, SessionBuilder, SessionModel, SharedExecutionStatistics, SolveConfig,
    SolveError, SolveFailure, SolveMeasurements, SolvePhase, SourceBatching, Subject, WorldView,
    WorldViewError, WorldViewFailure, WorldViewLimits,
};
