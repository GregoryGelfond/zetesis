//! Bounded relational evaluation of lifted minimization contributions.
//!
//! The caller supplies a complete model and remains responsible for proving
//! stability. Objective conditions only read that model: they never derive
//! atoms, contribute support or decide stable-model acceptance. No global
//! domain, external solver or complete objective grounding is constructed.
#![forbid(unsafe_code)]

mod program;
mod score;
mod error;
mod evaluate;
mod condition;

pub use condition::{Condition, ConditionNode};
pub use error::{Error, ErrorKind, Limits, Statistics, Stop};
pub use evaluate::evaluate;
pub use program::{
    AdmissionError, AdmissionLimits, AdmissionResource, ObjectiveProgram, ObjectiveTemplate,
    WeightPolarity,
};
pub use score::{Contribution, Evaluation, Score};
