//! Full-answer reconstruction over the existing base formula enumerator.
//!
//! A base answer is not exposed as an answer of the original source. The
//! admitted terminal-definition correspondence permits precisely one extension,
//! and publication follows only after that extension completes.

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_themelios::{ReconstructionStatistics, TerminalFormula, TerminalReconstruction};

use crate::execution_observation::ExecutionSink;
use crate::formula_execution::Execution;
use crate::formula_session::FormulaSession;
use crate::hybrid_session::{HybridInput, HybridSession};
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{AnswerSelection, Interruption, SearchState, SemanticOutcome, SolveConfig, SolveError};

#[cfg(test)]
mod tests;

/// Verified base answers consumed by full-answer reconstruction.
///
/// `base_answers = reconstructed + pending`. Verified base answers still
/// buffered by the inner engine are excluded. Only reconstructed answers count
/// toward original-program membership or its requested model limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalExecutionStatistics {
    /// How the base was grounded.
    pub base: zetesis_themelios::BaseKind,
    /// Base answers consumed, before reconstruction starts.
    pub base_answers: u64,
    /// Complete interpretations reconstructed for the original source.
    pub reconstructed: u64,
    /// Consumed base answers whose full reconstruction did not finish, zero or one.
    pub pending: u64,
    /// Cumulative frontend reconstruction work, including accepted admission
    /// history and interrupted attempts. This is separate from base search work.
    pub reconstruction: ReconstructionStatistics,
}

/// The base enumerator: an eager formula session, or a hybrid session that
/// yields only the core answers its streamed constraints accept.
enum BaseSession<'a> {
    Eager(Box<FormulaSession<'a, Execution>>),
    Hybrid(Box<HybridSession<'a>>),
}

pub(crate) struct TerminalSession<'a> {
    owner: &'a TerminalFormula,
    base: BaseSession<'a>,
    base_config: SolveConfig,
    reconstruction: TerminalReconstruction<'a>,
    statistics: TerminalExecutionStatistics,
    final_outcome: Option<SemanticOutcome>,
}

