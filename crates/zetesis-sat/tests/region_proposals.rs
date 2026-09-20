//! Parallel region production preserves candidates independently of membership.

#[path = "support/choice_theories.rs"]
mod choice_theories;
#[path = "support/formula_theories.rs"]
mod theories;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_ferraris::{Interpretation, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Control, Incomplete, Limits, SearchLimits, StableModels,
};

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn batch(count: usize) -> BatchLimits {
    BatchLimits {
        max_candidates: nonzero(count),
        max_pending_bytes: 1024 * 1024,
    }
}

fn residual(
    _: &Theory,
    candidates: &[Interpretation],
) -> Result<Vec<BatchVerdict>, std::collections::TryReserveError> {
    let mut verdicts = Vec::new();
    verdicts.try_reserve_exact(candidates.len())?;
    verdicts.resize(candidates.len(), BatchVerdict::Residual);
    Ok(verdicts)
}

fn proposed(theory: &Theory, limits: Limits) -> StableModels {
    StableModels::with_region_producers(theory, nonzero(4), limits, Control::default()).unwrap()
}

fn expected(theory: &Theory) -> BTreeSet<Vec<usize>> {
    (0..1_usize << theory.atom_count())
        .filter_map(|mask| {
            let candidate = Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
            )
            .unwrap();
            zetesis_ferraris::check(
                theory,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Control::default(),
            )
            .unwrap()
            .accepted()
            .then(|| candidate.atoms().collect())
        })
        .collect()
}

#[test]
fn batched_producers_preserve_the_complete_answer_family() {
    let inputs = [
        theories::mixed(),
        choice_theories::choices(5),
        theories::theory(
            2,
            vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
            vec![2],
        ),
        theories::theory(1, vec![Node::Atom(0), Node::Implies(0, 0)], vec![1]),
        theories::theory(0, vec![], vec![]),
        theories::theory(0, vec![Node::False], vec![0]),
    ];
    for theory in inputs {
        let expected = expected(&theory);
        for count in [1, 3, 32] {
            let mut search = proposed(&theory, Limits::default());
            let mut actual = BTreeSet::new();
            while !search.exhausted() {
                for answer in search.next_batch(batch(count), residual).unwrap() {
                    assert!(actual.insert(answer.atoms().collect()));
                }
            }
            assert_eq!(actual, expected);
            assert_eq!(
                search.statistics().candidates,
                search.batch_statistics().committed
            );
            assert_eq!(search.batch_statistics().pending, 0);
        }
    }
}

#[test]
fn producers_leave_nonminimal_models_to_the_checker() {
    let theory = theories::theory(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Implies(0, 1),
            Node::Implies(1, 0),
        ],
        vec![2, 3],
    );
    let mut search = proposed(&theory, Limits::default());
    let mut received = BTreeSet::new();
    let result = search.next_batch(batch(8), |_, candidates| {
        received.extend(
            candidates
                .iter()
                .map(|candidate| candidate.atoms().collect::<Vec<_>>()),
        );
        Err::<Vec<BatchVerdict>, _>("inspection")
    });
    assert!(matches!(result, Err(BatchError::Checker("inspection"))));
    assert_eq!(received, BTreeSet::from([vec![], vec![0, 1]]));
    assert_eq!(search.statistics().countermodel_queries, 0);
    assert_eq!(search.statistics().stable_models, 0);
    assert_eq!(search.batch_statistics().pending, 2);
}

#[test]
fn a_checker_retry_retains_the_exact_proposals() {
    let theory = choice_theories::choices(6);
    let mut search = proposed(&theory, Limits::default());
    let mut first = Vec::new();
    let error = search.next_batch(batch(3), |_, candidates| {
        first = candidates
            .iter()
            .map(|candidate| candidate.atoms().collect::<Vec<_>>())
            .collect();
        Err::<Vec<BatchVerdict>, _>("device failure")
    });
    assert!(matches!(error, Err(BatchError::Checker("device failure"))));
    let before = search.statistics();
    let answers = search
        .next_batch(batch(3), |theory, candidates| {
            let retry: Vec<Vec<usize>> = candidates
                .iter()
                .map(|candidate| candidate.atoms().collect())
                .collect();
            assert_eq!(retry, first);
            residual(theory, candidates)
        })
        .unwrap();
    assert_eq!(answers.len(), 3);
    assert_eq!(search.statistics().candidates, before.candidates);
    assert_eq!(search.statistics().regions, before.regions);
}

