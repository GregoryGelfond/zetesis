//! Common-loop failure evidence with real native membership and an injected checker.
//! The adapter label explicitly identifies a test double, never physical GPU evidence.

use std::io;
use std::num::NonZeroUsize;

use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Theory};
use zetesis_sat::{BatchVerdict, StableModels};

use super::test_harness::{admitted, input, run};
use crate::formula_execution::{Failure, FormulaExecutionStatistics, MembershipExecution};
use crate::formula_queue::BatchQueue;
use crate::phase_timing::Recorder;
use crate::{Backend, Oracle, SearchMethod, SolveConfig, SolveError};

#[derive(Default)]
struct Injected {
    queue: BatchQueue,
    calls: usize,
    fail_on: usize,
    shape_on: usize,
}
impl Injected {
    fn check(&mut self, candidates: &[Interpretation]) -> Result<Vec<BatchVerdict>, Failure> {
        self.calls += 1;
        if self.calls == self.fail_on {
            return Err(Failure::Run(SolveError::ExecutionObservation(Box::new(
                io::Error::new(
                    io::ErrorKind::ConnectionReset,
                    "original checker transport failure",
                ),
            ))));
        }
        let count = candidates.len() - usize::from(self.calls == self.shape_on);
        Ok(vec![BatchVerdict::Residual; count])
    }
}
impl MembershipExecution for Injected {
    fn next(
        &mut self,
        models: &mut StableModels,
        options: &crate::SolveConfig,
        control: &Control,
        _: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        let mut queue = std::mem::take(&mut self.queue);
        let result = queue.next(models, options, control, |_: &Theory, candidates| {
            self.check(candidates)
        });
        self.queue = queue;
        result
    }
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics> {
        let batch = models.batch_statistics();
        Some(FormulaExecutionStatistics {
            gpu_limits: None,
            gpu_submitted_batches: 0,
            gpu_submitted_candidates: 0,
            adapter: "injected native checker; no physical device".into(),
            gpu_batches: 0,
            gpu_candidates: 0,
            gpu_work: 0,
            gpu_rounds: 0,
            gpu_decided: 0,
            cpu_residuals: batch.residuals,
            pending_candidates: batch.pending,
            queued_models: self.queue.len(),
            peak_accounted_bytes: 0,
            completion: self.queue.accounting(),
        })
    }
}

/// The batched protocol the injected checker takes part in belongs to
/// the clause search.
fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        search: SearchMethod::Clauses,
        oracle: Oracle::Countermodel,
        models: 0,
        batch_size: NonZeroUsize::new(2).unwrap(),
        max_objective_bound_work: 0,
        ..Default::default()
    }
}

fn require_external_error(error: &SolveError) {
    let SolveError::ExecutionObservation(error) = error else {
        panic!("the injected external failure changed type: {error:?}");
    };
    let error = error.downcast_ref::<io::Error>().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::ConnectionReset);
    assert_eq!(error.to_string(), "original checker transport failure");
}

#[test]
fn late_checker_failure_retains_the_checked_prefix() {
    let owner = admitted("{a;b}.");
    let capture = run(
        &owner,
        &config(),
        &Control::default(),
        &mut Injected {
            fail_on: 2,
            ..Default::default()
        },
    );
    require_external_error(capture.error.as_ref().unwrap());
    assert_eq!(capture.answers.len(), 2);
    assert_ne!(capture.answers[0].0, capture.answers[1].0);
    assert_eq!(capture.outcome.verified_models(), 2);
    assert_eq!(capture.outcome.candidate_progress(), 4);
    assert_eq!(capture.outcome.completion(), None);
    let execution = capture.outcome.formula_execution().unwrap();
    assert_eq!(
        (
            execution.pending_candidates,
            execution.queued_models,
            execution.cpu_residuals
        ),
        (2, 0, 2)
    );
}

