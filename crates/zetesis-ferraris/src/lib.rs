//! Finite formula reducts and exact subset-minimality checking.
//!
//! This general kernel complements the Horn closure specialization. It does not
//! parse source or ground rules. Its finite scalar aggregate constructor accepts
//! already coalesced tuple eligibility formulas. Its topological
//! Boolean transforms implement the definitions in `Zetesis.Ferraris` in the
//! repository's Lean specification; this Rust implementation is not verified by
//! those proofs. The CPU dependency supplies cancellation and deadline control.

#![forbid(unsafe_code)]

mod theory;
mod oracle;
mod normal;
mod aggregate;
mod tight;
mod checked;

pub use checked::{CheckedInterpretation, StableInterpretation, check_interpretation};

pub use aggregate::{
    AggregateBuild, AggregateComparison, AggregateElement, AggregateError, AggregateErrorKind,
    AggregateExtremum, AggregateFamilyBuild, AggregateFamilyLimits, AggregateGuard,
    AggregateLimits, AggregateProfile, AggregateStatistics, ExtremumBound, ValueExtremumElement,
    append_aggregate, append_aggregate_family, append_extremum, append_value_extremum,
};

pub use normal::{from_ground_program, from_ground_program_supported};
pub use oracle::{Check, Limits, Statistics, Verdict, check, models, models_reduct};
pub use theory::{AdmissionError, AdmissionLimits, Interpretation, Node, Theory};

pub use tight::{
    TightAttempt, TightCheck, TightCheckLimits, TightError, TightPlan, TightPlanLimits,
    TightPlanStatistics, TightProducer, TightProducerKind, TightResource, TightVerdict,
};
