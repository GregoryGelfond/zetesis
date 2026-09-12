//! Optional incumbent pruning; no bound enters the original reduct theory.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;

use zetesis_cpu::Control;
use zetesis_objective::Score;
use zetesis_sat::StableModels;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundLimits, ObjectivePlan, ObjectivePlanLimits,
};

use crate::countermodel::Input;
use crate::{SolveConfig, SolveError};

pub(crate) struct Bounds {
    plan: Option<ObjectivePlan>,
    work: u64,
}

impl Bounds {
    pub(crate) fn new(
        input: Input<'_>,
        options: &SolveConfig,
        observations: &mut impl ExecutionSink,
        control: &Control,
    ) -> Result<Self, SolveError> {
        let mut state = Self {
            plan: None,
            work: 0,
        };
        if !input.objectives.is_present() || options.max_objective_bound_work == 0 {
            return Ok(state);
        }
        let limits = ObjectivePlanLimits {
            max_work: options.max_objective_bound_work,
            max_bindings: options.max_objective_bindings,
            max_keys: options.max_objective_keys,
            max_key_bytes: options.max_objective_key_bytes,
            ..Default::default()
        };
        match ObjectivePlan::new(
            input.theory,
            input.atoms.atoms(),
            input.objectives,
            limits,
            control,
        ) {
            Ok(plan) => {
                state.work = plan.statistics().work;
                state.plan = Some(plan);
            }
            Err(error) => {
                state.work = error.statistics().work;
                observations.record(Event::ObjectiveUnavailable(error))?;
            }
        }
        Ok(state)
    }

    /// Call only after a strictly better, fully retained reduct-verified model.
    pub(crate) fn improve(
        &mut self,
        score: &Score,
        models: &mut StableModels,
        options: &SolveConfig,
        observations: &mut impl ExecutionSink,
        control: &Control,
    ) -> Result<(), SolveError> {
        let Some(plan) = &self.plan else {
            return Ok(());
        };
        let limits = ObjectiveBoundLimits {
            max_work: options.max_objective_bound_work.saturating_sub(self.work),
            ..Default::default()
        };
        let bound = match plan.bound(score, limits, control) {
            Ok(bound) => {
                self.work += bound.statistics().work;
                bound
            }
            Err(error) => {
                self.work += error.statistics().work;
                observations.record(Event::ObjectiveBoundStopped(error))?;
                self.plan = None;
                return Ok(());
            }
        };
        // Atom-count equality alone cannot establish semantic index meanings.
        // Input owns the completed catalog for this exact immutable theory.
        if !bound.original().same_instance(models.theory()) {
            observations.record(Event::ObjectiveTheoryMismatch)?;
            self.plan = None;
            return Ok(());
        }
        if let Err(error) = models.restrict_candidates(bound.theory()) {
            // Extension is transactional. Previous dominance bounds remain sound;
            // the next search step still observes any spent budget or cancellation.
            observations.record(Event::ObjectiveRestrictionStopped(error))?;
            self.plan = None;
        } else {
            observations.record(Event::ObjectiveBound {
                restrictions: models.statistics().candidate_restrictions,
                costs: score.costs(),
                work: self.work,
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/support/objective_observer_contracts.rs"]
mod observer_contract_tests;

#[cfg(test)]
#[path = "../tests/support/objective_bound_contracts.rs"]
mod contract_tests;
