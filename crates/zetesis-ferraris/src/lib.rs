//! Finite formula reducts and exact subset-minimality checking.
//!
//! This general kernel complements the Horn closure specialization. It does not
//! parse source or ground rules. Its finite scalar aggregate constructor accepts
//! already coalesced tuple eligibility formulas. Its topological
//! Boolean transforms implement the definitions in `Zetesis.Ferraris` in the
//! repository's Lean specification; this Rust implementation is not verified by
//! those proofs. The CPU dependency supplies cancellation and deadline control.

#![forbid(unsafe_code)]

mod aggregate;
mod atomic_choice;
mod checked;
mod evaluation;
mod extremum_cut;
pub mod extremum_lowering;
mod frozen_positive;
mod normal;
mod numeric_evaluation;
mod operand_evaluation;
mod oracle;
pub mod partition;
mod positive;
mod reduct;
mod regions;
mod stratified;
mod support;
#[cfg(test)]
mod test_support;
mod theory;
mod threshold;
pub mod threshold_lowering;
mod tight;

pub use extremum_cut::{ExtremaMeasure, ExtremumCut, ExtremumCutRule, ExtremumRow};

pub use checked::{CheckedInterpretation, StableInterpretation, check_interpretation};

pub use aggregate::native as native_aggregate;
pub use aggregate::{
    AggregateBuild, AggregateComparison, AggregateElement, AggregateError, AggregateErrorKind,
    AggregateExtremum, AggregateFamilyBuild, AggregateFamilyLimits, AggregateGuard,
    AggregateLimits, AggregateProfile, AggregateStatistics, ExtremumBound, FormulaNodes,
    ValueExtremumElement, ValueExtremumFamilyAttempt, ValueExtremumFamilySpace, ValueExtremumGuard,
    append_aggregate, append_aggregate_family, append_extremum, append_value_extremum,
    append_value_extremum_family_refs, append_value_extremum_refs,
};

pub use evaluation::{
    EvaluationAttempt, EvaluationError, EvaluationLimits, EvaluationWorkspace, FormulaEvaluation,
};
pub use frozen_positive::{FrozenPositiveAttempt, FrozenPositiveOutcome, FrozenPositiveWorkspace};
pub use normal::{from_ground_program, from_ground_program_supported};
pub use oracle::{Check, Limits, Statistics, Verdict, check, models, models_reduct};
pub use reduct::FrozenReduct;
pub use regions::{
    ChoiceHeads, Extraction, FrozenSubject, GeneratorOrder, GeneratorOrderAttempt,
    GeneratorOrderError, GeneratorOrderLimits, Knowledge, NARROWING_BATCH, Narrower,
    NarrowingAttempt, NarrowingQuota, NarrowingScratch, NarrowingStatistics, OriginalSubject,
    PreferenceAttempt, Producers, RegionLimits, producers,
};
pub use theory::{
    AdmissionError, AdmissionLimits, ContributionRow, ExtremumCutDescriptor, FormulaParts,
    FormulaSuffix, FormulaTransaction, FormulaView, Interpretation, InterpretationWords, Node,
    NodeView, NumericDomain, NumericRow, OperandSpan, Theory, TheoryAdmission, Threshold,
    ThresholdDescriptor,
};
pub use zetesis_cpu::regions::{Narrowing, Region};

pub use tight::{
    TightAttempt, TightCheck, TightCheckLimits, TightError, TightPlan, TightPlanLimits,
    TightPlanStatistics, TightProducer, TightProducerKind, TightResource, TightVerdict,
    TightWorkspace,
};

pub use support::{SupportAttempt, SupportError, SupportLimits, support_restriction};

pub use positive::{
    PositiveAttempt, PositiveError, PositivePlan, PositivePlanLimits, PositivePlanStatistics,
    PositiveResource,
};

pub use stratified::{
    StratifiedAttempt, StratifiedError, StratifiedPlan, StratifiedPlanLimits,
    StratifiedPlanStatistics, StratifiedResource,
};
