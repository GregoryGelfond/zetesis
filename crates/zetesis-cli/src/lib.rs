//! Streaming command-line orchestration for zetesis.
//!
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

#[cfg(test)]
#[path = "../tests/support/bounded_writer.rs"]
mod test_writer;

pub use optimization::{Optimization, OptimizationStop};

pub use devices::devices;
pub use driver::{
    Completion, Interruption, Report, RunError, run, run_bundle_detailed_with_diagnostics,
    run_bundle_with_diagnostics, run_detailed, run_detailed_with_diagnostics, run_with_diagnostics,
};
pub use failure::{PartialReport, RunFailure};
pub use formula_execution::{CompletionAccounting, FormulaExecutionStatistics};
pub use options::{Backend, Command, Grounder, Options, Oracle};
pub use process::entry;

pub use phase_timing::{PhaseTimings, SolvePhase};

pub use zetesis_telemetry::{GroundingMode, SolveStage, StageMeasurement, StageTimings};
