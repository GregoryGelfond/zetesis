//! Optional complete-theory certificates under the enumeration budget.

#[path = "certified/positive.rs"]
mod positive;
#[path = "certified/types.rs"]
mod types;
pub use types::{
    CertificateError, CertificateLimits, CertificateOrder, CertificatePlanStatistics,
    CertifiedStatistics,
};

use super::{StableModels, Statistics};
use crate::search::{Budget, LocalQuota, increment};
use crate::timing::{self, Phase};
use crate::{AdmissionError, Control, Incomplete, Limits, SearchStatistics};
use zetesis_ferraris::{
    Interpretation, PositiveError, PositivePlan, PositivePlanLimits, PositiveResource,
    TightCheckLimits, TightError, TightPlan, TightPlanLimits, TightResource, TightVerdict,
};

#[derive(Debug)]
pub(super) enum Certification {
    Tight {
        plan: TightPlan,
        max_bytes: u64,
    },
    Positive {
        plan: PositivePlan,
        max_bytes: usize,
    },
}

#[derive(Clone, Copy)]
enum Kind {
    Tight,
    Positive,
}

/// Only checked evidence from the actual selected plan produces these values.
pub(super) enum Verdict {
    Stable,
    NotModel,
    /// A present atom has no producer with a true body under the complete
    /// tight plan. By `TightPlans.stable_supported` the candidate is not an
    /// answer set: the candidate without that atom models its reduct.
    Unsupported {
        atom: usize,
    },
}

impl StableModels {
    /// Try only a tight normal/choice certificate for the original theory.
    /// This compatibility operation never selects the positive-cycle algorithm.
    /// Repeated configuration retains the first attempt, including refusal.
    ///
    /// # Errors
    /// Refuses late/closed configuration, shared control/allocation failures or
    /// cumulative work exhaustion. Failed setup never establishes coverage.
    pub fn enable_certified_checking(
        &mut self,
        limits: TightPlanLimits,
    ) -> Result<bool, Incomplete> {
        self.configure_certificates(
            CertificateLimits {
                tight: limits,
                ..Default::default()
            },
            &[Kind::Tight],
        )
    }

    /// Try complete-original-theory class certificates in the requested order.
    /// The order is only a scheduling hint; each plan checks every original root.
    /// A positive plan installs exact original-atom units for its least model, or
    /// an empty clause for a violated original constraint. Original theory and
    /// reduct semantics are unchanged. Units are transactional and consume the
    /// existing candidate CNF admission; no restriction formula DAG is copied.
    ///
    /// Optional shape/storage refusals leave the next plan and general reduct
    /// checking available. All preparation, failed attempts, unit construction
    /// and candidate checking consume the same cumulative search work. Invoke
    /// before enumeration; repeated calls never replace the first configuration
    /// or replenish a budget. Candidate restrictions retain the selected owner.
    /// Injected GPU checkers should use an enumeration without this CPU policy.
    ///
    /// # Errors
    /// Refuses late/closed configuration, shared control/allocation failures or
    /// cumulative work exhaustion. Partially appended units roll back on failure.
    pub fn enable_class_checking(
        &mut self,
        limits: CertificateLimits,
        order: CertificateOrder,
    ) -> Result<bool, Incomplete> {
        let kinds = match order {
            CertificateOrder::TightFirst => [Kind::Tight, Kind::Positive],
            CertificateOrder::PositiveFirst => [Kind::Positive, Kind::Tight],
        };
        self.configure_certificates(limits, &kinds)
    }

