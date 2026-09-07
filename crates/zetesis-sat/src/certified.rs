//! Optional complete-theory support certificates under the enumeration budget.

use zetesis_ferraris::{
    Interpretation, TightCheckLimits, TightError, TightPlan, TightPlanLimits, TightPlanStatistics,
    TightResource, TightVerdict,
};

use super::{StableModels, Statistics};
use crate::search::increment;
use crate::timing::{self, Phase};
use crate::{Control, Incomplete, Limits, SearchStatistics};

/// Attempted certificate construction and candidate checking, before batch commit.
/// A completed stable decision may still be pending after an unrelated batch
/// failure. Only the enumeration's `stable_models` counter records commit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CertifiedStatistics {
    /// Successfully checked complete-theory certificate, if eligible.
    pub plan: Option<TightPlanStatistics>,
    /// Exact reason construction was refused; never evidence of inconsistency.
    pub refusal: Option<TightError>,
    /// Construction work, including work spent before a refusal.
    pub construction_work: u64,
    /// Candidate checks entered, including interrupted attempts.
    pub checks: u64,
    /// Completed ranked-support proofs of stability, before publication or commit.
    pub stable: u64,
    /// Completed checks requiring exact countermodel completion.
    pub residuals: u64,
    /// Interrupted checks; no membership verdict was produced.
    pub failed: u64,
    /// Candidate-check work, including interrupted attempts.
    pub checking_work: u64,
}

#[derive(Debug)]
pub(super) struct Certification {
    plan: TightPlan,
    max_bytes: u64,
}

impl StableModels {
    /// Try a tight normal/choice certificate for the immutable original theory.
    /// Call before candidate enumeration. Repeated calls retain the original
    /// attempt, including refusal, and never restart a resource budget.
    ///
    /// Shape, cycle and optional construction limits leave exact reduct checking
    /// available. Construction and candidate checks consume the same cumulative
    /// work quota as outer search and residual completion. Candidate restrictions
    /// never change this certificate's original theory. Injected GPU checkers
    /// should use an enumeration without this optional CPU specialization.
    ///
    /// # Errors
    /// Refuses late configuration, terminal enumeration, shared cancellation,
    /// allocation and exhaustion of the cumulative work quota. Failed setup
    /// cannot establish exhaustive coverage.
    pub fn enable_certified_checking(
        &mut self,
        mut limits: TightPlanLimits,
    ) -> Result<bool, Incomplete> {
        if self.terminal {
            return Err(Incomplete::ClosedEnumerator);
        }
        if self.statistics.certified.is_some() {
            return Ok(self.certification.is_some());
        }
        if self.statistics.candidate_queries != 0 {
            return Err(Incomplete::LateCertificate);
        }
        limits.max_work = limits.max_work.min(
            self.limits
                .search
                .max_work
                .saturating_sub(self.statistics.search.work),
        );
        let attempt = TightPlan::compile_accounted(&self.theory, limits, &self.control);
        // The primitive admitted at most the remaining quota. Charge even a
        // refusal before any possible fallback is selected.
        self.statistics.search.work += attempt.work;
        let mut stats = CertifiedStatistics {
            construction_work: attempt.work,
            ..Default::default()
        };
        let result = match attempt.result {
            Ok(plan) => {
                stats.plan = Some(plan.statistics());
                self.certification = Some(Certification {
                    plan,
                    max_bytes: limits.max_bytes,
                });
                Ok(true)
            }
            Err(error) => {
                stats.refusal = Some(error);
                match error {
                    TightError::Stopped(stop) => Err(stop.into()),
                    TightError::Limit(TightResource::Work)
                        if self.statistics.search.work == self.limits.search.max_work =>
                    {
                        Err(Incomplete::WorkLimit)
                    }
                    _ => Ok(false),
                }
            }
        };
        self.statistics.certified = Some(stats);
        if result.is_err() {
            self.terminal = true;
        }
        result
    }
}

/// Only the enumeration coordinator calls this function. Joined Rayon workers
/// receive the remaining quota after this attempt has been charged.
pub(super) fn classify(
    certificate: &Certification,
    candidate: &Interpretation,
    limits: Limits,
    control: &Control,
    statistics: &mut Statistics,
    search: &mut SearchStatistics,
) -> Result<TightVerdict, Incomplete> {
    let started = timing::start(statistics.phase_timings.as_ref());
    let result = evaluate(certificate, candidate, limits, control, statistics, search);
    timing::finish(&mut statistics.phase_timings, Phase::Certified, started);
    result
}

fn evaluate(
    certificate: &Certification,
    candidate: &Interpretation,
    limits: Limits,
    control: &Control,
    statistics: &mut Statistics,
    search: &mut SearchStatistics,
) -> Result<TightVerdict, Incomplete> {
    let stats = statistics
        .certified
        .as_mut()
        .ok_or(Incomplete::InvalidWitness)?;
    increment(&mut stats.checks)?;
    let remaining = limits.search.max_work.saturating_sub(search.work);
    let attempt = certificate.plan.check_accounted(
        candidate,
        TightCheckLimits {
            max_bytes: certificate.max_bytes,
            max_work: remaining.min(limits.max_verification_work),
        },
        control,
    );
    search.work += attempt.work;
    stats.checking_work = stats
        .checking_work
        .checked_add(attempt.work)
        .ok_or(Incomplete::CounterOverflow)?;
    match attempt.result {
        Ok(check) => {
            match check.verdict {
                TightVerdict::Stable => increment(&mut stats.stable)?,
                TightVerdict::Residual { .. } => increment(&mut stats.residuals)?,
                TightVerdict::NotModel { .. } => {}
            }
            Ok(check.verdict)
        }
        Err(error) => {
            increment(&mut stats.failed)?;
            Err(match error {
                TightError::Stopped(stop) => stop.into(),
                TightError::Limit(TightResource::Work)
                    if remaining <= limits.max_verification_work =>
                {
                    Incomplete::WorkLimit
                }
                _ => Incomplete::Certificate(error),
            })
        }
    }
}
