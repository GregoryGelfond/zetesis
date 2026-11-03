//! Region search for the stable models of finite general-formula (Ferraris)
//! theories.
//!
//! Candidates are regions of the theory's atoms narrowed by its readings, with
//! no clause form; every leaf is a classical model of the theory. Membership is
//! the frozen reduct's: its proper-subset query is walked as a region tree over
//! the same index, and a countermodel is validated independently before it
//! refutes a candidate. Only original semantic atoms enter minimality.
//!
//! A hybrid program's streamed integrity constraints, admitted by
//! zetesis-themelios, are checked over candidate regions by
//! [`StreamedConstraints`] through that crate's checker; no caller code refutes
//! a region or decides an atom. Beyond that check, this crate does not parse,
//! ground, translate source aggregates, invoke an external solver, or claim a
//! refinement proof for its Rust implementation.
//!
//! The admitted source and its consequence representation enter through
//! `streamed_constraints`. Candidate composition applies those consequences;
//! frozen membership reads only the Ferraris theory and its own typed state.
#![forbid(unsafe_code)]

mod budget;
mod error;
mod ferraris;
mod region_filter;
mod subject;
mod streamed_constraints;
mod timing;
pub(crate) use region_filter::RegionFeasibility;
pub use region_filter::RegionFilterStatistics;
pub use streamed_constraints::StreamedConstraints;
mod checked;
mod reduct;
pub use reduct::ReductStatistics;
pub use zetesis_ferraris::partition;

pub use checked::{CheckedInterpretation, StableInterpretation, check_interpretation};

pub use budget::{SearchLimits, SearchStatistics};
pub use error::Incomplete;
pub use ferraris::{
    BatchError, BatchLimits, BatchStatistics, BatchVerdict, CertificateError, CertificateLimits,
    CertificateOrder, CertificatePlanStatistics, CertifiedStatistics, Check, CompletionExecutor,
    CompletionStatistics, ConstructionAttempt, Engagement, FrontierOrder, GeneratorOrderStatistics,
    GeneratorOrderStatus, LearnedStore, Limits, Lookahead, LookaheadDeclined, QueryDivision,
    QueryDivisionStatistics, RegionCounts, RegionFrontierStatistics, RegionSearchStatistics,
    RegionWalkEngagement, RegionWorkerStatistics, RegionalObservations, RegionalOutcome,
    RegionalPass, RegionalQueryObservation, RegionalSlotObservation, RegionalSlotState,
    SlateProposal, SlateTicket, StableModels, Statistics, check,
};
pub use zetesis_algebra::Cancellation;
pub use zetesis_ferraris::{
    CardinalityStatistics, ParityStatistics, PreparationStatus, PreparationWork,
};

pub use timing::{PhaseMeasurement, SearchPhaseTimings};
