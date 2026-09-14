//! Exact lazy reduct checking over source templates, streamed complete candidate
//! generation, an explicit dense static oracle, and a bounded owned Rayon pool.
//! The lazy oracle joins only derived tuples. The static oracle accepts an
//! already compiled graph; neither oracle silently performs static compilation.
//! Batched lazy rounds retain one appendable atom catalog and borrow canonical
//! committed rows while extending a disjoint identity tail. Catalog membership,
//! frozen seed membership and derived truth remain separate authorities.
#![forbid(unsafe_code)]

mod control;
mod oracle;
mod static_oracle;
mod candidates;
mod batch;
mod verified;
pub mod lazy;
pub mod table;

pub use batch::{BatchError, BatchOracle};
pub use candidates::{
    CandidateLimits, CandidateRestrictionLimits, CandidateStatistics, CandidateTermination,
    Candidates,
};
pub use control::{Control, Stop};
pub use oracle::source;
pub use oracle::{Check, ClosureWorkspace, Limits, PreparationLimits, PreparationStatistics, PreparedQueries, Statistics, check, check_view};
pub use static_oracle::{StaticCheck, StaticStatistics, check_static, check_static_view};
pub use verified::StableInterpretation;