#[test]
fn checker_failure_retains_unyielded_incumbent_ties() {
    let owner = admitted("{a;b}. #minimize{0:a;0:b}. #show.");
    let capture = run(
        &owner,
        &config(),
        &Control::default(),
        &mut Injected {
            fail_on: 2,
            ..Default::default()
        },
    );
    require_external_error(capture.error.as_ref().unwrap());
    assert!(capture.answers.is_empty());
    assert_eq!(capture.outcome.verified_models(), 2);
    assert_eq!(capture.outcome.retained_models(), 2);
    assert_eq!(
        capture
            .outcome
            .formula_execution()
            .unwrap()
            .pending_candidates,
        2
    );
    let incumbent = capture.outcome.incumbent().unwrap();
    assert_eq!((incumbent.tied_models, incumbent.scored_models), (2, 2));
    assert_eq!(incumbent.score.costs(), [(0, 0)]);
    assert_eq!(capture.outcome.completion(), None);
    assert!(!capture.outcome.optimum_proved());
}

#[test]
fn protocol_failure_retains_uncommitted_candidates() {
    let owner = admitted("{a;b}.");
    let config = SolveConfig {
        stats: true,
        ..config()
    };
    let capture = run(
        &owner,
        &config,
        &Control::default(),
        &mut Injected {
            shape_on: 1,
            ..Default::default()
        },
    );
    assert!(matches!(
        capture.error,
        Some(SolveError::FormulaBatchShape {
            expected: 2,
            actual: 1
        })
    ));
    assert!(capture.answers.is_empty());
    assert_eq!(capture.outcome.verified_models(), 0);
    assert_eq!(capture.outcome.candidate_progress(), 2);
    assert_eq!(
        capture
            .outcome
            .formula_execution()
            .unwrap()
            .pending_candidates,
        2
    );
    assert_eq!(capture.outcome.completion(), None);
    assert!(capture.timings.is_some());
}

#[test]
fn stopping_pulls_preserves_unconsumed_membership() {
    let owner = admitted("{a;b}.");
    let config = SolveConfig {
        batch_size: NonZeroUsize::new(3).unwrap(),
        ..config()
    };
    let control = Control::default();
    let phases = Recorder::new(false);
    let mut execution = Injected::default();
    let mut session = crate::formula_session::FormulaSession::with_selection(
        input(&owner),
        &mut execution,
        &config,
        &mut crate::execution_observation::Ignore,
        &control,
        &phases,
        crate::AnswerSelection::All,
    );
    let first = session
        .next(
            &config,
            &mut crate::execution_observation::Ignore,
            &control,
            &phases,
        )
        .unwrap()
        .unwrap();
    assert!(first.0.atoms().is_empty());
    let outcome = session.outcome(&phases);
    assert_eq!(outcome.verified_models(), 3);
    let statistics = outcome.formula_execution().unwrap();
    assert_eq!(
        (statistics.pending_candidates, statistics.queued_models),
        (0, 2)
    );
    assert_eq!(outcome.countermodel_statistics().unwrap().stable_models, 3);
    assert_eq!(outcome.completion(), None);
}

#[test]
fn checker_failure_does_not_recount_completed_residuals() {
    let owner = admitted("{a;b}.");
    for workers in [1, 2, 4] {
        for shape in [false, true] {
            let config = SolveConfig {
                completion_workers: NonZeroUsize::new(workers).unwrap(),
                ..config()
            };
            let mut execution = Injected {
                queue: BatchQueue::new(&config).unwrap(),
                fail_on: if shape { 0 } else { 2 },
                shape_on: if shape { 2 } else { 0 },
                ..Default::default()
            };
            let capture = run(&owner, &config, &Control::default(), &mut execution);
            let error = capture.error.as_ref().unwrap();
            if shape {
                assert!(matches!(
                    error,
                    SolveError::FormulaBatchShape {
                        expected: 2,
                        actual: 1
                    }
                ));
            } else {
                require_external_error(error);
            }
            let statistics = capture.outcome.formula_execution().unwrap();
            assert_eq!(statistics.completion.entered, 2);
            assert_eq!(statistics.completion.residual_completed, 2);
            assert_eq!(statistics.completion.failed, 0);
            assert_eq!(statistics.pending_candidates, 2);
            assert_eq!(capture.answers.len(), 2);
            assert_eq!(capture.outcome.verified_models(), 2);
        }
    }
}
