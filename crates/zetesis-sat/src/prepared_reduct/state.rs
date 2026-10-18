//! The enumeration coordinator owns preparation; workers borrow its result.

use zetesis_ferraris::{Interpretation, Theory};

use super::{PreparedReduct, ReductWorkspace};
use crate::timing::{self, Phase};
use crate::{
    Check, Incomplete, Limits, SearchMethod, Statistics,
    ferraris::{IndexedTheory, ReductQuery},
    search::{Budget, Quota, increment},
};

/// The membership machinery of one enumeration or worker: under the clause
/// kernel a prepared reduct encoding, under regions nothing of its own. The
/// regions method's proper-subset query reads the original theory's index,
/// which this state never holds or builds: its owner, the enumeration's or the
/// standalone check's `OriginalIndex`, lends it to each check. Mutable
/// evaluation and query knowledge remain private to each check or worker.
#[derive(Debug)]
pub(crate) struct State {
    method: SearchMethod,
    prepared: Option<PreparedReduct>,
    pub(crate) workspace: ReductWorkspace,
}

impl State {
    pub(crate) fn new(method: SearchMethod) -> Self {
        Self {
            method,
            prepared: None,
            workspace: ReductWorkspace::default(),
        }
    }

    pub(crate) fn prepared(&self) -> Option<&PreparedReduct> {
        self.prepared.as_ref()
    }

    /// Prepare the clause kernel's reduct encoding on first use. The regions
    /// method has nothing to prepare here.
    pub(crate) fn ensure(
        &mut self,
        theory: &Theory,
        limits: Limits,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut Statistics,
    ) -> Result<(), Incomplete> {
        if self.method == SearchMethod::Regions {
            return Ok(());
        }
        if let Some(prepared) = &self.prepared {
            return if prepared.theory().same_instance(theory) {
                Ok(())
            } else {
                Err(Incomplete::WrongTheory)
            };
        }
        let started = timing::start(statistics.phase_timings.as_ref());
        let attempt = PreparedReduct::with_budget(
            theory,
            limits.reduct_admission,
            limits.max_reduct_bytes,
            budget,
        );
        timing::finish(
            &mut statistics.phase_timings,
            Phase::ReductPreparation,
            started,
        );
        statistics.reduct.preparation = Some(attempt.statistics);
        let prepared = attempt.result?;
        self.prepared = Some(prepared);
        Ok(())
    }

    /// Decide a classical candidate's membership. Under the regions method
    /// the proper-subset query reads `index`, lent by its owner; a regions
    /// check lent none is refused as an invalid witness, since every
    /// regions candidate comes from a walk that built the index.
    pub(crate) fn check(
        &mut self,
        theory: &Theory,
        index: Option<&IndexedTheory>,
        candidate: &Interpretation,
        limits: Limits,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut Statistics,
    ) -> Result<Check, Incomplete> {
        budget.cancellation.poll()?;
        if !theory.same_instance(candidate.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        self.ensure(theory, limits, budget, statistics)?;
        if self.method == SearchMethod::Regions {
            let index = index.ok_or(Incomplete::InvalidWitness)?;
            // Authenticate the lent index before any evaluation work.
            index.subject(theory)?;
            let query = ReductQuery::new(index);
            let result = (|| {
                let mut evaluated =
                    self.workspace
                        .evaluate(candidate, limits, budget.cancellation, statistics)?;
                if !evaluated.truth.is_model() {
                    return Ok(Check::NotModel);
                }
                let started = timing::start(statistics.phase_timings.as_ref());
                increment(&mut statistics.countermodel_queries)?;
                let result =
                    match positive_closure(&mut evaluated, candidate, limits, budget, statistics) {
                        Ok(Some(verdict)) => Ok(verdict),
                        Ok(None) => query.check(
                            zetesis_ferraris::FrozenSubject::new(theory, evaluated.truth.truth()),
                            candidate,
                            limits,
                            budget,
                            statistics,
                            evaluated.narrowing,
                        ),
                        Err(error) => Err(error),
                    };
                timing::finish(&mut statistics.phase_timings, Phase::Reduct, started);
                result
            })();
            return self
                .workspace
                .finish_check(result, limits.max_reduct_bytes, statistics);
        }
        let prepared = self.prepared.as_ref().ok_or(Incomplete::InvalidWitness)?;
        prepared.check_with(candidate, &mut self.workspace, limits, budget, statistics)
    }
}

/// Try exact least closure before asking the general proper-subset query.
/// Its accounted prefix is retained on every result; declined grammar alone
/// falls through. A strict-subset result uses the ordinary witness validator.
fn positive_closure(
    evaluated: &mut super::query::RegionEvaluation<'_>,
    candidate: &Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
) -> Result<Option<Check>, Incomplete> {
    use zetesis_ferraris::{
        EvaluationError, EvaluationLimits, FrozenPositiveOutcome, FrozenPositiveWorkspace,
    };

    increment(&mut statistics.reduct.frozen_positive_attempts)?;
    let wanted = u64::try_from(FrozenPositiveWorkspace::work_bound(candidate.theory()))
        .map_err(|_| Incomplete::CounterOverflow)?;
    let available = u128::from(limits.max_reduct_bytes)
        .checked_sub(evaluated.positive_other_bytes)
        .ok_or(Incomplete::ReductStorage {
            required: evaluated.positive_other_bytes,
            limit: u128::from(limits.max_reduct_bytes),
        })?;
    let cancellation = budget.cancellation;
    let attempt = budget.with_reservation(wanted, |granted| {
        let attempt = evaluated.positive.check(
            &evaluated.truth,
            EvaluationLimits {
                max_work: granted,
                max_bytes: usize::try_from(available).unwrap_or(usize::MAX),
            },
            cancellation,
        );
        let used = attempt.work;
        (attempt, used)
    })?;
    statistics.reduct.frozen_positive_work = statistics
        .reduct
        .frozen_positive_work
        .checked_add(attempt.work)
        .ok_or(Incomplete::CounterOverflow)?;
    statistics.reduct.frozen_positive_preparation_work = statistics
        .reduct
        .frozen_positive_preparation_work
        .checked_add(attempt.preparation_work)
        .ok_or(Incomplete::CounterOverflow)?;
    statistics.reduct.peak_workspace_bytes = statistics
        .reduct
        .peak_workspace_bytes
        .max(evaluated.positive_other_bytes + attempt.retained_bytes);
    let outcome = attempt.result.map_err(|error| match error {
        EvaluationError::Stopped(zetesis_cpu::Stop::WorkLimit) => Incomplete::WorkLimit,
        EvaluationError::Stopped(stop) => Incomplete::from(stop),
        EvaluationError::Storage { required, .. } => Incomplete::ReductStorage {
            required: evaluated.positive_other_bytes + required,
            limit: u128::from(limits.max_reduct_bytes),
        },
    })?;
    let verdict = match outcome {
        FrozenPositiveOutcome::Declined => return Ok(None),
        FrozenPositiveOutcome::NotModel => Check::NotModel,
        FrozenPositiveOutcome::Stable => Check::Stable,
        FrozenPositiveOutcome::NonMinimal(subset) => crate::ferraris::checked_countermodel(
            candidate.theory(),
            candidate,
            subset,
            limits,
            budget,
            statistics,
        )?,
    };
    increment(&mut statistics.reduct.frozen_positive_completed)?;
    Ok(Some(verdict))
}

#[cfg(test)]
mod tests;
