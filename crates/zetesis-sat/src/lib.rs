//! Bounded native Boolean search and finite Ferraris stable-model enumeration.
//!
//! Candidates are proposed either by regions of the theory's atoms narrowed
//! by its readings, with no clause form, or by the SAT kernel's iterative
//! chronological DPLL with two watched literals over a Tseitin encoding of
//! the theory. Frozen reducts receive full Tseitin equivalences; only
//! original semantic atoms enter minimality and model blocking. This
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
mod prepared_reduct;
pub use prepared_reduct::{
    PreparedReduct, ReductPreparationAttempt, ReductPreparationLimits, ReductPreparationStatistics,
    ReductQueryStatistics, ReductStatistics, ReductWorkspace,
};
pub use zetesis_ferraris::partition;

pub use checked::{CheckedInterpretation, StableInterpretation, check_interpretation};

pub use clauses::{Clause, Clauses};
pub use cnf::{AdmissionError, AdmissionLimits, Assignment, Cnf, Literal, Resource};
pub use error::Incomplete;
pub use ferraris::{
    BatchError, BatchLimits, BatchStatistics, BatchVerdict, CertificateError, CertificateLimits,
    CertificateOrder, CertificatePlanStatistics, CertifiedStatistics, Check, CompletionExecutor,
    CompletionScratch, CompletionStatistics, Limits, RegionCounts, RegionFrontierStatistics,
    RegionSearchStatistics, SearchMethod, StableModels, Statistics, SupportStatistics,
    SupportStatus, check, check_with,
};
pub use projection::{ProjectionLimits, ProjectionResource, ProjectionStatistics};
pub use search::{SearchLimits, SearchStatistics, Solve, solve, solve_with_statistics};
pub use zetesis_cpu::Cancellation;

pub use timing::{PhaseMeasurement, SearchPhaseTimings};