#[test]
fn candidate_exhaustion_follows_the_completed_prefix() {
    let theory = choice_theories::choices(5);
    let limits = Limits {
        max_candidates: 2,
        ..Limits::default()
    };
    let mut search = proposed(&theory, limits);
    let answers = search.next_batch(batch(32), residual).unwrap();
    assert_eq!(answers.len(), 2);
    let result = search.next_batch(batch(32), residual);
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::CandidateLimit))
    ));
    assert!(!search.exhausted());
    assert_eq!(search.statistics().stable_models, 2);
    assert_eq!(search.batch_statistics().committed, 2);
}

#[test]
fn an_empty_family_needs_no_candidate_allowance() {
    let theory = theories::theory(0, vec![Node::False], vec![0]);
    let mut search = proposed(
        &theory,
        Limits {
            max_candidates: 0,
            ..Limits::default()
        },
    );
    assert!(search.next_batch(batch(3), residual).unwrap().is_empty());
    assert!(search.exhausted());
}

#[test]
fn production_respects_the_shared_work_ceiling() {
    let theory = choice_theories::choices(6);
    let setup = proposed(&theory, Limits::default())
        .statistics()
        .search
        .work;
    for extra in [0, 1, 8, 64] {
        let ceiling = setup + extra;
        let limits = Limits {
            search: SearchLimits {
                max_work: ceiling,
                ..SearchLimits::default()
            },
            ..Limits::default()
        };
        let mut search = proposed(&theory, limits);
        let result = search.next_batch(batch(3), residual);
        assert!(matches!(
            result,
            Err(BatchError::Search(Incomplete::WorkLimit))
        ));
        assert!(search.statistics().search.work <= ceiling);
        assert!(!search.exhausted());
    }
}

#[test]
fn unvalidated_leaves_never_enter_a_checker_batch() {
    let theory = choice_theories::choices(5);
    let mut search = proposed(
        &theory,
        Limits {
            max_verification_work: 0,
            ..Limits::default()
        },
    );
    let mut calls = 0;
    let result = search.next_batch(batch(3), |theory, candidates| {
        calls += 1;
        residual(theory, candidates)
    });
    assert!(matches!(result, Err(BatchError::Search(_))));
    assert_eq!(calls, 0);
    assert_eq!(search.batch_statistics().pending, 0);
    assert_eq!(search.statistics().candidates, 0);
    assert!(!search.exhausted());
}

#[test]
fn a_restriction_reaches_the_retained_frontier() {
    let theory = choice_theories::choices(6);
    let mut search = proposed(&theory, Limits::default());
    let first = search.next_batch(batch(3), residual).unwrap();
    let restriction = choice_theories::theory_over(&theory, vec![Node::Atom(5)], vec![0]);
    search.restrict_candidates(&restriction).unwrap();
    let mut seen: BTreeSet<Vec<usize>> = first
        .iter()
        .map(|answer| answer.atoms().collect())
        .collect();
    while !search.exhausted() {
        for answer in search.next_batch(batch(3), residual).unwrap() {
            assert!(answer.contains(5));
            assert!(seen.insert(answer.atoms().collect()));
        }
    }
    let expected: BTreeSet<Vec<usize>> = expected(&theory)
        .into_iter()
        .filter(|answer| answer.contains(&5))
        .chain(first.iter().map(|answer| answer.atoms().collect()))
        .collect();
    assert_eq!(seen, expected);
}

#[test]
fn scalar_consumption_retains_reduct_membership() {
    let theory = theories::mixed();
    let mut search = proposed(&theory, Limits::default());
    let found: BTreeSet<Vec<usize>> = search
        .by_ref()
        .map(|answer| answer.unwrap().atoms().collect())
        .collect();
    assert!(search.exhausted());
    assert_eq!(found, expected(&theory));
}
