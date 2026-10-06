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
            let (truth, scratch) =
                self.workspace
                    .evaluate(candidate, limits, budget.cancellation, statistics)?;
            if !truth.is_model() {
                return Ok(Check::NotModel);
            }
            let started = timing::start(statistics.phase_timings.as_ref());
            increment(&mut statistics.countermodel_queries)?;
            let result = query.check(
                zetesis_ferraris::FrozenSubject::new(theory, truth.truth()),
                candidate,
                limits,
                budget,
                statistics,
                scratch,
            );
            timing::finish(&mut statistics.phase_timings, Phase::Reduct, started);
            return result;
        }
        let prepared = self.prepared.as_ref().ok_or(Incomplete::InvalidWitness)?;
        prepared.check_with(candidate, &mut self.workspace, limits, budget, statistics)
    }
}
