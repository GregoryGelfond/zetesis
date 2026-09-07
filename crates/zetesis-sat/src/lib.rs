//! Bounded native Boolean search and finite Ferraris stable-model enumeration.
//!
//! The SAT kernel uses iterative chronological DPLL and two watched literals.
//! Candidate formulas and frozen reducts receive full Tseitin equivalences;
//! only original semantic atoms enter minimality and model blocking. This
//! crate does not parse, ground, translate aggregates, optimize, invoke an
//! external solver, or claim a refinement proof for its Rust implementation.
#![forbid(unsafe_code)]

mod cnf;
mod error;
mod search;
mod ordering;
mod encoding;
mod ferraris;
mod timing;

pub use cnf::{AdmissionError, AdmissionLimits, Assignment, Cnf, Literal, Resource};
pub use error::Incomplete;
pub use ferraris::{
    BatchError, BatchLimits, BatchStatistics, BatchVerdict, Check, CompletionExecutor,
    CompletionScratch, CompletionStatistics, Limits, StableModels, Statistics, check,
};
pub use search::{SearchLimits, SearchStatistics, Solve, solve, solve_with_statistics};
pub use zetesis_cpu::Control;

pub use timing::{PhaseMeasurement, SearchPhaseTimings};
