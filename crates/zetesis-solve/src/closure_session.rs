//! The ordinary closure loop retained across semantic pulls.

use crate::execution_observation::ExecutionSink;
use std::sync::Arc;

use zetesis_core::{GroundProgram, Model, Program};
use zetesis_cpu::{
    Cancellation, CandidateLimits, CandidateRestrictionLimits, Candidates, Limits, Stop,
};

use crate::engine::{Engine, PreparationFailure};
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{
    ExecutionResources, Interruption, SearchState, SemanticOutcome, SolveConfig, SolveError,
};

pub(crate) struct ClosureSession<'a> {
    program: &'a Program,
    candidates: Candidates<'a>,
    engine: Result<Engine, Stop>,
    ready: std::vec::IntoIter<Result<Option<Model>, Stop>>,
    finished_batch: bool,
    pending_stop: Option<Stop>,
    pending_query_fault: Option<Arc<zetesis_cpu::BatchError>>,
    verified: u64,
    checked: u64,
    yielded: usize,
    search_state: Option<SearchState>,
    terminal: bool,
}

impl<'a> ClosureSession<'a> {
    pub(crate) fn with_resources(
        program: &'a Program,
        ground: Option<Arc<GroundProgram>>,
        config: &SolveConfig,
        resources: &ExecutionResources,
        observations: &mut impl ExecutionSink,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Result<Self, SolveError> {
        let engine = match phases.measure(SolvePhase::ExecutionSetup, || {
            Engine::with_ground(
                config,
                program,
                ground,
                resources,
                cancellation,
                observations,
                phases,
            )
        }) {
            Ok(engine) => Ok(engine),
            Err(PreparationFailure::Stopped(stop)) => Err(stop),
            Err(PreparationFailure::Run(error)) => return Err(error),
        };
        let candidates = phases.measure(SolvePhase::CandidateSetup, || {
            let mut candidates = Candidates::restricted(
                program,
                CandidateLimits {
                    max_candidates: config.max_candidates,
                    max_carrier_atoms: config.max_carrier_atoms,
                },
                CandidateRestrictionLimits {
                    max_work: config.max_search_work,
                    max_atoms: config.max_atoms,
                    max_bytes: config.max_candidate_bytes,
                },
                cancellation.clone(),
            );
            // The program's two closures, each charged as one candidate check,
            // bound the counter to the gate atoms some seed could derive and
            // not every seed must hold.
            candidates.bounded(Limits {
                max_work: config.max_work,
                max_derived_atoms: config.max_atoms,
                max_closure_bytes: config.max_closure_bytes,
            });
            candidates
        });
        Ok(Self {
            program,
            candidates,
            engine,
            ready: Vec::new().into_iter(),
            finished_batch: false,
            pending_stop: None,
            pending_query_fault: None,
            verified: 0,
            checked: 0,
            yielded: 0,
            search_state: None,
            terminal: false,
        })
    }

    pub(crate) fn next(
        &mut self,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Model, SolveError>> {
        if self.terminal {
            return None;
        }
        if let Err(stop) = &self.engine {
            self.complete(SearchState::Interrupted(Interruption::Preparation(*stop)));
            return None;
        }
        if config.models != 0 && self.yielded >= config.models {
            self.complete(SearchState::RequestedModels);
            return self.query_fault();
        }
        // All ready results are already checked. Consuming their prefix changes
        // neither verified membership nor the pending end-of-batch stop.
        loop {
            if let Some(result) = self.ready.next() {
                self.checked += 1;
                match result {
                    Ok(Some(model)) => {
                        self.yielded += 1;
                        return Some(Ok(model));
                    }
                    Ok(None) => continue,
                    Err(stop) => {
                        self.complete(SearchState::Interrupted(Interruption::Oracle(stop)));
                        return self.query_fault();
                    }
                }
            }
            if let Some(stop) = self.pending_stop {
                self.complete(SearchState::Interrupted(Interruption::Oracle(stop)));
                return self.query_fault();
            }
            if self.finished_batch {
                self.complete(SearchState::Exhausted);
                return self.query_fault();
            }
            if let Some(error) = self.query_fault() {
                return Some(error);
            }
            let count = if self.checked == 0 {
                1
            } else {
                config.batch_size.get()
            };
            let generation = phases.start(SolvePhase::CandidateGeneration);
            let mut seeds = match batch_storage(count, cancellation) {
                Ok(seeds) => seeds,
                Err(stop) => {
                    self.complete(SearchState::Interrupted(Interruption::Oracle(stop)));
                    return None;
                }
            };
            for _ in 0..count {
                match self.candidates.next_selection() {
                    Some(Ok(seed)) => seeds.push(seed),
                    Some(Err(stop)) => {
                        self.pending_stop = Some(stop);
                        break;
                    }
                    None => {
                        self.finished_batch = true;
                        break;
                    }
                }
            }
            drop(generation);
            let results = match &mut self.engine {
                Ok(engine) => engine.check(
                    config,
                    self.program,
                    &seeds,
                    self.candidates.prepared_queries(),
                    cancellation,
                    phases,
                ),
                Err(_) if seeds.is_empty() => Ok(Vec::new()),
                Err(stop) => Ok(vec![Err(*stop)]),
            };
            self.pending_query_fault = self
                .engine
                .as_ref()
                .ok()
                .and_then(Engine::query_observation)
                .and_then(|observation| observation.fault.clone());
            match results {
                Ok(results) => {
                    self.verified +=
                        results.iter().filter(|r| matches!(r, Ok(Some(_)))).count() as u64;
                    self.ready = results.into_iter();
                }
                Err(SolveError::Batch(zetesis_cpu::BatchError::Preparation(stop))) => {
                    self.complete(SearchState::Interrupted(Interruption::Preparation(stop)));
                    return self.query_fault();
                }
                Err(error) => {
                    self.terminal = true;
                    return Some(Err(error));
                }
            }
        }
    }

    fn query_fault(&mut self) -> Option<Result<Model, SolveError>> {
        self.pending_query_fault.take().map(|error| {
            self.terminal = true;
            Err(SolveError::QueryObservation(error))
        })
    }

    fn complete(&mut self, search_state: SearchState) {
        self.search_state = Some(search_state);
        self.terminal = true;
    }

    pub(crate) const fn terminal(&self) -> bool {
        self.terminal
    }

    pub(crate) fn outcome(&self) -> SemanticOutcome {
        SemanticOutcome {
            projection: None,
            subject: Some(crate::Subject::Program(self.program.clone())),
            selection: Some(crate::AnswerSelection::All),
            verified: self.verified,
            scored: 0,
            retained: 0,
            search_state: self.search_state.or_else(|| {
                self.pending_stop
                    .map(|stop| SearchState::PendingInterruption(Interruption::Oracle(stop)))
            }),
            optimization: None,
            objective_work: 0,
            checked: self.checked,
            gate_atoms: self.candidates.discovered_atoms(),
            candidate_statistics: Some(self.candidates.statistics()),
            countermodel_statistics: None,
            formula_execution: None,
            hybrid_execution: None,
            terminal_execution: None,
            model_construction: None,
            query_execution: self
                .engine
                .as_ref()
                .ok()
                .and_then(Engine::query_observation)
                .cloned(),
            closure_execution: self
                .engine
                .as_ref()
                .ok()
                .and_then(Engine::closure_statistics),
            shared_execution: self
                .engine
                .as_ref()
                .ok()
                .and_then(Engine::shared_statistics)
                .map(|mut statistics| {
                    statistics.queued_results = if statistics.last_stop.is_none() {
                        self.ready.len()
                    } else {
                        0
                    };
                    statistics
                }),
            lazy_execution: self
                .engine
                .as_ref()
                .ok()
                .and_then(|engine| engine.lazy_statistics(self.ready.len())),
        }
    }
}

// Reserve the occurrence slots before asking the generator for a candidate.
// Refusal therefore cannot consume an unsubmitted selection or discard an
// already checked batch prefix. Payload owners still belong to each selection.
fn batch_storage(
    count: usize,
    cancellation: &Cancellation,
) -> Result<Vec<zetesis_core::SeedSelection>, Stop> {
    cancellation.poll()?;
    let mut seeds = Vec::new();
    seeds
        .try_reserve_exact(count)
        .map_err(|_| Stop::Allocation)?;
    Ok(seeds)
}

#[cfg(test)]
mod storage_tests {
    use super::batch_storage;
    use zetesis_cpu::{Cancellation, Stop};

    #[test]
    fn capacity_refusal_is_typed_and_control_precedes_reservation() {
        assert!(matches!(
            batch_storage(usize::MAX, &Cancellation::default()),
            Err(Stop::Allocation)
        ));
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(matches!(
            batch_storage(usize::MAX, &cancelled),
            Err(Stop::Cancelled)
        ));
        let expired = Cancellation::with_deadline(std::time::Instant::now()).unwrap();
        assert!(matches!(
            batch_storage(usize::MAX, &expired),
            Err(Stop::Deadline)
        ));
        assert!(
            batch_storage(3, &Cancellation::default())
                .unwrap()
                .capacity()
                >= 3
        );
    }
}

#[cfg(test)]
mod query_tests;
