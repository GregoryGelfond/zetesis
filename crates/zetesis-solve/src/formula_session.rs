//! Stateful ordinary formula enumeration, scoring and incumbent retention.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;
use std::cell::Cell;
use std::cmp::Ordering;
use std::ops::ControlFlow;

use zetesis_core::{Model, ModelOrder};
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_stable::StableModels;

use crate::countermodel::Input;
use crate::formula_execution::{Failure, MembershipExecution};
use crate::objective_bounds::{Bounds, Preparation};
use crate::optimization::Incumbents;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{
    AnswerSelection, Interruption, Optimization, SearchState, SemanticOutcome, SolveConfig,
    SolveError,
};

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
    imported_timings: Cell<zetesis_stable::SearchPhaseTimings>,
    // The original-region filter an owning session installed, which a tie
    // pass's fresh stream installs again.
    region_filter: Option<std::sync::Arc<dyn zetesis_stable::RegionFilter>>,
    // While a tie pass runs: how many of the retained optima the strict
    // search found, which the pass meets again and must not retain twice.
    tie_pass: Option<usize>,
}

/// What one pull returns: the next answer with its score, a fault, or the end
/// of the answers.
type Next = Option<Result<(Model, Option<Score>), SolveError>>;

/// How the start of a tie pass ended.
enum TiePass {
    /// No tie pass is owed: the stream's end finishes the session.
    Skipped,
    /// A fresh stream bounded to cost at most the optimum replaced the
    /// exhausted one.
    Started,
    /// The pass could not start; the proved optimum stands, and the family
    /// is reported without a claim of completeness.
    Stopped(zetesis_stable::Incomplete),
    /// An execution or observation fault.
    Failed(SolveError),
}

impl<'a, E: MembershipExecution> FormulaSession<'a, E> {
    /// Install an original-only source restriction before the first proposal.
    /// An already stopped initialization has no traversal left to configure.
    pub(crate) fn set_region_filter(
        &mut self,
        filter: std::sync::Arc<dyn zetesis_stable::RegionFilter>,
        phases: &Recorder,
    ) {
        self.region_filter = Some(filter.clone());
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
    /// caller's shared token. A device execution then reads its last batch's
    /// compute-pass interval, unless the session's control, which it kept from
    /// construction, has fired. Every completion, failure and stop of this
    /// session or of an owning hybrid or terminal-definition session passes
    /// here. Refresh receipts after the workers and the execution have settled.
    pub(crate) fn stop(&mut self, phases: &Recorder) -> Result<(), zetesis_stable::Incomplete> {
        let result = self.models.as_mut().map_or(Ok(()), StableModels::stop);
        if let Some(execution) = self.execution.as_mut() {
            execution.settle(phases);
        }
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
            imported_timings: Cell::new(zetesis_stable::SearchPhaseTimings::default()),
            region_filter: None,
            tie_pass: None,
        }
    }

