//! A uniquely determined interpretation and independent original validation.

use super::{CertificateError, Verdict};
use crate::search::{Budget, LocalQuota};
use crate::{Cancellation, Cnf, Incomplete, Limits, Literal, SearchStatistics};
use zetesis_ferraris::{EvaluationError, EvaluationLimits, EvaluationWorkspace, Interpretation};

/// Borrowed result of a complete positive or stratified producer evaluation.
/// Constraints may exclude this interpretation; they never supply support.
#[derive(Clone, Copy)]
pub(super) struct Determined<'a> {
    pub interpretation: &'a Interpretation,
    pub failed_constraint: Option<usize>,
    pub retained_bytes: u128,
}

/// Only original semantic atom IDs enter these units. No new formula owner or
/// auxiliary gates are constructed. The original CNF remains unchanged on error;
/// admitted work and allocated capacity remain visible after rollback.
pub(super) fn restrict(
    plan: Determined<'_>,
    cnf: &mut Cnf,
    budget: &mut Budget<'_>,
) -> Result<usize, Incomplete> {
    let checkpoint = cnf.checkpoint();
    let result = (|| {
        if plan.failed_constraint.is_some() {
            budget.tick()?;
            crate::encoding::clause(cnf, [], budget)?;
            return Ok(1);
        }
        let least = plan.interpretation;
        for atom in 0..least.theory().atom_count() {
            // One charged identity/membership read and one charged literal write.
            budget.tick()?;
            crate::encoding::clause(cnf, [Literal::new(atom, least.contains(atom))], budget)?;
        }
        Ok(least.theory().atom_count())
    })();
    if result.is_err() {
        cnf.rollback(checkpoint);
    }
    result
}

pub(super) fn check(
    plan: Determined<'_>,
    candidate: &Interpretation,
    max_bytes: usize,
    limits: Limits,
    cancellation: &Cancellation,
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
        cancellation,
        statistics: *search,
    };
    let mut peak = plan.retained_bytes;
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
    plan: Determined<'_>,
    candidate: &Interpretation,
    max_bytes: usize,
    budget: &mut Budget<'_>,
    peak: &mut u128,
) -> Result<Verdict, Incomplete> {
    budget.cancellation.poll()?;
    if !plan
        .interpretation
        .theory()
        .same_instance(candidate.theory())
    {
        return Err(Incomplete::WrongTheory);
    }
    // The complete producer certificate determines the only possible answer.
    // Exact original evaluation still distinguishes non-models from models
    // rejected by that certificate; constraints cannot create another answer.
    let mut is_determined = true;
    for atom in 0..plan.interpretation.theory().atom_count() {
        budget.tick()?;
        is_determined &= candidate.contains(atom) == plan.interpretation.contains(atom);
    }
    let available = (max_bytes as u128)
        .checked_sub(plan.retained_bytes)
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
        budget.cancellation,
    );
    budget.statistics.work += attempt.work;
    *peak = (*peak).max(plan.retained_bytes + attempt.retained_bytes);
    match attempt.result {
        Ok(truth) if !truth.is_model() => Ok(Verdict::NotModel),
        Ok(_) if is_determined => Ok(Verdict::Stable),
        // The certificate excludes other original models. In the stratified
        // case the determined interpretation need not itself be their subset.
        Ok(_) => Ok(Verdict::NonMinimal),
        Err(EvaluationError::Stopped(zetesis_cpu::Stop::WorkLimit)) => Err(Incomplete::WorkLimit),
        Err(EvaluationError::Stopped(stop)) => Err(stop.into()),
        Err(error @ EvaluationError::Storage { .. }) => {
            Err(Incomplete::Certificate(CertificateError::Evaluation(error)))
        }
    }
}
