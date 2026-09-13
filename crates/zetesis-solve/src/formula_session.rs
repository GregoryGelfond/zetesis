//! Stateful ordinary formula enumeration, scoring and incumbent retention.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;
use std::cell::Cell;

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_objective::Score;
use zetesis_sat::StableModels;

use crate::countermodel::Input;
use crate::formula_execution::{Failure, MembershipExecution};
use crate::objective_bounds::Bounds;
use crate::optimization::Incumbents;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{AnswerSelection, Interruption, SearchState, SemanticOutcome, SolveConfig, SolveError};

pub(crate) struct FormulaSession<'a, E> {
    input: Input<'a>,
    selection: AnswerSelection,
    execution: E,
    models: Option<StableModels>,
    bounds: Option<Bounds>,
    incumbents: Incumbents,
    ready: std::vec::IntoIter<Model>,
    yielded: usize,
    final_outcome: Option<SemanticOutcome>,
    pending_error: Option<SolveError>,
    // This session's last imported cumulative prefix, independently of every
    // other session sharing the host recorder. Repeated outcomes import no work.
    imported_timings: Cell<zetesis_sat::SearchPhaseTimings>,
}

impl<'a, E: MembershipExecution> FormulaSession<'a, E> {
    pub(crate) fn with_selection(
        input: Input<'a>,
        execution: E,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        control: &Control,
        phases: &Recorder,
        selection: AnswerSelection,
    ) -> Self {
        let mut session = Self {
            input,
            selection: if input.objectives.is_present() {
                selection
            } else {
                AnswerSelection::All
            },
            execution,
            models: None,
            bounds: None,
            incumbents: Incumbents::default(),
            ready: Vec::new().into_iter(),
            yielded: 0,
            final_outcome: None,
            pending_error: None,
            imported_timings: Cell::new(zetesis_sat::SearchPhaseTimings::default()),
        };
        if let Err(error) = session.initialize(config, observations, control, phases) {
            session.fail(error, phases);
        }
        session
    }

    fn initialize(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        control: &Control,
        phases: &Recorder,
    ) -> Result<(), SolveError> {
        observations.record(Event::Formula {
            atoms: self.input.theory.atom_count(),
            nodes: self.input.theory.nodes().len(),
            roots: self.input.theory.roots().len(),
        })?;
        let models = phases.measure(SolvePhase::CandidateSetup, || {
            StableModels::new(
                self.input.theory,
                crate::countermodel::search_limits(config),
                control.clone(),
            )
        });
        let mut models = match models {
            Ok(models) => models,
            Err(error) => {
                self.complete(
                    SearchState::Interrupted(Interruption::Countermodel(error)),
                    phases,
                );
                return Ok(());
            }
        };
        if config.stats {
            models.enable_phase_timing();
        }
        // Ownership is established before any fallible certificate/diagnostic
        // operation, so its attempted work remains available on failure.
        self.models = Some(models);
        let models = self.models.as_mut().expect("candidate stream installed");
        if let Some(error) =
            crate::countermodel::prepare_certificate(models, config, observations, phases)?
        {
            self.complete(
                SearchState::Interrupted(Interruption::Countermodel(error)),
                phases,
            );
            return Ok(());
        }
        if self.selection == AnswerSelection::Optimal {
            self.bounds = Some(
                if self.input.objectives.is_present() && config.max_objective_bound_work != 0 {
                    phases.measure(SolvePhase::ObjectiveFeedback, || {
                        Bounds::new(self.input, config, observations, control)
                    })?
                } else {
                    Bounds::new(self.input, config, observations, control)?
                },
            );
        }
        Ok(())
    }

    pub(crate) fn next(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        control: &Control,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        let next = self.next_result(config, observations, control, phases);
        self.import_timings(
            phases,
            self.models
                .as_ref()
                .and_then(|models| models.statistics().phase_timings),
        );
        next
    }

