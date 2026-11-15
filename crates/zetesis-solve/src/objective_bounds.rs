//! Shared immutable objective preparation and separate incumbent pruning state.

use crate::ExecutionObservation as Event;
use crate::countermodel::Input;
use crate::execution_observation::ExecutionSink;
use crate::{SolveConfig, SolveError};
use zetesis_algebra::Cancellation;
use zetesis_objective::Score;
use zetesis_stable::StableModels;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundError, ObjectiveBoundLimits, ObjectivePlan, ObjectivePlanLimits,
    ObjectiveProjectionError,
};

/// A single preparation shared by score reads and optional bound generation.
/// Its owner and attempted work survive every pruning refusal.
pub(crate) struct Preparation {
    plan: Option<ObjectivePlan>,
    work: u64,
    refusal: Option<ObjectiveBoundError>,
    projection_refusal: Option<ObjectiveProjectionError>,
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
            projection_refusal: None,
            choice_refusal: None,
        };
        if !input.objectives.is_present() || options.max_objective_work == 0 {
            return state;
        }
        let limits = plan_limits(options, 0);
        if let Some(source) = input.objective_source {
            // Projection owns a separately bounded optional workspace, not a
            // debit from a global candidate-memory pool. Never retry the legacy
            // constructor here: exact detailed scoring retains the remaining
            // objective allowance after this single preparation attempt.
            let attempt = ObjectivePlan::new_projected(
                source,
                limits,
                options.max_candidate_bytes as u128,
                cancellation,
            );
            state.work = attempt.receipt().statistics().work;
            state.projection_refusal = attempt.error();
            state.plan = attempt.into_parts().0.ok();
            return state;
        }
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
        if let Some(error) = self.projection_refusal {
            observations.record(Event::ObjectiveProjectionUnavailable(error))?;
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

/// The bound to install: its plan, its incumbent's score, and whether it keeps
/// only strictly cheaper candidates.
struct Request<'a> {
    plan: Option<&'a ObjectivePlan>,
    score: &'a Score,
    strict: bool,
}

/// The bound last installed on the current stream.
struct Installed {
    score: Score,
    strict: bool,
}

#[derive(Default)]
pub(crate) struct Bounds {
    enabled: bool,
    work: u64,
    /// Improvements install strict bounds: always under
    /// [`crate::OptimumSearch::StrictWithTiePass`], and for a request of one
    /// answer under [`crate::OptimumSearch::StrictForOneAnswer`].
    strict: bool,
    installed: Option<Installed>,
}

impl Bounds {
    pub(crate) fn new(options: &SolveConfig) -> Self {
        Self {
            enabled: options.max_objective_bound_work != 0,
            work: 0,
            strict: match options.optimum_search {
                crate::OptimumSearch::StrictWithTiePass => true,
                crate::OptimumSearch::StrictForOneAnswer => options.models == 1,
                crate::OptimumSearch::Inclusive => false,
            },
            installed: None,
        }
    }

    /// Whether an improvement can still install a candidate bound.
    pub(crate) const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The incumbent score whose ties the last installed bound prunes, when
    /// that bound is strict. A strict bound at a worse score than the optimum
    /// keeps every optimal model, and so does an inclusive bound, so the
    /// search meets each of them; only a strict bound at the optimum itself
    /// owes a tie pass.
    pub(crate) fn pruned_ties_at(&self) -> Option<&Score> {
        self.installed
            .as_ref()
            .filter(|bound| bound.strict)
            .map(|bound| &bound.score)
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
        let request = Request {
            plan,
            score,
            strict: self.strict,
        };
        self.install(request, models, options, observations, cancellation)
            .map(|_| ())
    }

    /// Install a tie pass's bound on its fresh stream: cost at most the proved
    /// `optimum`, the stream's first bound, so it has no predecessor to imply.
    /// `Ok(false)` when no bound was installed; the pass then runs unbounded,
    /// and scoring alone keeps the optimum's ties.
    pub(crate) fn bound_ties(
        &mut self,
        plan: Option<&ObjectivePlan>,
        optimum: &Score,
        models: &mut StableModels,
        options: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
    ) -> Result<bool, SolveError> {
        let request = Request {
            plan,
            score: optimum,
            strict: false,
        };
        self.install(request, models, options, observations, cancellation)
    }

    fn install(
        &mut self,
        Request {
            plan,
            score,
            strict,
        }: Request<'_>,
        models: &mut StableModels,
        options: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
    ) -> Result<bool, SolveError> {
        let Some(plan) = plan.filter(|_| self.enabled) else {
            return Ok(false);
        };
        let limits = ObjectiveBoundLimits {
            max_work: options.max_objective_bound_work.saturating_sub(self.work),
            ..Default::default()
        };
        let compiled = if strict {
            plan.strict_bound(score, limits, cancellation)
        } else {
            plan.bound(score, limits, cancellation)
        };
        let bound = match compiled {
            Ok(bound) => {
                self.work += bound.statistics().work;
                bound
            }
            Err(error) => {
                self.work += error.statistics().work;
                self.enabled = false;
                observations.record(Event::ObjectiveBoundStopped(error))?;
                return Ok(false);
            }
        };
        if let Some(error) = bound.choice_failure() {
            observations.record(Event::ChoiceObjectiveBoundUnavailable(error))?;
        }
        if !bound.original().same_instance(models.theory()) {
            self.enabled = false;
            observations.record(Event::ObjectiveTheoryMismatch)?;
            return Ok(false);
        }
        if let Err(error) = models.improve_candidate_bound(plan.identity(), score, bound.theory()) {
            // Replacement is transactional: the older bound remains on failure.
            // Later scoring still borrows the unchanged immutable preparation.
            self.enabled = false;
            observations.record(Event::ObjectiveRestrictionStopped(error))?;
            return Ok(false);
        }
        self.installed = Some(Installed {
            score: score.clone(),
            strict,
        });
        observations.record(Event::ObjectiveBound {
            restrictions: models.statistics().candidate_restrictions,
            costs: score.costs(),
            strict,
            work: self.work,
        })?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
