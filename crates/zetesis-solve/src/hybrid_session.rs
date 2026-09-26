//! Source restrictions on candidate regions and complete answer checking.
//!
//! The retained core uses the ordinary formula session. Its answers
//! become answers of the original subject only after all streamed constraints
//! are satisfied. The append-constraints law justifies this composition; source
//! instance coverage and completed admission remain separate prerequisites.

use std::sync::Arc;
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_themelios::{ConstraintAllowance, ConstraintChecker, ConstraintVerdict, HybridFormula};

use crate::execution_observation::ExecutionSink;
use crate::formula_execution::Execution;
use crate::formula_session::FormulaSession;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{AnswerSelection, Interruption, SearchState, SemanticOutcome, SolveConfig, SolveError};

/// Consumed core answers and their streamed-constraint decisions.
///
/// `core_answers = accepted + rejected + pending`. Core answers still buffered
/// by the inner enumerator are excluded. Only `accepted` establishes membership
/// in the original program; core membership alone is insufficient.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HybridExecutionStatistics {
    /// Retained-core answers consumed by the source constraint checker.
    pub core_answers: u64,
    /// Answers satisfying every required streamed constraint instance.
    pub accepted: u64,
    /// Core answers rejected by an observed constraint violation.
    pub rejected: u64,
    /// Consumed core answers whose constraint check did not finish.
    pub pending: u64,
    /// Cumulative source checking work, including region checks, preparation and
    /// interrupted attempts. Live worker counters become final after joining.
    pub constraints: zetesis_themelios::ConstraintCheckStatistics,
}

#[cfg(test)]
#[path = "../tests/support/hybrid_terminal.rs"]
mod terminal_tests;

pub(crate) struct HybridSession<'a> {
    owner: &'a HybridFormula,
    core: FormulaSession<'a, Execution>,
    core_config: SolveConfig,
    checker: ConstraintChecker<'a>,
    allowance: ConstraintAllowance,
    regions: Option<Arc<crate::hybrid_regions::Constraints>>,
    statistics: HybridExecutionStatistics,
    final_outcome: Option<SemanticOutcome>,
}

impl<'a> HybridSession<'a> {
    pub(crate) fn new(
        owner: &'a HybridFormula,
        config: &SolveConfig,
        resources: crate::session::Executors<'_>,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
        selection: AnswerSelection,
    ) -> Result<Self, SolveError> {
        phases.lazy_grounding();
        observations.record(crate::ExecutionObservation::HybridGrounding {
            requested: config.grounder,
            streamed_templates: owner.streamed_templates(),
            streamed_instances: owner.streamed_instances(),
        })?;
        let allowance = ConstraintAllowance::new(config.constraints);
        let checker = owner
            .checker_with_allowance(&allowance, cancellation)
            .map_err(SolveError::Constraint)?;
        let statistics = HybridExecutionStatistics {
            constraints: checker.statistics(),
            ..HybridExecutionStatistics::default()
        };
        let core_config = SolveConfig {
            models: 0,
            grounder: crate::Grounder::Eager,
            ..*config
        };
        let input = crate::countermodel::Input {
            theory: owner.core_theory(),
            atoms: owner.atom_catalog(),
            gate_atoms: 0,
            keyed_constraints: owner.keyed_constraints(),
            key_analysis: owner.key_analysis(),
            objectives: owner.objectives(),
            certificate_order: crate::countermodel::certificate_order(
                owner.source_analysis(),
                owner.analysis_basis(),
            ),
        };
        let mut core = FormulaSession::with_resources(
            input,
            &core_config,
            resources,
            observations,
            cancellation,
            phases,
            selection,
        )?;
        let regions = (owner.streamed_templates() != 0
            && config.search == crate::SearchMethod::Regions)
            .then(|| {
                let regions = Arc::new(crate::hybrid_regions::Constraints::new(
                    owner.clone(),
                    allowance.clone(),
                ));
                core.set_region_filter(regions.clone(), phases);
                regions
            });
        Ok(Self {
            owner,
            core,
            core_config,
            checker,
            allowance,
            regions,
            statistics,
            final_outcome: None,
        })
    }