    /// The candidate stream this session's configuration selects over its
    /// theory, within `limits`: native region workers, device region
    /// producers, or one walk.
    fn candidate_stream(
        &self,
        config: &SolveConfig,
        limits: zetesis_stable::Limits,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Result<StableModels, zetesis_stable::Incomplete> {
        phases.measure(SolvePhase::CandidateSetup, || {
            // CPU workers decide their own leaves. Device producers return
            // unchecked leaves to the bounded batch protocol and join before
            // membership execution starts.
            if let Some(workers) = config.region_workers() {
                StableModels::with_region_workers(
                    self.input.theory,
                    workers,
                    limits,
                    cancellation.clone(),
                )
            } else if config.workers.get() > 1 {
                StableModels::with_region_producers(
                    self.input.theory,
                    config.workers,
                    limits,
                    cancellation.clone(),
                )
            } else {
                StableModels::new(self.input.theory, limits, cancellation.clone())
            }
        })
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
        let limits = crate::countermodel::search_limits(config);
        let models = self.candidate_stream(config, limits, cancellation, phases);
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
        if config.region_workers().is_none() && config.workers.get() > 1 {
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
        // An optimizing search tightens the candidate bound with each better
        // answer, so its producers share one frontier: production then stays
        // where the latest splits are, and older regions wait for the tighter
        // bounds that can refute them near the root.
        if self.selection == AnswerSelection::Optimal
            && self.bounds.is_enabled()
            && self
                .objective_plan
                .as_ref()
                .and_then(Preparation::plan)
                .is_some()
            && let Some(models) = self.models.as_mut()
            && let Err(error) = models.expect_candidate_bounds()
        {
            self.complete(
                SearchState::Interrupted(Interruption::Countermodel(error)),
                phases,
            );
        }
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
            let execution = self
                .execution
                .as_mut()
                .expect("unfinished session has a membership executor");
            // An enumeration owes the answers it has not yet yielded; a
            // request of zero is unbounded.
            let enumerates = self.selection == AnswerSelection::All;
            execution.demand(
                enumerates,
                (enumerates && config.models != 0)
                    .then(|| config.models.saturating_sub(self.yielded)),
            );
            let next = execution.next(models, config, cancellation, phases);
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
                None => match self.end_of_stream(config, observations, cancellation, phases) {
                    ControlFlow::Continue(()) => continue,
                    ControlFlow::Break(result) => return result,
                },
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
            if let ControlFlow::Break(result) = self.select_optimal(
                model,
                &interpretation,
                config,
                observations,
                cancellation,
                phases,
            ) {
                return result;
            }
        }
    }

    /// Score and retain one verified model of an optimizing search, and bound
    /// the candidates by it when it improves the incumbent. Continue to search
    /// on, or break with what the pull returns.
    fn select_optimal(
        &mut self,
        model: Model,
        interpretation: &zetesis_ferraris::Interpretation,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> ControlFlow<Next> {
        if let Some(revisit) = self.tie_pass
            && self.incumbents.holds_among(revisit, &model)
        {
            // The strict search already retained this optimum.
            return ControlFlow::Continue(());
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
                    .map(|plan| (plan, interpretation)),
            )
        });
        match scored {
            Ok(true) if self.tie_pass.is_some() => {
                // The strict search's exhaustion proved that nothing is
                // cheaper, so this cannot occur; if it did, the pass, under
                // a bound it never tightens, still meets every model at
                // the better cost, and the retained models are its own.
                self.tie_pass = Some(0);
            }
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
                    return ControlFlow::Break(self.pending_error.take().map(Err));
                }
            }
            Ok(false) => {}
            Err(reason) => {
                self.complete(SearchState::Interrupted(reason), phases);
                return ControlFlow::Break(self.next_retained().map(Ok));
            }
        }
        ControlFlow::Continue(())
    }

    /// At the end of a candidate stream, begin the tie pass when one is owed
    /// and continue to search it, or break with what the finished search
    /// returns.
    fn end_of_stream(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> ControlFlow<Next> {
        match self.begin_tie_pass(config, observations, cancellation, phases) {
            TiePass::Started => ControlFlow::Continue(()),
            TiePass::Skipped => ControlFlow::Break(self.finish_candidate_stream(phases)),
            TiePass::Stopped(error) => {
                self.complete(
                    SearchState::Interrupted(Interruption::Countermodel(error)),
                    phases,
                );
                ControlFlow::Break(self.next_retained().map(Ok))
            }
            TiePass::Failed(error) => {
                self.fail(error, phases);
                ControlFlow::Break(self.pending_error.take().map(Err))
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

    /// At the end of a candidate stream under strict bounds, start the tie
    /// pass when one is owed: more than one optimal answer is requested, and
    /// the last installed bound, strict at the optimum, pruned its ties. The
    /// exhausted stream proved that no model is cheaper than the optimum; a
    /// fresh stream over the same theory, with the same certificate and region
    /// filter and bounded to cost at most the optimum, then enumerates every
    /// optimal model, each once. It reports its receipts after the exhausted
    /// stream's, and only its exhaustion completes the optimal family.
    fn begin_tie_pass(
        &mut self,
        config: &SolveConfig,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> TiePass {
        if self.tie_pass.is_some()
            || self.selection != AnswerSelection::Optimal
            || config.models == 1
        {
            return TiePass::Skipped;
        }
        let Some(optimum) = self.incumbents.score().cloned() else {
            return TiePass::Skipped;
        };
        if self
            .bounds
            .pruned_ties_at()
            .is_none_or(|pruned| pruned.compare_costs(&optimum) != Ordering::Equal)
        {
            return TiePass::Skipped;
        }
        let earlier = self
            .models
            .as_mut()
            .expect("unfinished session has a candidate stream");
        if !earlier.exhausted() {
            // Finishing the stream reports that it did not exhaust.
            return TiePass::Skipped;
        }
        if let Err(error) = earlier.stop() {
            return TiePass::Stopped(error);
        }
        // The run's cumulative limits cover both passes together.
        let limits =
            crate::countermodel::search_limits(config).remaining_after(&earlier.statistics());
        let mut fresh = match self.candidate_stream(config, limits, cancellation, phases) {
            Ok(fresh) => fresh,
            Err(error) => return TiePass::Stopped(error),
        };
        if config.stats {
            fresh.enable_phase_timing();
        }
        if let Err(error) = fresh.follow(self.models.as_ref().expect("exhausted stream")) {
            return TiePass::Stopped(error);
        }
        // The fresh stream reports the exhausted one's receipts with its own,
        // so it owns them before any fallible step, whose attempted work then
        // remains in the session's receipts.
        let fresh = self.models.insert(fresh);
        // The same certificate as the strict search's, already reported.
        match crate::countermodel::prepare_certificate(
            fresh,
            self.input.certificate_order,
            config,
            &mut crate::execution_observation::Ignore,
            phases,
        ) {
            Ok(None) => {}
            Ok(Some(error)) => return TiePass::Stopped(error),
            Err(error) => return TiePass::Failed(error),
        }
        if let Some(filter) = self.region_filter.clone()
            && let Err(error) = fresh.set_region_filter(filter)
        {
            return TiePass::Stopped(error);
        }
        if let Err(error) = observations.record(Event::ObjectiveTiePass {
            costs: optimum.costs(),
        }) {
            return TiePass::Failed(error);
        }
        if let Err(error) = self.bounds.bound_ties(
            self.objective_plan.as_ref().and_then(Preparation::plan),
            &optimum,
            fresh,
            config,
            observations,
            cancellation,
        ) {
            return TiePass::Failed(error);
        }
        self.tie_pass = Some(self.incumbents.begin_tie_pass());
        TiePass::Started
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
            optimization: self.incumbents.metadata().map(|best| Optimization {
                // A strict bound at the optimum pruned its ties, and no tie
                // pass began to count them again.
                ties_pruned: self.tie_pass.is_none()
                    && self
                        .bounds
                        .pruned_ties_at()
                        .is_some_and(|pruned| pruned.compare_costs(&best.score) == Ordering::Equal),
                ..best.clone()
            }),
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
            source_evaluation: None,
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

    fn import_timings(
        &self,
        phases: &Recorder,
        timing: Option<zetesis_stable::SearchPhaseTimings>,
    ) {
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
                    cancellation,
                    observations,
                )
            })?);
        }
        Ok(session)
    }
}
