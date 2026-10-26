//! Finite Ferraris stable-model enumeration by region search.
//!
//! Candidates are regions of the theory's atoms narrowed by its readings, with
//! no clause form; every leaf is a classical model of the theory. Membership is
//! the frozen reduct's: its proper-subset query is walked as a region tree over
//! the same index, and a countermodel is validated independently before it
//! refutes a candidate. Only original semantic atoms enter minimality. This
//! crate does not parse, ground, translate source aggregates, invoke an
//! external solver, or claim a refinement proof for its Rust implementation.
#![forbid(unsafe_code)]

mod budget;
mod error;
mod ferraris;
mod region_filter;
mod timing;
pub use region_filter::{
    RegionConsequence, RegionFeasibility, RegionFilter, RegionFilterStatistics, RegionFilterWorker,
    RegionPass,
};
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
    CompletionStatistics, Engagement, FrontierOrder, GeneratorOrderStatistics,
    GeneratorOrderStatus, Limits, Lookahead, LookaheadDeclined, RegionCounts,
    RegionFrontierStatistics, RegionSearchStatistics, RegionWalkEngagement, RegionWorkerStatistics,
    SlateProposal, SlateTicket, StableModels, Statistics, check,
};
pub use zetesis_cpu::Cancellation;

pub use timing::{PhaseMeasurement, SearchPhaseTimings};
