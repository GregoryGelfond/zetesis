//! Reusable ordinary-solve sessions and command-line adapters for zetesis.
//!
//! [`Session`] consumes coherent prepared owners without argument parsing or output
//! writers. [`run_finalized`] retains semantic evidence separately from publication.
//! [`run`] accepts source and an injected output sink. Parsing, exact oracles,
//! and candidate enumeration remain reusable libraries. [`entry`] adapts these
//! operations to process arguments, standard streams, and exit codes.
#![forbid(unsafe_code)]

mod options;
mod driver;
mod failure;
mod display;
mod admission;
mod engine;
mod devices;
mod process;
mod presentation;
mod countermodel;
mod optimization;
mod objective_bounds;
mod statistics;
mod formula_execution;
mod completion_accounting;
mod phase_timing;
mod stage_timing;
mod formula_queue;
mod output;
mod solve_config;
mod semantic_outcome;

#[cfg(test)]
#[path = "../tests/support/bounded_writer.rs"]
mod test_writer;

pub use optimization::{Optimization, OptimizationStop};

pub use devices::devices;
pub use driver::{
    Completion, Interruption, Report, RunError, run, run_bundle_detailed_with_diagnostics,
    run_bundle_finalized_with_diagnostics, run_bundle_with_diagnostics, run_detailed,
    run_detailed_with_diagnostics, run_finalized, run_finalized_with_diagnostics,
    run_with_diagnostics,
};
pub use failure::{PartialReport, RunFailure};
pub use formula_execution::{CompletionAccounting, FormulaExecutionStatistics};
mod lazy_execution;
pub use lazy_execution::LazyExecutionStatistics;
pub use options::{Backend, Command, Grounder, Options, Oracle};
pub use presentation::ColorMode;
pub use process::entry;
pub use semantic_outcome::SemanticOutcome;
pub use solve_config::SolveConfig;

mod grounding_timing;
pub use grounding_timing::{GroundingMeasurement, GroundingTimings};
pub use phase_timing::{PhaseTimings, SolvePhase};
pub use zetesis_themelios::{GroundingOutcome, GroundingPhase, GroundingWork};

pub use zetesis_telemetry::{GroundingMode, SolveStage, StageMeasurement, StageTimings};

mod closure_session;
mod formula_session;
mod finalized;
pub use finalized::{Publication, SolveFailure, SolveReport};

mod session;
pub use session::{PreparedInput, PreparedProfile, Session, SessionModel, Subject};
