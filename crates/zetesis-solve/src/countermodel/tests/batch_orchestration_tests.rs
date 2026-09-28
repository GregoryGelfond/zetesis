//! The production queue/common loop tested with actual exact native membership.
//! No device is constructed, and these tests make no GPU execution/parity claim.

use std::num::NonZeroUsize;

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Theory};
use zetesis_sat::{BatchStatistics, BatchVerdict, Incomplete, StableModels, Statistics};

use super::test_harness::{Record, admitted, run, run_consuming};
use crate::formula_execution::{Failure, FormulaExecutionStatistics, MembershipExecution};
use crate::formula_queue::BatchQueue;
use crate::{
    Backend, Completion, Interruption, Oracle, PreparedInput, Session, SolveConfig, SolveError,
};

/// How the native test route answers each candidate of a batch.
#[derive(Clone, Copy, Default)]
enum Answers {
    /// Leave every candidate to the host's exact completion.
    #[default]
    Residual,
    /// Decide every candidate with the reference reduct checker.
    Decided,
    /// Decide even positions and leave odd positions to exact completion.
    Mixed,
    /// Contradict the host's own original-model check.
    NotModel,
}

/// One deviation the native test route introduces into the batch protocol.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Fault {
    #[default]
    None,
    /// Exhaust native encoding work on the first candidate.
    Work,
    /// Return one verdict fewer than the batch has candidates.
    OmitVerdict,
    /// Cancel the session after answering, before the host commits.
    CancelAfterAnswering,
    /// Fail every batch after the first.
    AfterFirst,
}

#[derive(Default)]
struct NativeBatch {
    queue: BatchQueue,
    snapshots: Vec<(BatchStatistics, Statistics, usize)>,
    proposed: Vec<Vec<Vec<usize>>>,
    stop: Option<Incomplete>,
    answers: Answers,
    fault: Fault,
}

/// The reference checker's reduct verdict for one original model.
fn decide(
    theory: &Theory,
    candidate: &Interpretation,
    cancellation: &Cancellation,
) -> Result<BatchVerdict, Failure> {
    let check = zetesis_ferraris::check(
        theory,
        candidate,
        zetesis_ferraris::Limits::default(),
        cancellation,
    )
    .map_err(|stop| Failure::Search(stop.into()))?;
    Ok(match check.verdict() {
        zetesis_ferraris::Verdict::Stable => BatchVerdict::NoProperSubset,
        zetesis_ferraris::Verdict::NotModel { .. } => BatchVerdict::NotModel,
        zetesis_ferraris::Verdict::NonMinimal { .. } => BatchVerdict::Refuted,
    })
}

impl MembershipExecution for NativeBatch {
    fn next(
        &mut self,
        models: &mut StableModels,
        options: &crate::SolveConfig,
        cancellation: &Cancellation,
        _: &crate::phase_timing::Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        let result = self.queue.next(models, options, cancellation, |batch| {
            let theory = batch.theory();
            let candidates = batch.candidates();
            self.proposed.push(
                candidates
                    .iter()
                    .map(|candidate| candidate.atoms().collect())
                    .collect(),
            );
            if let Some(stop) = self.stop {
                return Err(Failure::Search(stop));
            }
            if self.fault == Fault::Work {
                let limits = zetesis_sat::Limits {
                    search: zetesis_sat::SearchLimits {
                        max_work: 0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                if let zetesis_sat::Check::Inconclusive(error) =
                    zetesis_sat::check(theory, &candidates[0], limits, cancellation)
                {
                    return Err(Failure::Search(error));
                }
                panic!("the selected nonempty source needs native encoding work");
            }
            if self.fault == Fault::AfterFirst && self.proposed.len() > 1 {
                // A fault on a later batch; the boundary's protocol error stands
                // in for any fault a route reports.
                return Err(Failure::from(crate::ExecutorError::ForeignBatch));
            }
            // A residual verdict sends the candidate to the real exact native
            // reduct checker inside StableModels; a decided one is the
            // reference checker's own reduct verdict.
            let mut verdicts = Vec::new();
            verdicts
                .try_reserve_exact(candidates.len())
                .map_err(|_| Failure::Search(Incomplete::Allocation))?;
            for (position, candidate) in candidates.iter().enumerate() {
                verdicts.push(match self.answers {
                    Answers::Residual => BatchVerdict::Residual,
                    Answers::NotModel => BatchVerdict::NotModel,
                    Answers::Mixed if position % 2 == 1 => BatchVerdict::Residual,
                    Answers::Decided | Answers::Mixed => decide(theory, candidate, cancellation)?,
                });
            }
            if self.fault == Fault::OmitVerdict {
                verdicts.pop();
            }
            if self.fault == Fault::CancelAfterAnswering {
                cancellation.cancel();
            }
            batch.finish(verdicts).map_err(Failure::from)
        });
        self.snapshots.push((
            models.batch_statistics(),
            models.statistics(),
            self.queue.len(),
        ));
        result
    }

    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        // Actual native batches have no physical adapter, GPU work or upload counters.
        None
    }
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        oracle: Oracle::Countermodel,
        workers: NonZeroUsize::MIN,
        models: 0,
        batch_size: NonZeroUsize::new(3).unwrap(),
        ..Default::default()
    }
}