    pub(crate) fn next(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        if self.final_outcome.is_some() {
            return None;
        }
        if config.models != 0 && self.statistics.accepted >= config.models as u64 {
            return self.finish(Some(SearchState::RequestedModels), None, phases);
        }
        loop {
            let model = match self
                .core
                .next(&self.core_config, observations, cancellation, phases)
            {
                Some(Ok((model, _))) => model,
                Some(Err(error)) => {
                    return self.finish(None, Some(error), phases);
                }
                None => return self.finish(self.core.outcome(phases).search_state, None, phases),
            };
            let Some(count) = self.statistics.core_answers.checked_add(1) else {
                return self.finish(None, Some(SolveError::HybridStatisticsOverflow), phases);
            };
            self.statistics.core_answers = count;
            self.statistics.pending = 1;
            let checked = phases.measure(SolvePhase::OriginalValidation, || {
                self.checker.check(&model, cancellation)
            });
            self.statistics.constraints = self.allowance.statistics();
            match checked {
                Ok(ConstraintVerdict::Satisfied) => {
                    // The completed count is bounded by the checked core count.
                    self.statistics.accepted += 1;
                    self.statistics.pending = 0;
                    return Some(Ok((model, None)));
                }
                Ok(ConstraintVerdict::Violated { .. }) => {
                    self.statistics.rejected += 1;
                    self.statistics.pending = 0;
                }
                Err(error) => {
                    return self.finish(None, Some(SolveError::Constraint(error)), phases);
                }
            }
        }
    }

    fn snapshot(&self, phases: &Recorder) -> SemanticOutcome {
        let mut outcome = self.core.outcome(phases);
        outcome.subject = Some(crate::Subject::Hybrid(self.owner.clone()));
        outcome.verified = self.statistics.accepted;
        // Core exhaustion cannot establish completion of an unfinished source check.
        outcome.search_state = None;
        outcome.hybrid_execution = Some(HybridExecutionStatistics {
            constraints: self.allowance.statistics(),
            ..self.statistics
        });
        outcome
    }

    /// Record a final-check failure before stopping workers, then resolve every
    /// terminal path once. A worker may already have recorded a source fault;
    /// their shared slot preserves the first recording, not a wall-clock order.
    fn finish(
        &mut self,
        state: Option<SearchState>,
        error: Option<SolveError>,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        let mut error = match (error, &self.regions) {
            (Some(SolveError::Constraint(error)), Some(regions)) => {
                regions.record_failure(error);
                None
            }
            (error, _) => error,
        };
        let stopped = self.core.stop(phases);
        let source_failure = self.regions.as_ref().and_then(|r| r.take_failure());
        let source_state = matches!(
            state,
            Some(SearchState::Interrupted(Interruption::Countermodel(
                zetesis_sat::Incomplete::RegionFilter
            )))
        );
        // A source marker still needs its precise recorded cause. Any other
        // finalized interruption precedes faults discovered while joining;
        // taking the slot above still settles the source's retained receipt.
        if !matches!(state, Some(SearchState::Interrupted(_))) || source_state {
            error = error.or_else(|| source_failure.map(SolveError::Constraint));
            if (stopped == Err(zetesis_sat::Incomplete::RegionFilter) || source_state)
                && error.is_none()
            {
                error = Some(SolveError::ConstraintFailureMissing);
            }
        }
        let (state, result) = match error {
            Some(SolveError::Constraint(error)) if error.stop().is_some() => (
                Some(SearchState::Interrupted(Interruption::Constraint(
                    error.stop().expect("checked source stop"),
                ))),
                None,
            ),
            Some(error) => (None, Some(Err(error))),
            None => (crate::completion::after_cleanup(state, stopped), None),
        };
        let mut outcome = self.snapshot(phases);
        outcome.search_state = state;
        self.final_outcome = Some(outcome);
        result
    }

    pub(crate) fn outcome(&self, phases: &Recorder) -> SemanticOutcome {
        self.final_outcome
            .clone()
            .unwrap_or_else(|| self.snapshot(phases))
    }

    pub(crate) const fn finished(&self) -> bool {
        self.final_outcome.is_some()
    }
}
