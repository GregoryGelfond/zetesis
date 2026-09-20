//! The production queue/common loop tested with actual exact native membership.
//! No device is constructed, and these tests make no GPU execution/parity claim.

use std::num::NonZeroUsize;

use zetesis_cpu::Control;
use zetesis_ferraris::Interpretation;
use zetesis_sat::{BatchStatistics, BatchVerdict, Incomplete, StableModels, Statistics};

use super::test_harness::{Record, admitted, run, run_consuming};
use crate::formula_execution::{Failure, FormulaExecutionStatistics, MembershipExecution};
use crate::formula_queue::BatchQueue;
use crate::{
    Backend, Completion, Interruption, Oracle, PreparedInput, Session, SolveConfig, SolveError,
};

#[derive(Default)]
struct NativeBatch {
    queue: BatchQueue,
    snapshots: Vec<(BatchStatistics, Statistics, usize)>,
    proposed: Vec<Vec<Vec<usize>>>,
    fail_work: bool,
    omit_verdict: bool,
    stop: Option<Incomplete>,
}

impl MembershipExecution for NativeBatch {
    fn next(
        &mut self,
        models: &mut StableModels,
        options: &crate::SolveConfig,
        control: &Control,
        _: &crate::phase_timing::Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        let result = self.queue.next(models, options, control, |batch| {
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
            if self.fail_work {
                let limits = zetesis_sat::Limits {
                    search: zetesis_sat::SearchLimits {
                        max_work: 0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                if let zetesis_sat::Check::Inconclusive(error) =
                    zetesis_sat::check(theory, &candidates[0], limits, control)
                {
                    return Err(Failure::Search(error));
                }
                panic!("the selected nonempty source needs native encoding work");
            }
            // Declining partial propagation sends every original candidate to
            // the real exact native reduct checker inside StableModels.
            let mut verdicts = Vec::new();
            verdicts
                .try_reserve_exact(candidates.len())
                .map_err(|_| Failure::Search(Incomplete::Allocation))?;
            verdicts.resize(candidates.len(), BatchVerdict::Residual);
            if self.omit_verdict {
                verdicts.pop();
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
        let mut scalar =
            Session::new(PreparedInput::formula(&owner), config, Control::default()).unwrap();
        let mut expected: Vec<Record> = scalar
            .by_ref()
            .map(|answer| {
                let answer = answer.unwrap();
                (
                    answer.interpretation().atoms().iter().cloned().collect(),
                    answer.score().map(|score| score.costs().to_vec()),
                )
            })
            .collect();
        expected.sort();
        let mut execution = NativeBatch::default();
        let actual = run(&owner, &config, &Control::default(), &mut execution);
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
        &Control::default(),
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
        &Control::default(),
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
            let mut atoms: Vec<_> = candidate
                .iter()
                .map(|id| owner.atoms()[*id].clone())
                .collect();
            atoms.sort();
            (atoms, None)
        })
        .collect();
    let mut expected = expected;
    expected.sort();
    assert_eq!(capture.records(), expected);
}

#[test]
fn cancellation_preserves_unconsumed_verified_answers() {
    let owner = admitted("{a;b;c}. #show x.");
    let control = Control::default();
    let mut execution = NativeBatch::default();
    let capture = run_consuming(&owner, &config(), &control, &mut execution, |_| {
        control.cancel();
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
                vec![
                    owner
                        .atoms()
                        .iter()
                        .find(|atom| atom.predicate().name() == name)
                        .unwrap()
                        .clone(),
                ],
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
        let capture = run(&owner, &config, &Control::default(), &mut execution);
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
            _ => execution.fail_work = true,
        }
        let capture = run(&owner, &config, &Control::default(), &mut execution);
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
        let capture = run(&owner, &config(), &Control::default(), &mut execution);
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
        omit_verdict: true,
        ..Default::default()
    };
    let capture = run(&owner, &config(), &Control::default(), &mut execution);
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