impl<'a> TerminalSession<'a> {
    pub(crate) fn new(
        owner: &'a TerminalFormula,
        config: &SolveConfig,
        resources: &crate::ExecutionResources,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Result<Self, SolveError> {
        let streamed = match owner.base() {
            zetesis_themelios::TerminalBase::Eager(_) => None,
            zetesis_themelios::TerminalBase::Hybrid(core) => Some(crate::StreamedConstraints {
                templates: core.streamed_templates(),
                instances: core.streamed_instances(),
            }),
        };
        observations.record(crate::ExecutionObservation::TerminalDefinitions {
            requested: config.grounder,
            base: owner.base_kind(),
            deferred_templates: owner.deferred_templates(),
            streamed,
        })?;
        let reconstruction = owner.reconstruction().map_err(SolveError::Reconstruction)?;
        let statistics = TerminalExecutionStatistics {
            base: owner.base_kind(),
            reconstruction: reconstruction.statistics(),
            ..TerminalExecutionStatistics::default()
        };
        let (base, base_config) = match owner.base() {
            zetesis_themelios::TerminalBase::Eager(theory) => {
                let base_config = SolveConfig {
                    models: 0,
                    grounder: crate::Grounder::Eager,
                    ..*config
                };
                let input = crate::countermodel::Input {
                    theory,
                    atoms: owner.base_atom_catalog(),
                    objectives: owner.objectives(),
                    gate_atoms: 0,
                    keyed_constraints: owner.keyed_constraints(),
                    key_analysis: owner.key_analysis(),
                    certificate_order: crate::countermodel::certificate_order(
                        owner.base_analysis(),
                        owner.base_analysis_basis(),
                    ),
                };
                let base = FormulaSession::with_resources(
                    input,
                    &base_config,
                    resources,
                    observations,
                    cancellation,
                    phases,
                    AnswerSelection::All,
                )?;
                (BaseSession::Eager(Box::new(base)), base_config)
            }
            zetesis_themelios::TerminalBase::Hybrid(core) => {
                // The hybrid base checks every core answer before this session
                // sees it: check, then extend, then publish.
                let base_config = SolveConfig {
                    models: 0,
                    ..*config
                };
                let base = HybridSession::new(
                    HybridInput {
                        core,
                        subject: crate::Subject::TerminalDefinitions(owner.clone()),
                    },
                    &base_config,
                    resources,
                    observations,
                    cancellation,
                    phases,
                    AnswerSelection::All,
                )?;
                (BaseSession::Hybrid(Box::new(base)), base_config)
            }
        };
        Ok(Self {
            owner,
            base,
            base_config,
            reconstruction,
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
        if config.models != 0 && self.statistics.reconstructed >= config.models as u64 {
            return self.finish(Some(SearchState::RequestedModels), None, phases);
        }
        // Admit the next counter before consuming a base answer. An overflow
        // cannot lose an answer between the consumed and pending receipts.
        let Some(consumed) = self.statistics.base_answers.checked_add(1) else {
            return self.finish(None, Some(SolveError::TerminalStatisticsOverflow), phases);
        };
        let next = match &mut self.base {
            BaseSession::Eager(base) => {
                base.next(&self.base_config, observations, cancellation, phases)
            }
            BaseSession::Hybrid(base) => {
                base.next(&self.base_config, observations, cancellation, phases)
            }
        };
        let model = match next {
            Some(Ok((model, _))) => model,
            Some(Err(error)) => return self.finish(None, Some(error), phases),
            None => {
                let state = self.base_outcome(phases).search_state;
                return self.finish(state, None, phases);
            }
        };
        self.reconstruct(&model, consumed, cancellation, phases)
    }

    /// Qualify one consumed base answer before publishing original membership.
    /// The caller admits `consumed` before advancing the base enumerator.
    fn reconstruct(
        &mut self,
        model: &Model,
        consumed: u64,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        self.statistics.base_answers = consumed;
        self.statistics.pending = 1;
        let reconstructed = phases.measure(SolvePhase::AnswerReconstruction, || {
            self.reconstruction.reconstruct(model, cancellation)
        });
        self.statistics.reconstruction = self.reconstruction.statistics();
        match reconstructed {
            Ok(model) => {
                // A completed count cannot exceed the already checked consumed count.
                self.statistics.reconstructed += 1;
                self.statistics.pending = 0;
                Some(Ok((model, None)))
            }
            Err(error) => self.finish(None, Some(SolveError::Reconstruction(error)), phases),
        }
    }

    fn base_outcome(&self, phases: &Recorder) -> SemanticOutcome {
        match &self.base {
            BaseSession::Eager(base) => base.outcome(phases),
            BaseSession::Hybrid(base) => base.outcome(phases),
        }
    }

    fn snapshot(&self, phases: &Recorder) -> SemanticOutcome {
        let mut outcome = self.base_outcome(phases);
        outcome.subject = Some(crate::Subject::TerminalDefinitions(self.owner.clone()));
        outcome.selection = Some(AnswerSelection::All);
        outcome.verified = self.statistics.reconstructed;
        outcome.scored = 0;
        outcome.retained = 0;
        outcome.optimization = None;
        // Base exhaustion alone cannot complete an unfinished reconstruction.
        outcome.search_state = None;
        outcome.terminal_execution = Some(TerminalExecutionStatistics {
            reconstruction: self.reconstruction.statistics(),
            ..self.statistics
        });
        outcome
    }

    fn finish(
        &mut self,
        state: Option<SearchState>,
        error: Option<SolveError>,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        // Settle workers before taking the final base receipt. The original
        // reconstruction/execution failure or finalized interruption takes
        // precedence over cleanup, except that a hybrid base's constraint
        // failure, which decides a candidate's acceptance before any
        // extension, precedes a reconstruction refusal.
        let concluded = match &mut self.base {
            BaseSession::Eager(base) => {
                let stopped = base.stop(phases);
                Ok(crate::completion::after_cleanup(state, stopped))
            }
            BaseSession::Hybrid(base) => {
                base.conclude(if error.is_none() { state } else { None }, phases)
            }
        };
        let concluded = match concluded {
            Ok(state) => state,
            Err(base_error) => {
                let mut outcome = self.snapshot(phases);
                outcome.search_state = None;
                self.final_outcome = Some(outcome);
                return Some(Err(base_error));
            }
        };
        let (state, result) = match error {
            Some(SolveError::Reconstruction(error)) => match error.stop() {
                Some(stop) => (
                    Some(SearchState::Interrupted(Interruption::Reconstruction(stop))),
                    None,
                ),
                None => (None, Some(Err(SolveError::Reconstruction(error)))),
            },
            Some(error) => (None, Some(Err(error))),
            None => (concluded, None),
        };
        let mut outcome = self.snapshot(phases);
        outcome.search_state = state;
        self.final_outcome = Some(outcome);
        result
    }

    pub(crate) fn stop(&mut self, phases: &Recorder) {
        if self.final_outcome.is_none() {
            let _ = self.finish(None, None, phases);
        }
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
