//! Shared immutable objective preparation and separate incumbent pruning state.

use crate::ExecutionObservation as Event;
use crate::countermodel::Input;
use crate::execution_observation::ExecutionSink;
use crate::{SolveConfig, SolveError};
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_sat::StableModels;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundError, ObjectiveBoundLimits, ObjectivePlan, ObjectivePlanLimits,
};

/// A single preparation shared by score reads and optional bound generation.
/// Its owner and attempted work survive every pruning refusal.
pub(crate) struct Preparation {
    plan: Option<ObjectivePlan>,
    work: u64,
    refusal: Option<ObjectiveBoundError>,
    choice_refusal: Option<ObjectiveBoundError>,
}

impl Preparation {
    pub(crate) fn new(
        input: Input<'_>,
        options: &SolveConfig,
        cancellation: &Cancellation,
    ) -> Self {
        let mut state = Self {
            plan: None,
            work: 0,
            refusal: None,
            choice_refusal: None,
        };
        if !input.objectives.is_present() || options.max_objective_work == 0 {
            return state;
        }
        let limits = plan_limits(options, 0);
        match ObjectivePlan::new(
            input.theory,
            input.atoms.atoms(),
            input.objectives,
            limits,
            cancellation,
        ) {
            Ok(plan) => {
                state.work = plan.statistics().work;
                state.plan = Some(plan);
            }
            Err(error) => {
                state.work = error.statistics().work;
                state.refusal = Some(error);
            }
        }
        state
    }

    /// Optional necessary bounds supplement the exact plan. A refusal retains
    /// that plan and charges the attempted work before any diagnostic is sent.
    pub(crate) fn prepare_choices(
        &mut self,
        choices: Option<&zetesis_themelios::RequiredChoices>,
        options: &SolveConfig,
        cancellation: &Cancellation,
    ) {
        if options.max_objective_bound_work == 0 {
            return;
        }
        let (Some(plan), Some(choices)) = (self.plan.as_mut(), choices) else {
            return;
        };
        let result = plan.prepare_choice_bounds(
            choices,
            plan_limits(options, self.work),
            options.max_candidate_bytes as u128,
            cancellation,
        );
        match result {
            Ok(statistics) => self.work += statistics.work,
            Err(error) => {
                self.work += error.statistics().work;
                self.choice_refusal = Some(error);
            }
        }
    }

    pub(crate) const fn work(&self) -> u64 {
        self.work
    }
    pub(crate) fn plan(&self) -> Option<&ObjectivePlan> {
        self.plan.as_ref()
    }

    /// Observe only after the session owns and accounts for this preparation.
    pub(crate) fn observe(&self, observations: &mut impl ExecutionSink) -> Result<(), SolveError> {
        if let Some(error) = self.refusal {
            observations.record(Event::ObjectiveUnavailable(error))?;
        }
        if let Some(error) = self.choice_refusal {
            observations.record(Event::ChoiceObjectiveBoundUnavailable(error))?;
        }
        Ok(())
    }
}

fn plan_limits(options: &SolveConfig, spent: u64) -> ObjectivePlanLimits {
    let defaults = ObjectivePlanLimits::default();
    ObjectivePlanLimits {
        max_work: options
            .max_objective_work
            .saturating_sub(spent)
            .min(defaults.max_work),
        max_bindings: options.max_objective_bindings,
        max_keys: options.max_objective_keys,
        max_key_bytes: options.max_objective_key_bytes,
        ..defaults
    }
}

#[derive(Default)]
pub(crate) struct Bounds {
    enabled: bool,
    work: u64,
}

impl Bounds {
    pub(crate) fn new(options: &SolveConfig) -> Self {
        Self {
            enabled: options.max_objective_bound_work != 0,
            work: 0,
        }
    }

    /// Call only after a strictly better, fully retained reduct-verified model.
    /// Bound generation borrows the same immutable plan used by score reads.
    pub(crate) fn improve(
        &mut self,
        plan: Option<&ObjectivePlan>,
        score: &Score,
        models: &mut StableModels,
        options: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
    ) -> Result<(), SolveError> {
        let Some(plan) = plan.filter(|_| self.enabled) else {
            return Ok(());
        };
        let limits = ObjectiveBoundLimits {
            max_work: options.max_objective_bound_work.saturating_sub(self.work),
            ..Default::default()
        };
        let bound = match plan.bound(score, limits, cancellation) {
            Ok(bound) => {
                self.work += bound.statistics().work;
                bound
            }
            Err(error) => {
                self.work += error.statistics().work;
                self.enabled = false;
                observations.record(Event::ObjectiveBoundStopped(error))?;
                return Ok(());
            }
        };
        if let Some(error) = bound.choice_failure() {
            observations.record(Event::ChoiceObjectiveBoundUnavailable(error))?;
        }
        if !bound.original().same_instance(models.theory()) {
            self.enabled = false;
            observations.record(Event::ObjectiveTheoryMismatch)?;
            return Ok(());
        }
        if let Err(error) = models.tighten_candidate_bound(bound.theory()) {
            // Replacement is transactional: the older bound remains on failure.
            // Later scoring still borrows the unchanged immutable preparation.
            self.enabled = false;
            observations.record(Event::ObjectiveRestrictionStopped(error))?;
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
mod tests;
