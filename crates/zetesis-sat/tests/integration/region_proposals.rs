//! Parallel region production preserves candidates independently of membership.

use crate::support::choice_theories;
use crate::support::formula_theories as theories;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_ferraris::{Interpretation, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Cancellation, CompletionExecutor, Incomplete, Limits,
    SearchLimits, SearchMethod, StableModels,
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
    StableModels::with_region_producers(theory, nonzero(4), limits, Cancellation::default())
        .unwrap()
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
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
            .then(|| candidate.atoms().collect())
        })
        .collect()
}

#[test]
fn shared_indexes_preserve_non_tight_answer_families() {
    let choices = choice_theories::choices(3);
    let mut nodes = choices.nodes().to_vec();
    let mut roots = choices.roots().to_vec();
    let left = nodes.len();
    nodes.extend([
        Node::Atom(3),
        Node::Atom(4),
        Node::Implies(left, left + 1),
        Node::Implies(left + 1, left),
    ]);
    roots.extend([left + 2, left + 3]);
    let theory = theories::theory(5, nodes, roots);
    assert!(matches!(
        zetesis_ferraris::TightPlan::compile(
            &theory,
            zetesis_ferraris::TightPlanLimits::default(),
            &Cancellation::default()
        ),
        Err(zetesis_ferraris::TightError::PositiveCycle { .. })
    ));
    let expected = expected(&theory);
    assert_eq!(expected.len(), 8);
    let mut scalar =
        StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap();
    let actual: BTreeSet<Vec<_>> = scalar
        .by_ref()
        .map(|answer| answer.unwrap().atoms().collect())
        .collect();
    assert_eq!(actual, expected);
    assert!(scalar.exhausted());
    let reference = scalar.statistics();
    assert!(reference.countermodel_queries > 0);
    assert!(
        reference.countermodels > 0,
        "present unsupported cycles must be rejected"
    );
    for workers in [2, 4] {
        let mut native = StableModels::with_region_workers(
            &theory,
            nonzero(workers),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let answers: Vec<Vec<_>> = native
            .by_ref()
            .map(|answer| answer.unwrap().atoms().collect())
            .collect();
        assert!(native.exhausted());
        assert_eq!(answers.len(), expected.len());
        assert_eq!(answers.into_iter().collect::<BTreeSet<_>>(), expected);
        let actual = native.statistics();
        assert_eq!(actual.countermodel_queries, reference.countermodel_queries);
        assert_eq!(actual.countermodels, reference.countermodels);
        // Each query starts with private fresh knowledge. Scheduling neither
        // rebuilds an original index nor changes its frozen-query work.
        assert_eq!(actual.reduct.regions, reference.reduct.regions);
        for count in [1, 64] {
            for completion_workers in [1, 4] {
                let mut search = StableModels::with_region_producers(
                    &theory,
                    nonzero(workers),
                    Limits::default(),
                    Cancellation::default(),
                )
                .unwrap();
                let mut completion = CompletionExecutor::new(nonzero(completion_workers)).unwrap();
                let mut found = BTreeSet::new();
                while !search.exhausted() {
                    for answer in search
                        .next_batch_with_completion(batch(count), &mut completion, residual)
                        .unwrap()
                    {
                        assert!(found.insert(answer.atoms().collect::<Vec<_>>()));
                    }
                }
                assert_eq!(found, expected);
                let actual = search.statistics();
                assert_eq!(actual.countermodel_queries, reference.countermodel_queries);
                assert_eq!(actual.countermodels, reference.countermodels);
                assert_eq!(actual.reduct.regions, reference.reduct.regions);
                assert_eq!(search.batch_statistics().residuals, actual.candidates);
            }
        }
    }
}

#[test]
fn frontier_peaks_survive_complete_enumeration() {
    let theory = choice_theories::choices(5);
    for workers in [2, 4] {
        let mut search = StableModels::with_region_producers(
            &theory,
            nonzero(workers),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let mut prior = search.statistics().regions.unwrap().frontier.unwrap();
        assert_eq!(prior.regions, 1);
        assert!(prior.retained_bytes > 0);
        let mut actual = BTreeSet::new();
        while !search.exhausted() {
            for model in search.next_batch(batch(3), residual).unwrap() {
                assert!(actual.insert(model.atoms().collect::<Vec<_>>()));
            }
            let observed = search.statistics().regions.unwrap().frontier.unwrap();
            assert!(observed.regions <= observed.capacity);
            assert!(observed.peak_regions >= observed.regions);
            assert!(observed.peak_capacity >= observed.capacity);
            assert!(observed.peak_retained_bytes >= observed.retained_bytes);
            assert!(observed.peak_regions >= prior.peak_regions);
            assert!(observed.peak_capacity >= prior.peak_capacity);
            assert!(observed.peak_retained_bytes >= prior.peak_retained_bytes);
            prior = observed;
        }
        assert_eq!(actual, expected(&theory));
        assert_eq!(prior.regions, 0);
        assert!(prior.capacity > 0);
        assert!(prior.retained_bytes > 0);
        assert!(prior.peak_retained_bytes > prior.retained_bytes);
    }
}

#[test]
fn stopped_frontier_retains_its_ownership_receipt() {
    let theory = choice_theories::choices(5);
    let setup = proposed(&theory, Limits::default())
        .statistics()
        .search
        .work;
    let mut search = proposed(
        &theory,
        Limits {
            search: SearchLimits {
                max_work: setup,
                ..SearchLimits::default()
            },
            ..Limits::default()
        },
    );
    let before = search.statistics().regions.unwrap().frontier.unwrap();
    assert!(matches!(
        search.next_batch(batch(3), residual),
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    let after = search.statistics().regions.unwrap().frontier.unwrap();
    assert_eq!(after.regions, 1);
    assert!(after.retained_bytes > 0);
    assert!(after.peak_retained_bytes >= before.peak_retained_bytes);
    assert_eq!(search.statistics().search.work, setup);
    assert_eq!(search.statistics().stable_models, 0);
    assert!(!search.exhausted());
}

#[test]
fn an_unmeasured_frontier_is_absent() {
    let theory = choice_theories::choices(1);
    let searches = [
        StableModels::with_method(
            &theory,
            SearchMethod::Regions,
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap(),
        StableModels::with_region_producers(
            &theory,
            nonzero(1),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap(),
    ];
    for search in searches {
        assert_eq!(search.statistics().regions.unwrap().frontier, None);
    }
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
