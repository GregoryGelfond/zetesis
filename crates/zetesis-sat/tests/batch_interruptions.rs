//! Inclusive work and terminal contracts of the original-candidate batch API.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::num::NonZeroUsize;

use zetesis_cpu::Stop;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Control, Incomplete, Limits, SearchLimits, StableModels,
};

fn choices() -> Theory {
    // Three independent a OR NOT a formulas: all eight interpretations are
    // stable. External certificates below are checked by exhaustive Ferraris.
    Theory::new(
        3,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::Implies(1, 0),
            Node::Or(1, 4),
            Node::Implies(2, 0),
            Node::Or(2, 6),
            Node::Implies(3, 0),
            Node::Or(3, 8),
        ],
        vec![5, 7, 9],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn batch(count: usize) -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(count).unwrap(),
        max_pending_bytes: 1024 * 1024,
    }
}

fn never_called(_: &Theory, _: &[Interpretation]) -> Result<Vec<BatchVerdict>, Infallible> {
    panic!("a proposal/control refusal must not invoke the checker")
}

fn certified(theory: &Theory, candidates: &[Interpretation]) -> Result<Vec<BatchVerdict>, Stop> {
    for candidate in candidates {
        assert!(
            zetesis_ferraris::check(
                theory,
                candidate,
                zetesis_ferraris::Limits::default(),
                &Control::default(),
            )?
            .accepted()
        );
    }
    Ok(vec![BatchVerdict::NoProperSubset; candidates.len()])
}

fn closed(search: &mut StableModels) {
    assert!(!search.exhausted());
    assert!(matches!(
        search.next_batch(batch(3), never_called),
        Err(BatchError::Search(Incomplete::ClosedEnumerator))
    ));
    assert!(search.next().is_none());
    assert!(!search.exhausted());
}

#[test]
fn cancellation_before_proposal_and_before_retry_preserves_owned_candidates() {
    for retain_pending in [false, true] {
        let theory = choices();
        let control = Control::default();
        let mut search = StableModels::new(&theory, Limits::default(), control.clone()).unwrap();
        if retain_pending {
            let failure = search.next_batch(batch(3), |_, _| {
                Err::<Vec<BatchVerdict>, _>("checker temporarily unavailable")
            });
            assert!(matches!(failure, Err(BatchError::Checker(_))));
        }
        let before = search.statistics();
        let pending = search.batch_statistics();
        control.cancel();
        assert!(matches!(
            search.next_batch(batch(3), never_called),
            Err(BatchError::Search(Incomplete::Cancelled))
        ));
        assert_eq!(search.statistics(), before);
        assert_eq!(search.batch_statistics(), pending);
        assert_eq!(pending.pending, if retain_pending { 3 } else { 0 });
        assert_eq!(pending.committed, 0);
        closed(&mut search);
    }
}

#[test]
fn candidate_decision_and_original_verification_limits_cannot_publish_proposals() {
    let theory = choices();
    for (limits, expected) in [
        (
            Limits {
                max_candidates: 0,
                ..Limits::default()
            },
            Incomplete::CandidateLimit,
        ),
        (
            Limits {
                search: SearchLimits {
                    max_decisions: 0,
                    ..SearchLimits::default()
                },
                ..Limits::default()
            },
            Incomplete::DecisionLimit,
        ),
        (
            Limits {
                max_verification_work: 0,
                ..Limits::default()
            },
            Incomplete::Verification(Stop::WorkLimit),
        ),
    ] {
        let mut search = StableModels::new(&theory, limits, Control::default()).unwrap();
        assert!(matches!(
            search.next_batch(batch(3), never_called),
            Err(BatchError::Search(actual)) if actual == expected
        ));
        assert_eq!(search.statistics().candidates, 0);
        assert_eq!(search.statistics().candidate_queries, 1);
        assert_eq!(search.statistics().stable_models, 0);
        assert_eq!(search.batch_statistics().pending, 0);
        assert_eq!(search.batch_statistics().checker_calls, 0);
        closed(&mut search);
    }
}

fn proposal_work(theory: &Theory, count: usize) -> (u64, u64) {
    let mut search = StableModels::new(theory, Limits::default(), Control::default()).unwrap();
    let encoded = search.statistics().search.work;
    let result = search.next_batch(batch(count), |_, candidates| {
        assert_eq!(candidates.len(), count);
        Err::<Vec<BatchVerdict>, _>("measure the public proposal boundary")
    });
    assert!(matches!(result, Err(BatchError::Checker(_))));
    (encoded, search.statistics().search.work)
}

fn with_work(theory: &Theory, max_work: u64) -> StableModels {
    StableModels::new(
        theory,
        Limits {
            search: SearchLimits {
                max_work,
                ..SearchLimits::default()
            },
            ..Limits::default()
        },
        Control::default(),
    )
    .unwrap()
}

#[test]
fn residual_work_exhaustion_cannot_commit_an_earlier_certified_prefix() {
    let theory = choices();
    let (_, proposed) = proposal_work(&theory, 3);
    let mut search = with_work(&theory, proposed);
    let result = search.next_batch(batch(3), |original, candidates| {
        let mut verdicts = certified(original, candidates)?;
        // The first independently certified answer is already valid. The next
        // query still needs native reduct work, with no remaining allowance.
        verdicts[1] = BatchVerdict::Residual;
        Ok::<_, Stop>(verdicts)
    });
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    assert_eq!(search.statistics().search.work, proposed);
    assert_eq!(search.statistics().candidates, 3);
    assert_eq!(search.statistics().stable_models, 0);
    assert_eq!(search.batch_statistics().pending, 3);
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.batch_statistics().propagated, 0);
    assert_eq!(search.batch_statistics().residuals, 0);
    closed(&mut search);
    // A fresh exact run of the same immutable theory still has all eight models.
    let mut complete = StableModels::new(&theory, Limits::default(), Control::default()).unwrap();
    let models: BTreeSet<Vec<_>> = complete
        .by_ref()
        .map(|model| model.unwrap().atoms().collect())
        .collect();
    let expected: BTreeSet<Vec<_>> = (0..8)
        .map(|bits| (0..3).filter(|atom| bits & (1 << atom) != 0).collect())
        .collect();
    assert_eq!(models, expected);
    assert!(complete.exhausted());
}

#[test]
fn every_first_proposal_work_cutoff_returns_only_proved_models_and_a_terminal_stop() {
    let theory = choices();
    let (encoded, proposed) = proposal_work(&theory, 1);
    assert!(proposed > encoded);
    assert!(
        proposed - encoded < 4096,
        "bounded tiny-fixture work interval"
    );
    for allowance in encoded..=proposed {
        let mut search = with_work(&theory, allowance);
        match search.next_batch(batch(1), certified) {
            Ok(models) => {
                assert_eq!(models.len(), 1);
                assert_eq!(search.statistics().stable_models, 1);
                assert_eq!(search.batch_statistics().committed, 1);
                // A block may have hit the bound after owning this model, or
                // the next proposal hits it. Both preserve the same typed stop.
                assert!(matches!(
                    search.next_batch(batch(1), never_called),
                    Err(BatchError::Search(Incomplete::WorkLimit))
                ));
            }
            Err(BatchError::Search(Incomplete::WorkLimit)) => {
                assert_eq!(search.statistics().stable_models, 0);
                assert_eq!(search.batch_statistics().committed, 0);
            }
            other => panic!("unexpected work-cutoff result at {allowance}: {other:?}"),
        }
        assert_eq!(search.statistics().search.work, allowance);
        assert_eq!(search.batch_statistics().pending, 0);
        closed(&mut search);
    }
}
