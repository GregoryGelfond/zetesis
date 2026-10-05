//! zetesis's single-shot backend for the canonical Rust solve and query APIs.
//!
//! [`Solver`] implements [`themelios_solve::contract::Backend`]. Both admitted source and
//! constructed programs enter through the same canonical program value. Lowering
//! validates and retains that value; each solve prepares and enumerates its full
//! answer sets using zetesis's CPU reduct machinery. Objectives and `#project`
//! do not restrict this enumeration. `#show` affects display alone.
//!
//! The adapter declares enumeration, cooperative request deadlines and
//! cancellation. The query tier derives consequences from complete enumeration.
//! Optimization, assumptions, theory propagation, external functions and named
//! program parts are outside this initial backend profile. Native GPU sessions
//! remain available through `zetesis_solve::Session`.
//!
//! Configuration ceilings are resource faults, never exhaustion or a request
//! time budget. Full enumeration can be stopped by the caller without establishing
//! a complete world view. The general ASP laws and native proof work do not yet
//! constitute end-to-end verification of this adapter.
#![forbid(unsafe_code)]

mod backend;
mod config;
mod faults;
mod model;
mod prepared;
mod run;

pub use backend::Solver;
pub use config::{Config, Grounder};
pub use model::{OutputError, OutputLimits, SymbolStop};
