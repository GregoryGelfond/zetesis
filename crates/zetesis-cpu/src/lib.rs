//! Exact lazy reduct checking over source templates, streamed complete candidate
//! generation, an explicit dense static oracle, and a bounded owned Rayon pool.
//! The lazy oracle joins only derived tuples. The static oracle accepts an
//! already compiled graph; neither oracle silently performs static compilation.
#![forbid(unsafe_code)]

mod control;
mod oracle;
mod static_oracle;
mod candidates;
mod batch;

pub use batch::{BatchError, BatchOracle};
pub use candidates::{CandidateLimits, Candidates};
pub use control::{Control, Stop};
pub use oracle::{Check, Limits, Statistics, check};
pub use static_oracle::{StaticCheck, StaticStatistics, check_static};