    fn next_result(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        control: &Control,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        if let Some(error) = self.pending_error.take() {
            return Some(Err(error));
        }
        if self.final_outcome.is_some() {
            return self.next_retained().map(Ok);
        }
        if self.selection == AnswerSelection::All
            && config.models != 0
            && self.yielded >= config.models
        {
            self.complete(SearchState::RequestedModels, phases);
            return None;
        }
        // The retained candidate stream owns cumulative work and exact blocks;
        // pulls never restart it. Objective improvements narrow only candidates.
        loop {
            let models = self
                .models
                .as_mut()
                .expect("unfinished session has a candidate stream");
            let next = self.execution.next(models, config, control, phases);
            let interpretation = match next {
                Some(Ok(model)) => model,
                Some(Err(Failure::Search(error))) => {
                    self.complete(
                        SearchState::Interrupted(Interruption::Countermodel(error)),
                        phases,
                    );
                    return self.next_retained().map(Ok);
                }
                Some(Err(Failure::Run(error))) => {
                    self.fail(error, phases);
                    return self.pending_error.take().map(Err);
                }
                None => {
                    debug_assert!(models.exhausted());
                    self.complete(SearchState::Exhausted, phases);
                    return self.next_retained().map(Ok);
                }
            };
            let model = match Model::from_positions(self.input.atoms, interpretation.atoms()) {
                Ok(model) => model,
                Err(error) => {
                    self.fail(SolveError::Model(error), phases);
                    return self.pending_error.take().map(Err);
                }
            };
            if !self.input.objectives.is_present() {
                self.yielded += 1;
                return Some(Ok((model, None)));
            }
            if self.selection == AnswerSelection::All {
                let score = phases.measure(SolvePhase::ObjectiveScoringRetention, || {
                    self.incumbents
                        .evaluate(self.input.objectives, &model, config, control)
                });
                return match score {
                    Ok(score) => {
                        self.yielded += 1;
                        Some(Ok((model, Some(score))))
                    }
                    Err(reason) => {
                        self.complete(SearchState::Interrupted(reason), phases);
                        None
                    }
                };
            }
            let scored = phases.measure(SolvePhase::ObjectiveScoringRetention, || {
                self.incumbents
                    .consider(self.input.objectives, model, config, control)
            });
            match scored {
                Ok(true) => {
                    let _feedback = (config.max_objective_bound_work != 0)
                        .then(|| phases.start(SolvePhase::ObjectiveFeedback));
                    let improved = self
                        .bounds
                        .as_mut()
                        .expect("initialized objective plan")
                        .improve(
                            self.incumbents
                                .score()
                                .expect("retained improvement has a score"),
                            self.models.as_mut().expect("owned candidate stream"),
                            config,
                            observations,
                            control,
                        );
                    if let Err(error) = improved {
                        self.fail(error, phases);
                        return self.pending_error.take().map(Err);
                    }
                }
                Ok(false) => {}
                Err(reason) => {
                    self.complete(SearchState::Interrupted(reason), phases);
                    return self.next_retained().map(Ok);
                }
            }
        }
    }

    fn next_retained(&mut self) -> Option<(Model, Option<Score>)> {
        self.ready.next().map(|model| {
            let score = self.incumbents.score().cloned();
            (model, score)
        })
    }

    fn complete(&mut self, search_state: SearchState, phases: &Recorder) {
        let mut outcome = self.snapshot(phases);
        outcome.search_state = Some(search_state);
        self.final_outcome = Some(outcome);
        self.ready = self.incumbents.take_models();
    }

    fn fail(&mut self, error: SolveError, phases: &Recorder) {
        self.final_outcome = Some(self.snapshot(phases));
        self.pending_error = Some(error);
    }

    pub(crate) fn outcome(&self, phases: &Recorder) -> SemanticOutcome {
        self.final_outcome
            .clone()
            .unwrap_or_else(|| self.snapshot(phases))
    }

    pub(crate) const fn finished(&self) -> bool {
        self.final_outcome.is_some()
    }

    fn snapshot(&self, phases: &Recorder) -> SemanticOutcome {
        let statistics = self.models.as_ref().map(StableModels::statistics);
        self.import_timings(
            phases,
            statistics.and_then(|statistics| statistics.phase_timings),
        );
        SemanticOutcome {
            subject: Some(crate::Subject::Theory(self.input.theory.clone())),
            selection: Some(self.selection),
            verified: statistics.map_or(0, |s| s.stable_models),
            scored: self.incumbents.scored(),
            retained: self.incumbents.retained(),
            search_state: None,
            optimization: self.incumbents.metadata().cloned(),
            checked: statistics.map_or(0, |s| s.candidates),
            gate_atoms: self.input.gate_atoms,
            candidate_statistics: None,
            countermodel_statistics: statistics,
            lazy_execution: None,
            shared_execution: None,
            formula_execution: self
                .models
                .as_ref()
                .and_then(|models| self.execution.statistics(models)),
        }
    }

    fn import_timings(&self, phases: &Recorder, timing: Option<zetesis_sat::SearchPhaseTimings>) {
        if let Some(timing) = timing {
            phases.search(self.imported_timings.replace(timing), timing);
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/formula_timing_contracts.rs"]
mod timing_tests;
