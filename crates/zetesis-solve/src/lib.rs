//! Composed answer-set solving through the reduct.
//!
//! Sessions consume coherent admitted inputs and expose checked answer sets,
//! complete world views, bounded execution and typed observations. Source loading,
//! argument parsing and answer publication belong to their consumers.
//!
//! # Begin with a coherent input
//!
//! Use [`PreparedInput::admitted`] for a source-admitted relational program,
//! [`PreparedInput::formula`] for its finite formula counterpart, or
//! [`PreparedInput::program`] for a native relational program. The input borrows
//! one owner; atom indexes, objectives and the original subject cannot be mixed
//! across unrelated admissions. Preparing this borrow performs no execution.
//!
//! ```
//! use zetesis_cpu::Cancellation;
//! use zetesis_solve::{Backend, PreparedInput, Session, SolveConfig, WorldViewLimits};
//! use zetesis_themelios::{AdmissionOptions, admit};
//!
//! let admitted = admit("a :- not b. b :- not a.".into(), AdmissionOptions::default())?;
//! let config = SolveConfig { backend: Backend::Cpu, models: 0, ..Default::default() };
//! let family = Session::builder(
//!     PreparedInput::admitted(&admitted), config, Cancellation::default(),
//! ).collect(WorldViewLimits::default())?;
//! assert_eq!(family.len(), 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Choose the question and retain its evidence
//!
//! [`Session::new`] selects objective incumbents when an objective exists.
//! [`Session::enumerate`] streams all original answer sets with their scores.
//! Both yield privately constructed [`AnswerSet`] values. Their full atoms and
//! [`Subject`] identity are independent of a source's displayed projection.
//! [`Session::progress`] records current evidence; [`Session::outcome`] becomes
//! available when search finishes, stops or fails. Completion can remain absent
//! after failure. Neither successful iteration
//! nor a score alone establishes exhaustive coverage or proved optimality.
//!
//! [`SessionBuilder::collect`] starts unrestricted enumeration and returns a
//! [`WorldView`] only after complete capture within [`WorldViewLimits`]. A failed
//! collection retains a checked prefix in [`WorldViewFailure`]. A complete empty
//! family establishes inconsistency; a stopped empty prefix does not.
//!
//! # Compose execution effects
//!
//! [`SolveConfig`] controls execution and named resource budgets. The GPU feature
//! enables wgpu and is on by default; CPU execution remains available without it.
//! [`SessionBuilder`] optionally takes [`ExecutionResources`] and
//! [`SolveMeasurements`]. Shared device infrastructure can retain an exact
//! compiled formula profile, while every session owns fresh search, residency,
//! budgets and results. No global device or pipeline cache is installed.
//!
//! Membership is always decided by zetesis's own reduct check, on the CPU or on
//! a qualified GPU route; the session owns candidate coverage, exact residual
//! completion and objectives.
//!
//! [`ExecutionObserver`] borrows typed facts during setup or a pull. Callback
//! failures are distinct from device faults and cannot request backend fallback.
//! [`SolveFailure`] preserves the original cause and available semantic evidence;
//! [`SolveFailure::into_parts`] transfers them into a consumer's error type.
//! Output publication belongs to that consumer and cannot strengthen the solve's
//! evidence. Host measurements are optional attempted-work records, not proofs.
//!
//! Normal programs require agreement with the reduct's least consequence set
//! and its constraints. General formulas require original satisfaction and no
//! proper-subset model of the frozen reduct. The lower libraries implement these
//! checks; this crate composes them with candidate enumeration and honest bounded
//! outcomes. Lean semantic laws do not yet certify the complete Rust/WGSL path.
#![forbid(unsafe_code)]

mod execution_observation;
mod execution_resources;
mod batch_executor;
mod policy;
mod engine;
mod optimization;
mod objective_bounds;
mod formula_execution;
#[cfg(feature = "gpu")]
mod formula_tight;
mod completion_accounting;
mod formula_queue;
mod solve_config;
mod semantic_outcome;
mod lazy_execution;
mod shared_execution;
mod closure_execution;
mod query_observation;
pub use query_observation::QueryExecutionObservation;
mod closure_session;
mod formula_session;
mod model_construction;
mod hybrid_regions;
mod hybrid_session;
mod terminal_session;
mod session;
mod world_view;
mod projection;
mod error;
mod completion;
mod countermodel;
mod phase_timing;
mod stage_timing;
mod grounding_timing;

pub use batch_executor::ExecutorError;
pub use closure_execution::{ClosureExecutionStatistics, ClosureJoinStatistics, ClosureRoute};
pub use completion::{Completion, Interruption, SearchState};
pub use error::{FailureParts, SolveError, SolveFailure};
pub use execution_observation::{ExecutionObservation, ExecutionObserver, StreamedConstraints};
pub use execution_resources::ExecutionResources;
pub use formula_execution::{
    CompletionAccounting, FormulaDeviceLimits, FormulaExecutionStatistics,
    FormulaResidualStatistics,
};
pub use grounding_timing::{GroundingMeasurement, GroundingTimings};
pub use hybrid_session::HybridExecutionStatistics;
pub use lazy_execution::{
    LazyBufferUsage, LazyExecutionStatistics, LazyTransportReplacements, LazyTransportUsage,
};
pub use model_construction::{ModelConstructionStatistics, ModelConstructionStop};
pub use optimization::{Optimization, OptimizationStop};
pub use phase_timing::{PhaseTimings, SolvePhase};
pub use policy::{Grounder, Oracle, SourceBatching};
pub use projection::{ProjectionError, ProjectionLimits, ProjectionResource, ProjectionStatistics};
pub use semantic_outcome::{AnswerSelection, SemanticOutcome};
pub use session::{
    AnswerSet, PreparedInput, PreparedProfile, Session, SessionBuilder, SessionModel, Subject,
};
pub use shared_execution::SharedExecutionStatistics;
pub use solve_config::SolveConfig;
pub use terminal_session::TerminalExecutionStatistics;
pub use world_view::{
    WorldView, WorldViewError, WorldViewFailure, WorldViewFailureParts, WorldViewLimits,
};
pub use zetesis_backend::{Backend, GpuApi};
pub use zetesis_sat::BatchVerdict;
pub use zetesis_sat::SearchMethod;
pub use zetesis_telemetry::{GroundingMode, SolveStage, StageMeasurement, StageTimings};
pub use zetesis_themelios::{GroundingOutcome, GroundingPhase, GroundingWork};

mod measurements;
#[cfg(test)]
mod test_support;
pub use measurements::{MeasurementSpan, SolveMeasurements};
