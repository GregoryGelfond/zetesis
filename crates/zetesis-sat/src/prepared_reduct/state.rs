//! The enumeration coordinator owns preparation; workers borrow its result.

use std::sync::Arc;

use zetesis_ferraris::{Interpretation, Theory};

use super::{PreparedReduct, ReductWorkspace};
use crate::timing::{self, Phase};
use crate::{
    Check, Incomplete, Limits, SearchMethod, Statistics,
    ferraris::{IndexedTheory, ReductQuery},
    search::{Budget, Quota, increment},
};

/// The membership machinery of one enumeration: under the clause kernel a
/// prepared reduct encoding, under regions the theory's index for the
/// proper-subset query. Enumeration shares its already charged candidate index;
/// standalone membership prepares an index on first use. Mutable evaluation
/// and query knowledge remain private to each check or worker.
#[derive(Debug)]
pub(crate) struct State {
    method: SearchMethod,
    prepared: Option<PreparedReduct>,
    query: Option<ReductQuery>,
    pub(crate) workspace: ReductWorkspace,
}

impl State {
    pub(crate) fn new(method: SearchMethod) -> Self {
        Self {
            method,
            prepared: None,
            query: None,
            workspace: ReductWorkspace::default(),
        }
    }

    /// Attach an original index whose construction the enumeration already paid.
    pub(crate) fn with_index(index: Arc<IndexedTheory>) -> Self {
        Self {
            query: Some(ReductQuery::from_index(index)),
            ..Self::new(SearchMethod::Regions)
        }
    }

    pub(crate) fn prepared(&self) -> Option<&PreparedReduct> {
        self.prepared.as_ref()
    }

    pub(crate) fn query(&self) -> Option<&ReductQuery> {
        self.query.as_ref()
    }

    pub(crate) fn ensure(
        &mut self,
        theory: &Theory,
        limits: Limits,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut Statistics,
    ) -> Result<(), Incomplete> {
        if self.method == SearchMethod::Regions {
            if let Some(query) = &self.query {
                return if query.theory().same_instance(theory) {
                    Ok(())
                } else {
                    Err(Incomplete::WrongTheory)
                };
            }
            let index = IndexedTheory::new(theory);
            let work = index.narrower().work();
            budget.charge(work)?;
            statistics.reduct.regions.work = statistics
                .reduct
                .regions
                .work
                .checked_add(work)
                .ok_or(Incomplete::CounterOverflow)?;
            self.query = Some(ReductQuery::from_index(Arc::new(index)));
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

    pub(crate) fn check(
        &mut self,
        theory: &Theory,
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
        if let Some(query) = &self.query {
            let (truth, _) =
                self.workspace
                    .evaluate(candidate, limits, budget.cancellation, statistics)?;
            if !truth.is_model() {
                return Ok(Check::NotModel);
            }
            let started = timing::start(statistics.phase_timings.as_ref());
            increment(&mut statistics.countermodel_queries)?;
            let result = query.check(theory, candidate, truth.truth(), limits, budget, statistics);
            timing::finish(&mut statistics.phase_timings, Phase::Reduct, started);
            return result;
        }
        let prepared = self.prepared.as_ref().ok_or(Incomplete::InvalidWitness)?;
        prepared.check_with(candidate, &mut self.workspace, limits, budget, statistics)
    }
}
