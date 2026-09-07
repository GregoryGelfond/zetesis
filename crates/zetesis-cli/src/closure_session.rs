//! The ordinary closure loop retained across semantic pulls.

use std::io::Write;
use std::sync::Arc;

use zetesis_core::{GroundProgram, Model, Program};
use zetesis_cpu::{CandidateLimits, Candidates, Control, Stop};

use crate::engine::Engine;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{Completion, Interruption, RunError, SemanticOutcome, SolveConfig};

pub(crate) struct ClosureSession<'a> {
    program: &'a Program,
    candidates: Candidates<'a>,
    engine: Engine,
    ready: std::vec::IntoIter<Result<Option<Model>, Stop>>,
    finished_batch: bool,
    pending_stop: Option<Stop>,
    verified: u64,
    checked: u64,
    yielded: usize,
    completion: Option<Completion>,
    interruption: Option<Interruption>,
    terminal: bool,
}

impl<'a> ClosureSession<'a> {
    pub(crate) fn new(
        program: &'a Program,
        ground: Option<Arc<GroundProgram>>,
        config: &SolveConfig,
        diagnostics: &mut impl Write,
        control: &Control,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        let engine = phases.measure(SolvePhase::ExecutionSetup, || {
            Engine::with_ground(config, program, ground, diagnostics, phases)
        })?;
        let candidates = phases.measure(SolvePhase::CandidateSetup, || {
            Candidates::new(
                program,
                CandidateLimits {
                    max_candidates: config.max_candidates,
                    max_carrier_atoms: config.max_carrier_atoms,
                },
                control.clone(),
            )
        });
        Ok(Self {
            program,
            candidates,
            engine,
            ready: Vec::new().into_iter(),
            finished_batch: false,
            pending_stop: None,
            verified: 0,
            checked: 0,
            yielded: 0,
            completion: None,
            interruption: None,
            terminal: false,
        })
    }

    pub(crate) fn next(
        &mut self,
        config: &SolveConfig,
        diagnostics: &mut impl Write,
        control: &Control,
        phases: &Recorder,
    ) -> Option<Result<Model, RunError>> {
        if self.terminal {
            return None;
        }
        if config.models != 0 && self.yielded >= config.models {
            self.complete(Completion::RequestedModels, None);
            return None;
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
                        self.complete(Completion::Interrupted, Some(Interruption::Oracle(stop)));
                        return None;
                    }
                }
            }
            if let Some(stop) = self.pending_stop {
                self.complete(Completion::Interrupted, Some(Interruption::Oracle(stop)));
                return None;
            }
            if self.finished_batch {
                self.complete(Completion::Exhausted, None);
                return None;
            }
            let count = if self.checked == 0 {
                1
            } else {
                config.batch_size.get()
            };
            let mut seeds = Vec::new();
            let generation = phases.start(SolvePhase::CandidateGeneration);
            for _ in 0..count {
                match self.candidates.next() {
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
            match self
                .engine
                .check(config, self.program, &seeds, diagnostics, control, phases)
            {
                Ok(results) => {
                    self.verified +=
                        results.iter().filter(|r| matches!(r, Ok(Some(_)))).count() as u64;
                    self.ready = results.into_iter();
                }
                Err(error) => {
                    self.terminal = true;
                    return Some(Err(error));
                }
            }
        }
    }

    fn complete(&mut self, completion: Completion, interruption: Option<Interruption>) {
        self.completion = Some(completion);
        self.interruption = interruption;
        self.terminal = true;
    }

    pub(crate) const fn terminal(&self) -> bool {
        self.terminal
    }

    pub(crate) fn outcome(&self) -> SemanticOutcome {
        SemanticOutcome {
            subject: Some(crate::Subject::Program(self.program.clone())),
            verified: self.verified,
            scored: 0,
            retained: 0,
            completion: self.completion,
            interruption: self.interruption.or_else(|| {
                self.completion
                    .is_none()
                    .then_some(self.pending_stop)
                    .flatten()
                    .map(Interruption::Oracle)
            }),
            optimization: None,
            checked: self.checked,
            gate_atoms: self.candidates.discovered_atoms(),
            countermodel_statistics: None,
            formula_execution: None,
        }
    }
}
