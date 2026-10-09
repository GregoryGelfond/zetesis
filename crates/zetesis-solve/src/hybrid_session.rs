//! Source restrictions on candidate regions and complete answer checking.
//!
//! The retained core uses the ordinary formula session. Its answers
//! become answers of the original subject only after all streamed constraints
//! are satisfied. Only those complete original answers enter scoring, incumbent
//! retention and objective-bound feedback. This remains a CPU formula route;
//! terminal-definition objectives and device hybrid checking are excluded.
//! The append-constraints law justifies this composition; source
//! instance coverage and completed admission remain separate prerequisites.

use std::sync::Arc;
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_themelios::{ConstraintAllowance, ConstraintChecker, ConstraintVerdict, StreamedCore};

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
mod tests;

/// A streamed core and the subject its outcomes report: the hybrid owner, or
/// the terminal owner whose base the core is.
pub(crate) struct HybridInput<'a> {
    pub(crate) core: &'a StreamedCore,
    pub(crate) subject: crate::Subject,
}

pub(crate) struct HybridSession<'a> {
    /// The subject an outcome reports: the hybrid owner, or the terminal owner
    /// whose base this core is.
    subject: crate::Subject,
    core: FormulaSession<'a, Execution>,
    checker: ConstraintChecker<'a>,
    allowance: ConstraintAllowance,
    regions: Option<Arc<crate::hybrid_regions::Constraints>>,
    statistics: HybridExecutionStatistics,
    final_outcome: Option<SemanticOutcome>,
}

/// Establish original membership for one fresh core answer. A failed check
/// leaves it pending; successful acceptance precedes every objective operation.
fn accept(
    checker: &mut ConstraintChecker<'_>,
    statistics: &mut HybridExecutionStatistics,
    model: &Model,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> Result<bool, SolveError> {
    let count = statistics
        .core_answers
        .checked_add(1)
        .ok_or(SolveError::HybridStatisticsOverflow)?;
    statistics.core_answers = count;
    statistics.pending = 1;
    let verdict = phases.measure(SolvePhase::OriginalValidation, || {
        checker.check(model, cancellation)
    });
    match verdict.map_err(SolveError::Constraint)? {
        ConstraintVerdict::Satisfied => {
            // A refused score still leaves this original answer counted.
            statistics.accepted += 1;
            statistics.pending = 0;
            Ok(true)
        }
        ConstraintVerdict::Violated { .. } => {
            statistics.rejected += 1;
            statistics.pending = 0;
            Ok(false)
        }
    }
}

impl<'a> HybridSession<'a> {
    /// A hybrid owner's session: the route is recorded as lazy grounding and
    /// observed as hybrid grounding before the core session starts. A terminal
    /// owner's hybrid base is observed by its terminal session instead.
    pub(crate) fn observed(
        input: HybridInput<'a>,
        config: &SolveConfig,
        resources: &crate::ExecutionResources,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
        selection: AnswerSelection,
    ) -> Result<Self, SolveError> {
        phases.lazy_grounding();
        observations.record(crate::ExecutionObservation::HybridGrounding {
            requested: config.grounder,
            streamed_templates: input.core.streamed_templates(),
            streamed_instances: input.core.streamed_instances(),
        })?;
        Self::new(
            input,
            config,
            resources,
            observations,
            cancellation,
            phases,
            selection,
        )
    }

    pub(crate) fn new(
        input: HybridInput<'a>,
        config: &SolveConfig,
        resources: &crate::ExecutionResources,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
        selection: AnswerSelection,
    ) -> Result<Self, SolveError> {
        let HybridInput {
            core: owner,
            subject,
        } = input;
        let allowance = ConstraintAllowance::new(config.constraints);
        let checker = owner
            .checker_with_allowance(&allowance, cancellation)
            .map_err(SolveError::Constraint)?;
        let statistics = HybridExecutionStatistics::default();
        let core_config = SolveConfig {
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
                owner.analysis(),
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
            subject,
            core,
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
            return self.core.next_retained().map(Ok);
        }
        let Self {
            core,
            checker,
            statistics,
            ..
        } = self;
        let next =
            core.next_with_acceptance(config, observations, cancellation, phases, &mut |model| {
                accept(checker, statistics, model, cancellation, phases)
            });
        match next {
            Some(Err(error)) => {
                let state = self.core.outcome(phases).search_state;
                match self.finish(state, Some(error), phases) {
                    Ok(()) => self.core.next_retained().map(Ok),
                    Err(error) => Some(Err(error)),
                }
            }
            next => {
                if self.core.finished() {
                    let state = self.core.outcome(phases).search_state;
                    if let Err(error) = self.finish(state, None, phases) {
                        return Some(Err(error));
                    }
                }
                next
            }
        }
    }

    fn snapshot(&self, phases: &Recorder) -> SemanticOutcome {
        let mut outcome = self.core.outcome(phases);
        outcome.subject = Some(self.subject.clone());
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
    ) -> Result<(), SolveError> {
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
                Ok(()),
            ),
            Some(error) => (None, Err(error)),
            None => (crate::completion::after_cleanup(state, stopped), Ok(())),
        };
        self.core.conclude(state, phases);
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

    /// Settle this session for an enclosing one that stops for `state`: resolve
    /// it once (workers stopped, the region-failure slot taken) unless it has
    /// already finished, and return its final search state, or the constraint
    /// failure that cleanup established.
    pub(crate) fn conclude(
        &mut self,
        state: Option<SearchState>,
        phases: &Recorder,
    ) -> Result<Option<SearchState>, SolveError> {
        if let Some(outcome) = &self.final_outcome {
            return Ok(outcome.search_state);
        }
        self.finish(state, None, phases)?;
        Ok(self
            .final_outcome
            .as_ref()
            .and_then(|outcome| outcome.search_state))
    }
}
