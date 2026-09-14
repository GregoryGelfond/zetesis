//! Exact least-model restriction and independent membership validation.

use super::{CertificateError, Verdict};
use crate::search::{Budget, LocalQuota};
use crate::{Cnf, Control, Incomplete, Limits, Literal, SearchStatistics};
use zetesis_ferraris::{
    EvaluationError, EvaluationLimits, EvaluationWorkspace, Interpretation, PositivePlan,
};

/// Only original semantic atom IDs enter these units. No new formula owner or
/// auxiliary gates are constructed. The original CNF remains unchanged on error;
/// admitted work and allocated capacity remain visible after rollback.
pub(super) fn restrict(
    plan: &PositivePlan,
    cnf: &mut Cnf,
    budget: &mut Budget<'_>,
) -> Result<usize, Incomplete> {
    let checkpoint = cnf.checkpoint();
    let result = (|| {
        if plan.failed_constraint().is_some() {
            budget.tick()?;
            crate::encoding::clause(cnf, [], budget)?;
            return Ok(1);
        }
        let least = plan.least_consequences();
        for atom in 0..plan.theory().atom_count() {
            // One charged identity/membership read and one charged literal write.
            budget.tick()?;
            crate::encoding::clause(cnf, [Literal::new(atom, least.contains(atom))], budget)?;
        }
        Ok(plan.theory().atom_count())
    })();
    if result.is_err() {
        cnf.rollback(checkpoint);
    }
    result
}

pub(super) fn check(
    plan: &PositivePlan,
    candidate: &Interpretation,
    max_bytes: usize,
    limits: Limits,
    control: &Control,
    search: &mut SearchStatistics,
) -> (Result<Verdict, Incomplete>, u128) {
    let remaining = limits.search.max_work.saturating_sub(search.work);
    let mut budget = Budget {
        quota: LocalQuota,
        limits: crate::SearchLimits {
            max_work: search
                .work
                .saturating_add(remaining.min(limits.max_verification_work)),
            ..limits.search
        },
        control,
        statistics: *search,
    };
    let mut peak = plan.statistics().retained_bytes;
    let result = validate(plan, candidate, max_bytes, &mut budget, &mut peak);
    *search = budget.statistics;
    let result = match result {
        Err(Incomplete::WorkLimit) if limits.max_verification_work < remaining => {
            Err(Incomplete::Verification(zetesis_cpu::Stop::WorkLimit))
        }
        other => other,
    };
    (result, peak)
}

fn validate(
    plan: &PositivePlan,
    candidate: &Interpretation,
    max_bytes: usize,
    budget: &mut Budget<'_>,
    peak: &mut u128,
) -> Result<Verdict, Incomplete> {
    budget.control.poll()?;
    if !plan.theory().same_instance(candidate.theory()) {
        return Err(Incomplete::WrongTheory);
    }
    // The transactional complete unit restriction admits only this exact
    // interpretation. A mismatching proposal is a broken search witness, not a
    // reason to relabel an arbitrary candidate as certified or nonminimal.
    if plan.failed_constraint().is_some() {
        return Err(Incomplete::InvalidWitness);
    }
    for atom in 0..plan.theory().atom_count() {
        budget.tick()?;
        if candidate.contains(atom) != plan.least_consequences().contains(atom) {
            return Err(Incomplete::InvalidWitness);
        }
    }
    let available = (max_bytes as u128)
        .checked_sub(plan.statistics().retained_bytes)
        .ok_or(Incomplete::InvalidWitness)?;
    let mut evaluation = EvaluationWorkspace::default();
    let attempt = evaluation.evaluate(
        candidate,
        EvaluationLimits {
            max_work: budget
                .limits
                .max_work
                .saturating_sub(budget.statistics.work),
            max_bytes: usize::try_from(available).map_err(|_| Incomplete::CounterOverflow)?,
        },
        budget.control,
    );
    budget.statistics.work += attempt.work;
    *peak = (*peak).max(plan.statistics().retained_bytes + attempt.retained_bytes);
    match attempt.result {
        Ok(truth) if truth.is_model() => Ok(Verdict::Stable),
        Ok(_) => Ok(Verdict::NotModel),
        Err(EvaluationError::Stopped(zetesis_cpu::Stop::WorkLimit)) => Err(Incomplete::WorkLimit),
        Err(EvaluationError::Stopped(stop)) => Err(stop.into()),
        Err(error @ EvaluationError::Storage { .. }) => {
            Err(Incomplete::Certificate(CertificateError::Evaluation(error)))
        }
    }
}
