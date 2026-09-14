//! Bounded native Boolean search and finite Ferraris stable-model enumeration.
//!
//! The SAT kernel uses iterative chronological DPLL and two watched literals.
//! Candidate formulas and frozen reducts receive full Tseitin equivalences;
//! only original semantic atoms enter minimality and model blocking. This
//! crate does not parse, ground, translate source aggregates, invoke an
//! external solver, or claim a refinement proof for its Rust implementation.
#![forbid(unsafe_code)]

mod cnf;
mod clauses;
mod error;
mod projection;
mod search;
mod ordering;
mod encoding;
mod ferraris;
mod timing;
mod checked;
pub use zetesis_ferraris::partition;

pub use checked::{CheckedInterpretation, StableInterpretation, check_interpretation};

pub use clauses::{Clause, Clauses};
pub use cnf::{AdmissionError, AdmissionLimits, Assignment, Cnf, Literal, Resource};
pub use error::Incomplete;
pub use ferraris::{
    BatchError, BatchLimits, BatchStatistics, BatchVerdict, CertifiedStatistics, Check,
    CompletionExecutor, CompletionScratch, CompletionStatistics, Limits, StableModels, Statistics,
    SupportStatistics, SupportStatus, check,
};
pub use projection::{ProjectionLimits, ProjectionResource, ProjectionStatistics};
pub use search::{SearchLimits, SearchStatistics, Solve, solve, solve_with_statistics};
pub use zetesis_cpu::Control;

pub use timing::{PhaseMeasurement, SearchPhaseTimings};
