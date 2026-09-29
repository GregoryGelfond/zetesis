//! Source loading, presentation and command-line adapters for zetesis.
//!
//! [`Session`] is re-exported from the canonical `zetesis-solve` library.
//! [`run_finalized`] consumes that library and retains semantic evidence separately
//! from publication in [`PublicationOutcome`] or [`PublicationFailure`].
//! [`run`] accepts source and an injected output sink. Parsing, exact oracles,
//! and candidate enumeration remain reusable libraries. [`entry`] adapts these
//! operations to process arguments, standard streams, and exit codes.
//! [`publish_prepared`] and [`AnswerRenderer`] provide a replaceable typed view
//! without argument parsing or complete-family buffering. [`HumanRenderer`] and
//! [`JsonRenderer`] use that same contract; semantic evidence stays independent
//! of the renderer's acknowledgement.
#![forbid(unsafe_code)]

mod options;
mod command;
pub mod testing;
pub mod statistics_view;
pub use command::Invocation;
pub use statistics_view::StatisticsView;
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
mod view;

pub use view::{
    AnswerRenderer, AnswerView, HumanRenderer, JsonRenderer, PublicationConfig, PublicationView,
    SummaryDelivery, SummaryStage,
};

pub use devices::devices;
pub use driver::{
    Report, RunError, publish_prepared, run, run_bundle_detailed_with_diagnostics,
    run_bundle_finalized_with_diagnostics, run_bundle_with_diagnostics, run_bundle_with_renderer,
    run_detailed, run_detailed_with_diagnostics, run_finalized, run_finalized_with_diagnostics,
    run_with_diagnostics, run_with_renderer,
};
pub use failure::{PartialReport, RunFailure};
pub use options::{Command, Options};
pub use presentation::ColorMode;
pub use process::entry;

mod grounding_timing;
pub use zetesis_themelios::{GroundingOutcome, GroundingPhase, GroundingWork};

pub use zetesis_telemetry::{GroundingMode, SolveStage, StageMeasurement, StageTimings};

mod finalized;
pub use finalized::{
    Publication, PublicationFailure, PublicationOutcome, PublicationPhase, PublicationReport,
    PublicationStop, StoppedPublication,
};

// Compatibility exports preserve the canonical solver types, not another implementation.
pub use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, ClosureExecutionStatistics, ClosureJoinStatistics,
    ClosureRoute, Completion, CompletionAccounting, ExecutionObservation, ExecutionObserver,
    ExecutionResources, FormulaDeviceLimits, FormulaExecutionStatistics, FormulaResidualStatistics,
    Grounder, GroundingMeasurement, GroundingTimings, Interruption, LazyBufferUsage,
    LazyExecutionStatistics, LazyTransportReplacements, LazyTransportUsage, MeasurementSpan,
    Optimization, OptimizationStop, Oracle, PhaseTimings, PreparedInput, PreparedProfile,
    QueryExecutionObservation, SearchMethod, SearchState, SemanticOutcome, Session, SessionBuilder,
    SessionModel, SharedExecutionStatistics, SolveConfig, SolveError, SolveFailure,
    SolveMeasurements, SolvePhase, SourceBatching, Subject, WorldView, WorldViewError,
    WorldViewFailure, WorldViewFailureParts, WorldViewLimits,
};
