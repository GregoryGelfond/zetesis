//! Stateful ordinary formula enumeration, scoring and incumbent retention.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;
use std::cell::Cell;

use zetesis_core::{Model, ModelOrder};
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_sat::StableModels;

use crate::countermodel::Input;
use crate::formula_execution::{Failure, MembershipExecution};
use crate::objective_bounds::{Bounds, Preparation};
use crate::optimization::Incumbents;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{AnswerSelection, Interruption, SearchState, SemanticOutcome, SolveConfig, SolveError};

pub(crate) struct FormulaSession<'a, E> {
    input: Input<'a>,
    selection: AnswerSelection,
    execution: Option<E>,
    models: Option<StableModels>,
    model_order: Option<ModelOrder<'a>>,
    construction: Option<crate::model_construction::Account>,
    objective_plan: Option<Preparation>,
    bounds: Bounds,
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
    /// Install an original-only source restriction before the first proposal.
    /// An already stopped initialization has no traversal left to configure.
    pub(crate) fn set_region_filter(
        &mut self,
        filter: std::sync::Arc<dyn zetesis_sat::RegionFilter>,
        phases: &Recorder,
    ) {
        if let Some(models) = self.models.as_mut()
            && let Err(error) = models.set_region_filter(filter)
        {
            self.complete(
                SearchState::Interrupted(Interruption::Countermodel(error)),
                phases,
            );
        }
    }

    /// Stop candidate production and join native workers without cancelling the
    /// caller's shared token. Refresh receipts after the workers have settled.
    pub(crate) fn stop(&mut self, phases: &Recorder) -> Result<(), zetesis_sat::Incomplete> {
        let result = self.models.as_mut().map_or(Ok(()), StableModels::stop);
        if let Some(previous) = self.final_outcome.take() {
            let mut outcome = self.snapshot(phases);
            outcome.search_state = previous.search_state;
            // Completion already moved these models into the delivery queue.
            // Joining workers refreshes search receipts, not retained evidence.
            outcome.retained = previous.retained;
            self.final_outcome = Some(outcome);
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn with_selection(
        input: Input<'a>,
        execution: E,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
        selection: AnswerSelection,
    ) -> Self {
        let mut session = Self::uninitialized(input, selection);
        session.execution = Some(execution);
        if let Err(error) = session.initialize(config, observations, cancellation, phases) {
            session.fail(error, phases);
        }
        session
    }

    fn uninitialized(input: Input<'a>, selection: AnswerSelection) -> Self {
        Self {
            input,
            selection: if input.objectives.is_present() {
                selection
            } else {
                AnswerSelection::All
            },
            execution: None,
            models: None,
            model_order: None,
            construction: None,
            objective_plan: None,
            bounds: Bounds::default(),
            incumbents: Incumbents::default(),
            ready: Vec::new().into_iter(),
            yielded: 0,
            final_outcome: None,
            pending_error: None,
            imported_timings: Cell::new(zetesis_sat::SearchPhaseTimings::default()),
        }
    }

    fn initialize(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Result<(), SolveError> {
        observations.record(Event::Formula {
            atoms: self.input.theory.atom_count(),
            nodes: self.input.theory.nodes().len(),
            operands: self.input.theory.parts().occurrences(),
            roots: self.input.theory.roots().len(),
            keyed_constraints: self.input.keyed_constraints,
        })?;
        if let zetesis_themelios::KeyAnalysis::Stopped(stop) = self.input.key_analysis {
            observations.record(Event::KeyAnalysisStopped(stop))?;
        }
        let account = self
            .construction
            .insert(crate::model_construction::Account::default());
        let prepared = phases.measure(SolvePhase::ModelConstruction, || {
            account.prepare(self.input.atoms, config, cancellation)
        });
        match prepared {
            Ok(order) => self.model_order = Some(order),
            Err(crate::model_construction::Failure::Interrupted(stop)) => {
                self.complete(
                    SearchState::Interrupted(Interruption::ModelConstruction(stop)),
                    phases,
                );
                return Ok(());
            }
            Err(crate::model_construction::Failure::Run(error)) => return Err(error),
        }
        let models = phases.measure(SolvePhase::CandidateSetup, || {
            // CPU workers decide their own leaves. Device producers return
            // unchecked leaves to the bounded batch protocol and join before
            // membership execution starts.
            if let Some(workers) = config.region_workers() {
                StableModels::with_region_workers(
                    self.input.theory,
                    workers,
                    crate::countermodel::search_limits(config),
                    cancellation.clone(),
                )
            } else if config.search == crate::SearchMethod::Regions && config.workers.get() > 1 {
                StableModels::with_region_producers(
                    self.input.theory,
                    config.workers,
                    crate::countermodel::search_limits(config),
                    cancellation.clone(),
                )
            } else {
                StableModels::with_method(
                    self.input.theory,
                    config.search,
                    crate::countermodel::search_limits(config),
                    cancellation.clone(),
                )
            }
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
        if config.region_workers().is_none()
            && config.search == crate::SearchMethod::Regions
            && config.workers.get() > 1
        {
            observations.record(Event::ParallelProposals {
                workers: config.workers,
            })?;
        }
        let models = self.models.as_mut().expect("candidate stream installed");
        if let Some(error) = crate::countermodel::prepare_certificate(
            models,
            self.input.certificate_order,
            config,
            observations,
            phases,
        )? {
            self.complete(
                SearchState::Interrupted(Interruption::Countermodel(error)),
                phases,
            );
            return Ok(());
        }
        self.prepare_objectives(config, observations, cancellation, phases)
    }

    /// Retain and charge preparation before exposing its optional refusal.
    fn prepare_objectives(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Result<(), SolveError> {
        if self.input.objectives.is_present() {
            self.objective_plan =
                Some(phases.measure(SolvePhase::ObjectiveScoringRetention, || {
                    let mut preparation = Preparation::new(self.input, config, cancellation);
                    if self.selection == AnswerSelection::Optimal {
                        preparation.prepare_choices(
                            self.input.required_choices,
                            config,
                            cancellation,
                        );
                    }
                    preparation
                }));
            let preparation = self
                .objective_plan
                .as_ref()
                .expect("prepared objective attempt");
            if let Err(reason) = self.incumbents.prepare(preparation.work()) {
                self.complete(SearchState::Interrupted(reason), phases);
                return Ok(());
            }
            preparation.observe(observations)?;
        }
        self.bounds = Bounds::new(config);
        Ok(())
    }

    pub(crate) fn next(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        self.next_with_acceptance(config, observations, cancellation, phases, &mut |_| {
            Ok(true)
        })
    }

    /// Qualify each newly constructed model before scoring, retention or bound
    /// feedback. Ordinary formula execution uses a statically specialized
    /// identity callback. Retained answers bypass this callback.
    ///
    /// A callback failure returns before classifying completion: its owner must
    /// settle shared failures and workers, then call [`Self::conclude`]. It must
    /// not resume candidate production after that failure.
    pub(crate) fn next_with_acceptance(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
        accept: &mut impl FnMut(&Model) -> Result<bool, SolveError>,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        let next = self.next_result(config, observations, cancellation, phases, accept);
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
        cancellation: &Cancellation,
        phases: &Recorder,
        accept: &mut impl FnMut(&Model) -> Result<bool, SolveError>,
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
            let next = self
                .execution
                .as_mut()
                .expect("unfinished session has a membership executor")
                .next(models, config, cancellation, phases);
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
                None => return self.finish_candidate_stream(phases),
            };
            let model =
                match self.construct_model(interpretation.atoms(), config, cancellation, phases) {
                    Ok(model) => model,
                    Err(failure) => return self.finish_construction_failure(failure, phases),
                };
            match accept(&model) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => return Some(Err(error)),
            }
            if !self.input.objectives.is_present() {
                self.yielded += 1;
                return Some(Ok((model, None)));
            }
            if self.selection == AnswerSelection::All {
                return self.score_model(model, &interpretation, config, cancellation, phases);
            }
            let scored = phases.measure(SolvePhase::ObjectiveScoringRetention, || {
                self.incumbents.consider(
                    self.input.objectives,
                    model,
                    config,
                    cancellation,
                    self.objective_plan
                        .as_ref()
                        .and_then(Preparation::plan)
                        .map(|plan| (plan, &interpretation)),
                )
            });
            match scored {
                Ok(true) => {
                    let _feedback = (config.max_objective_bound_work != 0)
                        .then(|| phases.start(SolvePhase::ObjectiveFeedback));
                    let improved = self.bounds.improve(
                        self.objective_plan.as_ref().and_then(Preparation::plan),
                        self.incumbents
                            .score()
                            .expect("retained improvement has a score"),
                        self.models.as_mut().expect("owned candidate stream"),
                        config,
                        observations,
                        cancellation,
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

    /// Score one verified model without selecting or retaining an incumbent.
    fn score_model(
        &mut self,
        model: Model,
        interpretation: &zetesis_ferraris::Interpretation,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        let score = phases.measure(SolvePhase::ObjectiveScoringRetention, || {
            self.incumbents.evaluate(
                self.input.objectives,
                &model,
                config,
                cancellation,
                self.objective_plan
                    .as_ref()
                    .and_then(Preparation::plan)
                    .map(|plan| (plan, interpretation)),
            )
        });
        match score {
            Ok(score) => {
                self.yielded += 1;
                Some(Ok((model, Some(score))))
            }
            Err(reason) => {
                self.complete(SearchState::Interrupted(reason), phases);
                None
            }
        }
    }

    fn construct_model(
        &mut self,
        positions: impl IntoIterator<Item = usize>,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Result<Model, crate::model_construction::Failure> {
        phases.measure(SolvePhase::ModelConstruction, || {
            self.construction
                .as_mut()
                .expect("initialized construction account")
                .select(
                    self.model_order.as_ref().expect("prepared model order"),
                    positions,
                    config,
                    cancellation,
                )
        })
    }

    fn finish_construction_failure(
        &mut self,
        failure: crate::model_construction::Failure,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        // Settle workers before snapshotting their checked prefix, preserving
        // the construction failure over any secondary cleanup failure.
        let _ = self.models.as_mut().expect("owned candidate stream").stop();
        match failure {
            crate::model_construction::Failure::Interrupted(stop) => {
                self.complete(
                    SearchState::Interrupted(Interruption::ModelConstruction(stop)),
                    phases,
                );
                self.next_retained().map(Ok)
            }
            crate::model_construction::Failure::Run(error) => {
                self.fail(error, phases);
                self.pending_error.take().map(Err)
            }
        }
    }

    fn finish_candidate_stream(
        &mut self,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        if !self
            .models
            .as_ref()
            .expect("unfinished session has a candidate stream")
            .exhausted()
        {
            self.fail(SolveError::CandidateStreamNotExhausted, phases);
            return self.pending_error.take().map(Err);
        }
        self.complete(SearchState::Exhausted, phases);
        self.next_retained().map(Ok)
    }

    pub(crate) fn next_retained(&mut self) -> Option<(Model, Option<Score>)> {
        self.ready.next().map(|model| {
            let score = self.incumbents.score().cloned();
            (model, score)
        })
    }

    /// Apply an enclosing acceptance stage's settled classification. The caller
    /// has joined workers and resolved any shared original-source failure first.
    /// Reclassification preserves an already draining queue; an unclassified
    /// fault retains its evidence but publishes no more answers.
    pub(crate) fn conclude(&mut self, state: Option<SearchState>, phases: &Recorder) {
        if let Some(outcome) = &mut self.final_outcome {
            outcome.search_state = state;
        } else if let Some(state) = state {
            self.complete(state, phases);
        } else {
            self.final_outcome = Some(self.snapshot(phases));
        }
        if state.is_none() {
            self.ready = Vec::new().into_iter();
        }
    }

    fn complete(&mut self, search_state: SearchState, phases: &Recorder) {
        let cleanup = self.stop(phases);
        let mut outcome = self.snapshot(phases);
        outcome.search_state = crate::completion::after_cleanup(Some(search_state), cleanup);
        self.final_outcome = Some(outcome);
        self.ready = self.incumbents.take_models();
    }

    fn fail(&mut self, error: SolveError, phases: &Recorder) {
        // Preserve the primary fault, but settle worker receipts before
        // publishing its checked prefix. Live progress never performs this join.
        let _ = self.stop(phases);
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
            projection: None,
            subject: Some(crate::Subject::Theory(self.input.theory.clone())),
            selection: Some(self.selection),
            verified: statistics.map_or(0, |s| s.stable_models),
            scored: self.incumbents.scored(),
            retained: self.incumbents.retained(),
            search_state: None,
            optimization: self.incumbents.metadata().cloned(),
            objective_work: self.incumbents.work(),
            checked: statistics.map_or(0, |s| s.candidates),
            gate_atoms: self.input.gate_atoms,
            candidate_statistics: None,
            countermodel_statistics: statistics,
            lazy_execution: None,
            shared_execution: None,
            closure_execution: None,
            query_execution: None,
            hybrid_execution: None,
            terminal_execution: None,
            model_construction: self
                .construction
                .as_ref()
                .map(crate::model_construction::Account::statistics),
            formula_execution: self
                .models
                .as_ref()
                .and_then(|models| self.execution.as_ref()?.statistics(models)),
        }
    }

    fn import_timings(&self, phases: &Recorder, timing: Option<zetesis_sat::SearchPhaseTimings>) {
        if let Some(timing) = timing {
            phases.search(self.imported_timings.replace(timing), timing);
        }
    }
}

#[cfg(test)]
mod tests;

impl<'a> FormulaSession<'a, crate::formula_execution::Execution> {
    /// Own semantic preparation before choosing its execution route. A stopped
    /// preparation retains its receipts without constructing a device pipeline.
    /// Route construction keeps the ordinary session's fallible start door.
    pub(crate) fn with_resources(
        input: Input<'a>,
        config: &SolveConfig,
        resources: &crate::ExecutionResources,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
        selection: AnswerSelection,
    ) -> Result<Self, SolveError> {
        let mut session = Self::uninitialized(input, selection);
        if let Err(error) = session.initialize(config, observations, cancellation, phases) {
            session.fail(error, phases);
        }
        if session.final_outcome.is_none() {
            let plan = session
                .models
                .as_ref()
                .and_then(StableModels::prepared_tight_certificate);
            session.execution = Some(phases.measure(SolvePhase::ExecutionSetup, || {
                crate::formula_execution::Execution::with_resources(
                    config,
                    resources,
                    plan,
                    observations,
                )
            })?);
        }
        Ok(session)
    }
}