    fn configure_certificates(
        &mut self,
        limits: CertificateLimits,
        kinds: &[Kind],
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
        let mut stats = CertifiedStatistics::default();
        let result = (|| {
            for kind in kinds {
                let prepared = match kind {
                    Kind::Tight => self.prepare_tight(limits.tight, &mut stats),
                    Kind::Positive => self.prepare_positive(limits.positive, &mut stats),
                }?;
                if let Some(plan) = prepared {
                    stats.plan = Some(match &plan {
                        Certification::Tight { plan, .. } => {
                            CertificatePlanStatistics::Tight(plan.statistics())
                        }
                        Certification::Positive { plan, .. } => {
                            CertificatePlanStatistics::Positive(plan.statistics())
                        }
                    });
                    stats.refusal = None;
                    self.certification = Some(plan);
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        self.statistics.certified = Some(stats);
        if result.is_err() {
            self.terminal = true;
        }
        result
    }

    fn remaining_certificate_work(&self) -> u64 {
        self.limits
            .search
            .max_work
            .saturating_sub(self.statistics.search.work)
    }

    fn prepare_tight(
        &mut self,
        mut limits: TightPlanLimits,
        stats: &mut CertifiedStatistics,
    ) -> Result<Option<Certification>, Incomplete> {
        limits.max_work = limits.max_work.min(self.remaining_certificate_work());
        let attempt = TightPlan::compile_accounted(&self.theory, limits, &self.control);
        self.statistics.search.work += attempt.work;
        stats.construction_work += attempt.work;
        match attempt.result {
            Ok(plan) => Ok(Some(Certification::Tight {
                plan,
                max_bytes: limits.max_bytes,
            })),
            Err(error) => {
                stats.tight_refusal = Some(error);
                stats.refusal = Some(CertificateError::Tight(error));
                match error {
                    TightError::Stopped(stop) => Err(stop.into()),
                    TightError::Limit(TightResource::Work)
                        if self.remaining_certificate_work() == 0 =>
                    {
                        Err(Incomplete::WorkLimit)
                    }
                    _ => Ok(None),
                }
            }
        }
    }

    fn prepare_positive(
        &mut self,
        mut limits: PositivePlanLimits,
        stats: &mut CertifiedStatistics,
    ) -> Result<Option<Certification>, Incomplete> {
        limits.max_work = limits.max_work.min(self.remaining_certificate_work());
        let attempt = PositivePlan::compile_accounted(&self.theory, limits, &self.control);
        self.statistics.search.work += attempt.statistics.work;
        stats.construction_work += attempt.statistics.work;
        stats.positive_attempt = Some(attempt.statistics);
        let plan = match attempt.result {
            Ok(plan) => plan,
            Err(error) => {
                stats.positive_refusal = Some(error);
                stats.refusal = Some(CertificateError::Positive(error));
                return match error {
                    PositiveError::InvalidClosure { .. } => Err(Incomplete::InvalidWitness),
                    PositiveError::Stopped(stop) => Err(stop.into()),
                    PositiveError::Limit {
                        resource: PositiveResource::Work,
                        ..
                    } if self.remaining_certificate_work() == 0 => Err(Incomplete::WorkLimit),
                    _ => Ok(None),
                };
            }
        };
        let mut budget = Budget {
            quota: LocalQuota,
            limits: self.limits.search,
            control: &self.control,
            statistics: self.statistics.search,
        };
        // The units are a candidate-only restriction in clause form; the
        // regions proposer reads the plan's consequences through the
        // theory's own readings and needs none.
        let result = match &mut self.proposer {
            super::Proposer::Clauses(clauses) => {
                positive::restrict(&plan, &mut clauses.cnf, &mut budget)
            }
            super::Proposer::Regions(_) => Ok(0),
        };
        stats.restriction_work = budget.statistics.work - self.statistics.search.work;
        self.statistics.search = budget.statistics;
        match result {
            Ok(clauses) => {
                stats.restriction_clauses = clauses;
                Ok(Some(Certification::Positive {
                    plan,
                    max_bytes: limits.max_bytes,
                }))
            }
            Err(Incomplete::Admission(
                error @ (AdmissionError::Limit { .. } | AdmissionError::Overflow),
            )) => {
                stats.restriction_refusal = Some(error);
                stats.refusal = Some(CertificateError::Restriction(error));
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

/// Only the coordinator classifies certificates. Joined workers receive the
/// remaining quota after these actual attempts have been charged.
pub(super) fn classify(
    certificate: &Certification,
    candidate: &Interpretation,
    limits: Limits,
    control: &Control,
    statistics: &mut Statistics,
    search: &mut SearchStatistics,
) -> Result<Verdict, Incomplete> {
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
) -> Result<Verdict, Incomplete> {
    let stats = statistics
        .certified
        .as_mut()
        .ok_or(Incomplete::InvalidWitness)?;
    increment(&mut stats.checks)?;
    let before = search.work;
    let result = match certificate {
        Certification::Tight { plan, max_bytes } => {
            check_tight(plan, candidate, *max_bytes, limits, control, search)
        }
        Certification::Positive { plan, max_bytes } => {
            let (result, peak) =
                positive::check(plan, candidate, *max_bytes, limits, control, search);
            stats.positive_check_peak_bytes =
                Some(stats.positive_check_peak_bytes.unwrap_or(0).max(peak));
            result
        }
    };
    stats.checking_work = stats
        .checking_work
        .checked_add(search.work - before)
        .ok_or(Incomplete::CounterOverflow)?;
    match &result {
        Ok(Verdict::Stable) => increment(&mut stats.stable)?,
        Ok(Verdict::Unsupported { .. }) => increment(&mut stats.refuted)?,
        Ok(Verdict::NotModel) => {}
        Err(_) => increment(&mut stats.failed)?,
    }
    result
}

fn check_tight(
    plan: &TightPlan,
    candidate: &Interpretation,
    max_bytes: u64,
    limits: Limits,
    control: &Control,
    search: &mut SearchStatistics,
) -> Result<Verdict, Incomplete> {
    let remaining = limits.search.max_work.saturating_sub(search.work);
    let attempt = plan.check_accounted(
        candidate,
        TightCheckLimits {
            max_bytes,
            max_work: remaining.min(limits.max_verification_work),
        },
        control,
    );
    search.work += attempt.work;
    match attempt.result {
        Ok(check) => Ok(match check.verdict {
            TightVerdict::Stable => Verdict::Stable,
            TightVerdict::NotModel { .. } => Verdict::NotModel,
            TightVerdict::Residual { unsupported_atom } => Verdict::Unsupported {
                atom: unsupported_atom,
            },
        }),
        Err(TightError::Stopped(stop)) => Err(stop.into()),
        Err(TightError::Limit(TightResource::Work))
            if remaining <= limits.max_verification_work =>
        {
            Err(Incomplete::WorkLimit)
        }
        Err(error) => Err(Incomplete::Certificate(CertificateError::Tight(error))),
    }
}

#[cfg(test)]
#[path = "../tests/support/positive_restriction.rs"]
mod positive_restriction_tests;
