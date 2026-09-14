//! The enumeration coordinator owns preparation; workers borrow its result.

use zetesis_ferraris::{Interpretation, Theory};

use super::{PreparedReduct, ReductWorkspace};
use crate::timing::{self, Phase};
use crate::{
    Check, Incomplete, Limits, Statistics,
    search::{Budget, Quota},
};

#[derive(Debug, Default)]
pub(crate) struct State {
    prepared: Option<PreparedReduct>,
    pub(crate) workspace: ReductWorkspace,
}

impl State {
    pub(crate) fn prepared(&self) -> Option<&PreparedReduct> {
        self.prepared.as_ref()
    }

    pub(crate) fn ensure(
        &mut self,
        theory: &Theory,
        limits: Limits,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut Statistics,
    ) -> Result<(), Incomplete> {
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
        budget.control.poll()?;
        if !theory.same_instance(candidate.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        self.ensure(theory, limits, budget, statistics)?;
        let prepared = self.prepared.as_ref().ok_or(Incomplete::InvalidWitness)?;
        prepared.check_with(candidate, &mut self.workspace, limits, budget, statistics)
    }
}
