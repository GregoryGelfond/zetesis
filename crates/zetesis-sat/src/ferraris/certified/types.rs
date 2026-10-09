//! Public evidence for the selected complete-theory checking algorithm.

use crate::AdmissionError;
use std::fmt;
use zetesis_ferraris::{
    EvaluationError, PositiveError, PositivePlanLimits, PositivePlanStatistics, StratifiedError,
    StratifiedPlanLimits, StratifiedPlanStatistics, TightError, TightPlanLimits,
    TightPlanStatistics,
};

/// Independent optional construction ceilings for complete-theory plans.
#[derive(Clone, Copy, Debug, Default)]
pub struct CertificateLimits {
    /// Ranked normal/choice support preparation and candidate scratch.
    pub tight: TightPlanLimits,
    /// Positive least-consequence preparation and candidate scratch.
    pub positive: PositivePlanLimits,
    /// Stratified normal evaluation and candidate scratch.
    pub stratified: StratifiedPlanLimits,
}

/// Which complete-original-theory classifier to try first.
/// A source analysis hint may choose order, but cannot authenticate a plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertificateOrder {
    /// Try ranked normal/choice support before positive least consequences.
    #[default]
    TightFirst,
    /// Try positive least consequences before ranked normal/choice support.
    PositiveFirst,
    /// Try direct stratified evaluation before ranked support and positive closure.
    StratifiedFirst,
}

/// The actual selected complete-theory algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificatePlanStatistics {
    /// Complete ranked-support plan.
    Tight(TightPlanStatistics),
    /// Complete positive least-consequence plan.
    Positive(PositivePlanStatistics),
    /// Complete stratified normal evaluation.
    Stratified(StratifiedPlanStatistics),
}

/// Optional construction refusal or failed certified membership operation.
/// None of these errors establishes inconsistency or exhaustive coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateError {
    /// Ranked-support classification, construction or checking refusal.
    Tight(TightError),
    /// Positive classification or least-consequence construction refusal.
    Positive(PositiveError),
    /// Stratified classification or direct evaluation refusal.
    Stratified(StratifiedError),
    /// The exact determined-interpretation units could not fit candidate CNF admission.
    /// No part of the attempted restriction remains in the CNF.
    Restriction(AdmissionError),
    /// Independent original-theory evaluation failed. A storage limit here is
    /// the remaining workspace allowance after the retained selected plan.
    Evaluation(EvaluationError),
}

impl fmt::Display for CertificateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tight(error) => error.fmt(formatter),
            Self::Positive(error) => error.fmt(formatter),
            Self::Stratified(error) => error.fmt(formatter),
            Self::Restriction(error) => write!(formatter, "determined-answer restriction: {error}"),
            Self::Evaluation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CertificateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Tight(error) => error,
            Self::Positive(error) => error,
            Self::Stratified(error) => error,
            Self::Restriction(error) => error,
            Self::Evaluation(error) => error,
        })
    }
}

/// Attempted certificate construction and candidate checking, before batch commit.
/// Only the enumeration's `stable_models` counter records publication/commit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CertifiedStatistics {
    /// Successfully selected complete-theory certificate, if eligible.
    pub plan: Option<CertificatePlanStatistics>,
    /// Last optional refusal when no plan was selected; absent on success.
    pub refusal: Option<CertificateError>,
    /// Actual ranked-plan refusal, retained even if the positive plan succeeds.
    pub tight_refusal: Option<TightError>,
    /// Actual positive-plan refusal, retained even if the ranked plan succeeds.
    pub positive_refusal: Option<PositiveError>,
    /// Actual positive construction receipt, including a refused work/storage prefix.
    /// A successful primitive may still be followed by a restriction refusal.
    pub positive_attempt: Option<PositivePlanStatistics>,
    /// Actual stratified-plan refusal, including unsupported signed recursion.
    pub stratified_refusal: Option<StratifiedError>,
    /// Actual stratified construction receipt, including a failed prefix.
    pub stratified_attempt: Option<StratifiedPlanStatistics>,
    /// Exact unit-restriction admission refusal, even if another plan succeeds.
    pub restriction_refusal: Option<AdmissionError>,
    /// Sum of actual plan-construction work, including every refused attempt.
    pub construction_work: u64,
    /// Exact determined-interpretation restriction work, including rolled-back attempts.
    /// Included in cumulative search work, separate from plan construction.
    pub restriction_work: u64,
    /// Successfully committed original-atom units, or one empty candidate contradiction.
    /// Zero after a failed transactional restriction attempt.
    pub restriction_clauses: usize,
    /// Candidate checks entered, including interrupted attempts.
    pub checks: u64,
    /// Completed certificate proofs of stability, before publication or commit.
    pub stable: u64,
    /// Candidates a complete certificate refuted: an unsupported present atom
    /// under the tight plan, or an original model different from the completely
    /// determined interpretation, without a reduct query. Every candidate a
    /// selected certificate checks is decided: stable, refuted or not a model.
    pub refuted: u64,
    /// Interrupted checks; no membership verdict was produced.
    pub failed: u64,
    /// Candidate-check work, including interrupted attempts.
    pub checking_work: u64,
    /// Maximum retained tight-plan and local evaluation-vector payload during
    /// an entered tight check, including failures. Absent when never entered;
    /// excludes shared theory, vector headers and allocator overhead. Parallel
    /// enumeration reports the largest worker value, not their sum.
    pub tight_check_peak_bytes: Option<u128>,
    /// Maximum actual retained positive plan plus local evaluation workspace
    /// capacity during an entered positive check, including failures. Absent
    /// when that route was never entered; excludes proposed/refused growth,
    /// shared theory, allocator overhead and other stack state.
    pub positive_check_peak_bytes: Option<u128>,
    /// Retained stratified plan plus independent candidate evaluation capacity.
    pub stratified_check_peak_bytes: Option<u128>,
}