#[test]
fn native_batches_preserve_complete_full_answers() {
    for source in [
        "",
        ":-.",
        "a | b.",
        "{a;b;c}. p:-p.",
        "1 {a;b;c} 1. #show.",
        "{p;-p}. #show x.",
        "{a;b}. #show a:a. #minimize{1,a:a;1,b:b}.",
    ] {
        let owner = admitted(source);
        let config = config();
        let mut scalar = Session::new(
            PreparedInput::formula(&owner),
            config,
            Cancellation::default(),
        )
        .unwrap();
        let mut expected: Vec<Record> = scalar
            .by_ref()
            .map(|answer| {
                let answer = answer.unwrap();
                (
                    answer.interpretation().clone(),
                    answer.score().map(|score| score.costs().to_vec()),
                )
            })
            .collect();
        expected.sort();
        let mut execution = NativeBatch::default();
        let actual = run(&owner, &config, &Cancellation::default(), &mut execution);
        assert!(actual.error.is_none(), "{source}: {:?}", actual.error);
        assert_eq!(actual.outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(actual.records(), expected, "{source}");
        let (batch, search, queued) = execution.snapshots.last().unwrap();
        assert_eq!((batch.pending, *queued), (0, 0));
        assert_eq!(batch.committed, search.candidates);
        assert_eq!(batch.residuals, search.candidates);
        assert_eq!(batch.propagated, 0);
        assert!(execution.proposed.iter().all(|batch| batch.len() <= 3));
        assert!(actual.outcome.formula_execution().is_none());
    }
}

#[test]
fn requested_model_limit_preserves_queued_membership() {
    let owner = admitted("{a;b;c}.");
    let mut execution = NativeBatch::default();
    let capture = run(
        &owner,
        &SolveConfig {
            models: 1,
            ..config()
        },
        &Cancellation::default(),
        &mut execution,
    );
    assert!(capture.error.is_none());
    assert_eq!(
        capture.outcome.completion(),
        Some(Completion::RequestedModels)
    );
    assert_eq!(capture.answers.len(), 1);
    assert_eq!(capture.outcome.verified_models(), 3);
    assert_eq!(capture.outcome.candidate_progress(), 3);
    assert_eq!(execution.queue.len(), 2);
    assert_eq!(execution.snapshots[0].0.committed, 3);
}

#[test]
fn proposal_limit_delivers_the_committed_prefix() {
    let owner = admitted("{a;b;c}.");
    let mut execution = NativeBatch::default();
    let capture = run(
        &owner,
        &SolveConfig {
            max_candidates: 2,
            ..config()
        },
        &Cancellation::default(),
        &mut execution,
    );
    assert!(capture.error.is_none());
    assert_eq!(capture.outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        capture.outcome.interruption(),
        Some(Interruption::Countermodel(Incomplete::CandidateLimit))
    );
    assert_eq!(capture.answers.len(), 2);
    assert_eq!(capture.outcome.verified_models(), 2);
    assert_eq!(capture.outcome.candidate_progress(), 2);
    assert_eq!(execution.queue.len(), 0);
    let expected: Vec<Record> = execution.proposed[0]
        .iter()
        .map(|candidate| {
            (
                Model::from_positions(owner.atom_catalog(), candidate.iter().copied()).unwrap(),
                None,
            )
        })
        .collect();
    let mut expected = expected;
    expected.sort();
    assert_eq!(capture.records(), expected);
}

#[test]
fn cancellation_preserves_unconsumed_verified_answers() {
    let owner = admitted("{a;b;c}. #show x.");
    let cancellation = Cancellation::default();
    let mut execution = NativeBatch::default();
    let capture = run_consuming(&owner, &config(), &cancellation, &mut execution, |_| {
        cancellation.cancel();
    });
    assert!(capture.error.is_none());
    assert_eq!(capture.outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        capture.outcome.interruption(),
        Some(Interruption::Countermodel(Incomplete::Cancelled))
    );
    assert_eq!(capture.answers.len(), 1);
    assert!(capture.answers[0].0.atoms().is_empty());
    assert_eq!(capture.outcome.verified_models(), 3);
    assert_eq!(execution.queue.len(), 2);
    assert_eq!(execution.snapshots.last().unwrap().0.pending, 0);
}

#[test]
fn objective_bounds_preserve_queued_optimal_ties() {
    let owner = admitted("1 {a;b;c} 1. #minimize{1,a:a;1,b:b;2,c:c}. #show.");
    let expected: Vec<Record> = ["a", "b"]
        .into_iter()
        .map(|name| {
            (
                Model::from_positions(
                    owner.atom_catalog(),
                    [owner
                        .atoms()
                        .iter()
                        .position(|atom| atom.predicate().name() == name)
                        .unwrap()],
                )
                .unwrap(),
                Some(vec![(0, 1)]),
            )
        })
        .collect();
    for enabled in [false, true] {
        let mut config = config();
        if !enabled {
            config.max_objective_bound_work = 0;
        }
        let mut execution = NativeBatch::default();
        let capture = run(&owner, &config, &Cancellation::default(), &mut execution);
        assert!(capture.error.is_none());
        assert_eq!(capture.outcome.completion(), Some(Completion::Exhausted));
        let optimum = capture.outcome.incumbent().unwrap();
        assert_eq!(optimum.tied_models, 2);
        assert_eq!(optimum.score.costs(), [(0, 1)]);
        assert_eq!(capture.records(), expected);
        assert_eq!(optimum.scored_models, 3);
        assert!(capture.outcome.optimum_proved());
        if enabled {
            assert!(execution.snapshots.iter().any(|(_, statistics, queued)| {
                statistics.candidate_restrictions > 0 && *queued > 0
            }));
        }
    }
}

#[test]
fn bounded_execution_cannot_prove_optimality() {
    let owner = admitted("1 {a;b;c} 1. #minimize{1,a:a;1,b:b;1,c:c}.");
    for kind in 0..4 {
        let mut config = config();
        let mut execution = NativeBatch::default();
        match kind {
            0 => config.max_objective_work = 0,
            1 => config.max_optimal_models = 1,
            2 => config.max_batch_bytes = 0,
            _ => execution.fault = Fault::Work,
        }
        let capture = run(&owner, &config, &Cancellation::default(), &mut execution);
        assert!(capture.error.is_none());
        assert_eq!(capture.outcome.completion(), Some(Completion::Interrupted));
        assert!(!capture.outcome.optimum_proved());
        assert!(capture.outcome.interruption().is_some());
        if kind == 1 {
            assert_eq!(capture.answers.len(), 1);
            assert_eq!(capture.outcome.incumbent().unwrap().tied_models, 2);
        }
        if kind == 3 {
            assert_eq!(execution.snapshots.last().unwrap().0.pending, 3);
            assert!(capture.answers.is_empty());
        }
    }
}

#[test]
fn stopped_checker_preserves_uncommitted_proposals() {
    let owner = admitted("{a;b;c}.");
    for stop in [Incomplete::Cancelled, Incomplete::Deadline] {
        let mut execution = NativeBatch {
            stop: Some(stop),
            ..Default::default()
        };
        let capture = run(&owner, &config(), &Cancellation::default(), &mut execution);
        assert!(capture.error.is_none());
        assert!(capture.answers.is_empty());
        assert_eq!(capture.outcome.verified_models(), 0);
        assert_eq!(capture.outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(
            capture.outcome.interruption(),
            Some(Interruption::Countermodel(stop))
        );
        assert_eq!(execution.proposed.len(), 1);
        assert_eq!(execution.proposed[0].len(), 3);
        let (batch, _, queued) = execution.snapshots.last().unwrap();
        assert_eq!((batch.pending, batch.propagated, *queued), (3, 0, 0));
    }
}

#[test]
fn malformed_checker_shape_prevents_answer_delivery() {
    let owner = admitted("{a;b;c}.");
    let mut execution = NativeBatch {
        fault: Fault::OmitVerdict,
        ..Default::default()
    };
    let capture = run(&owner, &config(), &Cancellation::default(), &mut execution);
    assert!(matches!(
        capture.error,
        Some(SolveError::FormulaBatchShape {
            expected: 3,
            actual: 2
        })
    ));
    assert!(capture.answers.is_empty());
    assert_eq!(capture.outcome.verified_models(), 0);
    assert_eq!(capture.outcome.completion(), None);
    assert_eq!(execution.snapshots.last().unwrap().0.pending, 3);
}

#[test]
fn a_failed_later_batch_retains_the_verified_prefix() {
    let owner = admitted("{a;b}.");
    let mut execution = NativeBatch {
        fault: Fault::AfterFirst,
        ..Default::default()
    };
    let capture = run(&owner, &config(), &Cancellation::default(), &mut execution);
    assert!(matches!(
        capture.error,
        Some(SolveError::Executor(crate::ExecutorError::ForeignBatch))
    ));
    assert_eq!(capture.answers.len(), 3);
    assert_eq!(capture.outcome.verified_models(), 3);
    assert_eq!(capture.outcome.completion(), None);
    let (batch, _, _) = execution.snapshots.last().unwrap();
    assert_eq!(
        (batch.committed, batch.pending, batch.checker_calls),
        (3, 1, 2)
    );
}

#[test]
fn cancellation_after_decisive_verdicts_commits_nothing() {
    let owner = admitted("{a;b}.");
    let mut execution = NativeBatch {
        answers: Answers::Decided,
        fault: Fault::CancelAfterAnswering,
        ..Default::default()
    };
    let capture = run(&owner, &config(), &Cancellation::default(), &mut execution);
    assert!(capture.error.is_none());
    assert!(capture.answers.is_empty());
    assert_eq!(capture.outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(capture.outcome.verified_models(), 0);
    let (batch, _, _) = execution.snapshots.last().unwrap();
    assert_eq!(
        (batch.committed, batch.propagated, batch.pending),
        (0, 0, 3)
    );
}

#[test]
fn a_verdict_contradicting_the_original_model_is_incomplete() {
    let owner = admitted("{a}.");
    let mut execution = NativeBatch {
        answers: Answers::NotModel,
        ..Default::default()
    };
    let capture = run(&owner, &config(), &Cancellation::default(), &mut execution);
    assert!(capture.error.is_none());
    assert!(capture.answers.is_empty());
    assert_eq!(
        capture.outcome.interruption(),
        Some(Interruption::Countermodel(Incomplete::InvalidWitness))
    );
    assert_eq!(capture.outcome.verified_models(), 0);
    assert_eq!(execution.snapshots.last().unwrap().0.pending, 2);
}

#[test]
fn mixed_decisions_and_residuals_share_one_commit() {
    let owner = admitted("{a;b}.");
    let mut execution = NativeBatch {
        answers: Answers::Mixed,
        ..Default::default()
    };
    let capture = run(&owner, &config(), &Cancellation::default(), &mut execution);
    assert!(capture.error.is_none());
    assert_eq!(capture.answers.len(), 4);
    assert_eq!(capture.outcome.completion(), Some(Completion::Exhausted));
    let (batch, _, _) = execution.snapshots.last().unwrap();
    assert_eq!(
        (
            batch.committed,
            batch.residuals,
            batch.propagated,
            batch.pending
        ),
        (4, 1, 3, 0)
    );
}
