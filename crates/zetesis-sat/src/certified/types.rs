//! Public evidence for the selected complete-theory checking algorithm.

use crate::AdmissionError;
use std::fmt;
use zetesis_ferraris::{
    EvaluationError, PositiveError, PositivePlanLimits, PositivePlanStatistics, TightError,
    TightPlanLimits, TightPlanStatistics,
};

/// Independent optional construction ceilings for both complete-theory plans.
#[derive(Clone, Copy, Debug, Default)]
pub struct CertificateLimits {
    /// Ranked normal/choice support preparation and candidate scratch.
    pub tight: TightPlanLimits,
    /// Positive least-consequence preparation and candidate scratch.
    pub positive: PositivePlanLimits,
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
}

/// The actual selected plan; neither variant describes the other algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificatePlanStatistics {
    /// Complete ranked-support plan.
    Tight(TightPlanStatistics),
    /// Complete positive least-consequence plan.
    Positive(PositivePlanStatistics),
}

/// Optional construction refusal or failed certified membership operation.
/// None of these errors establishes inconsistency or exhaustive coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateError {
    /// Ranked-support classification, construction or checking refusal.
    Tight(TightError),
    /// Positive classification or least-consequence construction refusal.
    Positive(PositiveError),
    /// The exact least-model units could not fit candidate CNF admission.
    /// No part of the attempted restriction remains in the CNF.
    Restriction(AdmissionError),
    /// Independent original-theory evaluation failed. A storage limit here is
    /// the remaining workspace allowance after the retained positive plan.
    Evaluation(EvaluationError),
}

impl fmt::Display for CertificateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tight(error) => error.fmt(formatter),
            Self::Positive(error) => error.fmt(formatter),
            Self::Restriction(error) => write!(formatter, "least-model restriction: {error}"),
            Self::Evaluation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CertificateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Tight(error) => error,
            Self::Positive(error) => error,
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
    /// Exact unit-restriction admission refusal, even if another plan succeeds.
    pub restriction_refusal: Option<AdmissionError>,
    /// Sum of actual plan-construction work, including every refused attempt.
    pub construction_work: u64,
    /// Exact least-model restriction work, including rolled-back attempts.
    /// Included in cumulative search work, separate from plan construction.
    pub restriction_work: u64,
    /// Successfully committed original-atom units, or one empty candidate contradiction.
    /// Zero after a failed transactional restriction attempt.
    pub restriction_clauses: usize,
    /// Candidate checks entered, including interrupted attempts.
    pub checks: u64,
    /// Completed certificate proofs of stability, before publication or commit.
    pub stable: u64,
    /// Candidates the complete tight plan refuted for an unsupported present
    /// atom, by the support law, without a reduct query. Every candidate a
    /// selected certificate checks is decided: stable, refuted or not a model.
    pub refuted: u64,
    /// Interrupted checks; no membership verdict was produced.
    pub failed: u64,
    /// Candidate-check work, including interrupted attempts.
    pub checking_work: u64,
    /// Maximum actual retained positive plan plus local evaluation workspace
    /// capacity during an entered positive check, including failures. Absent
    /// when that route was never entered; excludes proposed/refused growth,
    /// shared theory, allocator overhead and other stack state.
    pub positive_check_peak_bytes: Option<u128>,
}
