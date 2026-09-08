//! Stateful ordinary formula enumeration, scoring and incumbent retention.

use std::io::Write;

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_objective::Score;
use zetesis_sat::StableModels;

use crate::countermodel::Input;
use crate::formula_execution::{Failure, MembershipExecution};
use crate::objective_bounds::Bounds;
use crate::optimization::Incumbents;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{Completion, Interruption, RunError, SemanticOutcome, SolveConfig};

pub(crate) struct FormulaSession<'a, E> {
    input: Input<'a>,
    execution: E,
    models: Option<StableModels>,
    bounds: Option<Bounds>,
    incumbents: Incumbents,
    ready: std::vec::IntoIter<Model>,
    yielded: usize,
    final_outcome: Option<SemanticOutcome>,
    pending_error: Option<RunError>,
}

impl<'a, E: MembershipExecution> FormulaSession<'a, E> {
    pub(crate) fn new(
        input: Input<'a>,
        execution: E,
        config: &SolveConfig,
        diagnostics: &mut impl Write,
        control: &Control,
        phases: &Recorder,
    ) -> Self {
        let mut session = Self {
            input,
            execution,
            models: None,
            bounds: None,
            incumbents: Incumbents::default(),
            ready: Vec::new().into_iter(),
            yielded: 0,
            final_outcome: None,
            pending_error: None,
        };
        if let Err(error) = session.initialize(config, diagnostics, control, phases) {
            session.fail(error, phases);
        }
        session
    }

    fn initialize(
        &mut self,
        config: &SolveConfig,
        diagnostics: &mut impl Write,
        control: &Control,
        phases: &Recorder,
    ) -> Result<(), RunError> {
        writeln!(
            diagnostics,
            "Formula: {} atoms, {} nodes, {} roots",
            self.input.theory.atom_count(),
            self.input.theory.nodes().len(),
            self.input.theory.roots().len()
        )?;
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
                    Completion::Interrupted,
                    Some(Interruption::Countermodel(error)),
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
            crate::countermodel::prepare_certificate(models, config, diagnostics, phases)?
        {
            self.complete(
                Completion::Interrupted,
                Some(Interruption::Countermodel(error)),
                phases,
            );
            return Ok(());
        }
        self.bounds = Some(
            if self.input.objectives.is_present() && config.max_objective_bound_work != 0 {
                phases.measure(SolvePhase::ObjectiveFeedback, || {
                    Bounds::new(self.input, config, diagnostics, control)
                })?
            } else {
                Bounds::new(self.input, config, diagnostics, control)?
            },
        );
        Ok(())
    }

    pub(crate) fn next(
        &mut self,
        config: &SolveConfig,
        diagnostics: &mut impl Write,
        control: &Control,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), RunError>> {
        if let Some(error) = self.pending_error.take() {
            return Some(Err(error));
        }
        if self.final_outcome.is_some() {
            return self.next_retained().map(Ok);
        }
        if !self.input.objectives.is_present()
            && config.models != 0
            && self.yielded >= config.models
        {
            self.complete(Completion::RequestedModels, None, phases);
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
                        Completion::Interrupted,
                        Some(Interruption::Countermodel(error)),
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
                    self.complete(Completion::Exhausted, None, phases);
                    return self.next_retained().map(Ok);
                }
            };
            let model = Model::new(
                interpretation
                    .atoms()
                    .map(|atom| self.input.atoms[atom].clone()),
            );
            if !self.input.objectives.is_present() {
                self.yielded += 1;
                return Some(Ok((model, None)));
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
                            diagnostics,
                            control,
                        );
                    if let Err(error) = improved {
                        self.fail(RunError::Output(error), phases);
                        return self.pending_error.take().map(Err);
                    }
                }
                Ok(false) => {}
                Err(reason) => {
                    self.complete(Completion::Interrupted, Some(reason), phases);
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

    fn complete(
        &mut self,
        completion: Completion,
        interruption: Option<Interruption>,
        phases: &Recorder,
    ) {
        let mut outcome = self.snapshot(phases);
        outcome.completion = Some(completion);
        outcome.interruption = interruption;
        self.final_outcome = Some(outcome);
        self.ready = self.incumbents.take_models();
    }

    fn fail(&mut self, error: RunError, phases: &Recorder) {
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
        if let Some(statistics) = statistics {
            phases.search(statistics.phase_timings);
        }
        SemanticOutcome {
            subject: Some(crate::Subject::Theory(self.input.theory.clone())),
            verified: statistics.map_or(0, |s| s.stable_models),
            scored: self.incumbents.scored(),
            retained: self.incumbents.retained(),
            completion: None,
            interruption: None,
            optimization: self.incumbents.metadata().cloned(),
            checked: statistics.map_or(0, |s| s.candidates),
            gate_atoms: self.input.gate_atoms,
            countermodel_statistics: statistics,
            lazy_execution: None,
            shared_execution: None,
            formula_execution: self
                .models
                .as_ref()
                .and_then(|models| self.execution.statistics(models)),
        }
    }
}
