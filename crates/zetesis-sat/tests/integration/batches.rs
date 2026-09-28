//! Candidate batches are checked before publication against an independent reduct oracle.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::num::NonZeroUsize;

use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Cancellation, Incomplete, Limits, StableModels,
};

/// Enumerate by the clause forms, the subject of the tests below.
fn by_clauses(
    theory: &zetesis_ferraris::Theory,
    limits: zetesis_sat::Limits,
    cancellation: zetesis_sat::Cancellation,
) -> Result<zetesis_sat::StableModels, zetesis_sat::Incomplete> {
    zetesis_sat::StableModels::with_method(
        theory,
        zetesis_sat::SearchMethod::Clauses,
        limits,
        cancellation,
    )
}

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}
fn choices() -> Theory {
    theory(
        3,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::False,
            Node::Implies(0, 3),
            Node::Or(0, 4),
            Node::Implies(1, 3),
            Node::Or(1, 6),
            Node::Implies(2, 3),
            Node::Or(2, 8),
        ],
        vec![5, 7, 9],
    )
}
fn limits(count: usize) -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(count).unwrap(),
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
fn expected(theory: &Theory) -> BTreeSet<Vec<usize>> {
    (0..1_usize << theory.atom_count())
        .filter_map(|mask| {
            let model = Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
            )
            .unwrap();
            zetesis_ferraris::check(
                theory,
                &model,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
            .then(|| model.atoms().collect())
        })
        .collect()
}
fn collect(theory: &Theory, count: usize, propagate: bool) {
    let mut search = StableModels::new(theory, Limits::default(), Cancellation::default()).unwrap();
    let mut actual = BTreeSet::new();
    while !search.exhausted() {
        let batch = search
            .next_batch(limits(count), |original, candidates| {
                assert!(original.same_instance(theory));
                Ok::<_, Infallible>(
                    candidates
                        .iter()
                        .map(|candidate| {
                            assert!(theory.same_instance(candidate.theory()));
                            let check = zetesis_ferraris::check(
                                theory,
                                candidate,
                                zetesis_ferraris::Limits::default(),
                                &Cancellation::default(),
                            )
                            .unwrap();
                            if propagate && check.accepted() {
                                BatchVerdict::NoProperSubset
                            } else {
                                BatchVerdict::Residual
                            }
                        })
                        .collect(),
                )
            })
            .unwrap();
        for model in batch {
            assert!(actual.insert(model.atoms().collect()));
        }
        assert_eq!(search.batch_statistics().pending, 0);
    }
    assert_eq!(actual, expected(theory));
    assert_eq!(
        search.statistics().candidates,
        search.batch_statistics().committed
    );
    assert_eq!(
        search.statistics().stable_models,
        u64::try_from(actual.len()).unwrap()
    );
    assert!(
        search
            .next_batch(limits(count), residual)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn generated_original_and_frozen_formulas_match_complete_reference_in_irregular_batches() {
    for left in 0..3 {
        for right in 0..3 {
            for connective in [
                Node::And(left, right),
                Node::Or(left, right),
                Node::Implies(left, right),
            ] {
                for roots in [vec![], vec![3], vec![4], vec![3, 4]] {
                    let t = theory(
                        2,
                        vec![
                            Node::Atom(0),
                            Node::Atom(1),
                            Node::False,
                            connective,
                            Node::Implies(3, 2),
                        ],
                        roots,
                    );
                    collect(&t, 3, false);
                    collect(&t, 3, true);
                }
            }
        }
    }
    for count in [1, 3, 8, 32] {
        collect(&choices(), count, false);
        collect(&choices(), count, true);
    }
    collect(&theory(0, vec![], vec![]), 3, false);
    collect(&theory(0, vec![Node::False], vec![0]), 3, true);
}

#[test]
fn failed_checker_retains_exact_order_and_retries_without_reproposal() {
    let t = choices();
    let mut search = StableModels::new(&t, Limits::default(), Cancellation::default()).unwrap();
    let mut first = Vec::new();
    let result = search.next_batch(limits(3), |_, candidates| {
        first = candidates
            .iter()
            .map(|c| c.atoms().collect::<Vec<_>>())
            .collect();
        Err::<Vec<BatchVerdict>, _>("device lost")
    });
    assert!(matches!(result, Err(BatchError::Checker("device lost"))));
    assert_eq!(search.batch_statistics().pending, 3);
    let before = search.statistics().candidates;
    let result = search
        .next_batch(limits(3), |original, candidates| {
            assert_eq!(
                first,
                candidates
                    .iter()
                    .map(|c| c.atoms().collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            );
            residual(original, candidates)
        })
        .unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(search.statistics().candidates, before);
    assert_eq!(search.batch_statistics().checker_calls, 2);
    assert_eq!(search.batch_statistics().committed, 3);
    assert!(!search.exhausted());
}

#[test]
fn restrictions_do_not_discard_pending_old_region_candidates_or_change_the_reduct() {
    let t = choices();
    let mut search = StableModels::new(&t, Limits::default(), Cancellation::default()).unwrap();
    let _ = search.next_batch(limits(3), |_, _| Err::<Vec<BatchVerdict>, _>("retry"));
    let restriction = theory(3, vec![Node::False], vec![0]);
    search.restrict_candidates(&restriction).unwrap();
    assert_eq!(search.batch_statistics().pending, 3);
    let old_region = search.next_batch(limits(3), residual).unwrap();
    assert_eq!(old_region.len(), 3);
    assert!(search.theory().same_instance(&t));
    assert!(search.next_batch(limits(3), residual).unwrap().is_empty());
    assert!(search.exhausted());
}

#[test]
fn short_and_excess_checker_results_are_retryable_but_not_model_is_an_invariant_failure() {
    for length in [0, 4] {
        let t = choices();
        let mut search = StableModels::new(&t, Limits::default(), Cancellation::default()).unwrap();
        let result = search.next_batch(limits(3), |_, _| {
            Ok::<_, Infallible>(vec![BatchVerdict::Residual; length])
        });
        // A caller diagnosing a broken checker must be told which count was
        // returned and which candidate count was expected, without reversing them.
        let diagnostic = result.as_ref().unwrap_err().to_string();
        assert!(diagnostic.contains(&format!("{length} results")));
        assert!(diagnostic.contains("3 candidates"));
        assert!(
            matches!(result, Err(BatchError::Shape { expected: 3, actual }) if actual == length)
        );
        assert!(!search.exhausted());
        assert_eq!(search.batch_statistics().pending, 3);
        assert_eq!(search.next_batch(limits(3), residual).unwrap().len(), 3);
    }
    let mut search =
        StableModels::new(&choices(), Limits::default(), Cancellation::default()).unwrap();
    let result = search.next_batch(limits(3), |_, c| {
        Ok::<_, Infallible>(vec![BatchVerdict::NotModel; c.len()])
    });
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::InvalidWitness))
    ));
    assert_eq!(search.batch_statistics().pending, 3);
    assert_eq!(search.batch_statistics().committed, 0);
    assert!(!search.exhausted());
    assert!(matches!(
        search.next_batch(limits(3), residual),
        Err(BatchError::Search(Incomplete::ClosedEnumerator))
    ));
}

#[test]
fn candidate_limit_preserves_the_checked_batch_prefix() {
    let t = choices();
    let mut search = StableModels::new(
        &t,
        Limits {
            max_candidates: 2,
            ..Limits::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert_eq!(search.next_batch(limits(3), residual).unwrap().len(), 2);
    assert!(!search.exhausted());
    assert!(matches!(
        search.next_batch(limits(3), residual),
        Err(BatchError::Search(Incomplete::CandidateLimit))
    ));
}

#[test]
fn history_limit_preserves_the_checked_batch_prefix() {
    // A zero-atom stable theory has no encoding clauses; its exact empty block
    // cannot be stored under max_entries=0, after the candidate is already owned.
    let empty = theory(0, vec![], vec![]);
    let bounded = Limits {
        projections: zetesis_sat::ProjectionLimits {
            max_entries: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut search = by_clauses(&empty, bounded, Cancellation::default()).unwrap();
    let batch = search
        .next_batch(limits(2), |_, c| {
            Ok::<_, Infallible>(vec![BatchVerdict::NoProperSubset; c.len()])
        })
        .unwrap();
    assert_eq!(batch.len(), 1);
    assert!(batch[0].theory().same_instance(&empty));
    assert_eq!(batch[0].atoms().count(), 0);
    assert_eq!(search.statistics().stable_models, 1);
    assert_eq!(search.statistics().projections.entries, 0);
    assert_eq!(search.batch_statistics().committed, 1);
    assert!(matches!(
        search.next_batch(limits(2), residual),
        Err(BatchError::Search(Incomplete::ProjectionLimit {
            resource: zetesis_sat::ProjectionResource::Entries,
            required: 1,
            limit: 0,
        }))
    ));
    assert!(!search.exhausted());
}

#[test]
fn cancellation_after_proposal_preserves_pending_and_prevents_partial_commit() {
    let cancellation = Cancellation::default();
    let mut search =
        StableModels::new(&choices(), Limits::default(), cancellation.clone()).unwrap();
    let result = search.next_batch(limits(3), |theory, c| {
        cancellation.cancel();
        residual(theory, c)
    });
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::Cancelled))
    ));
    assert_eq!(search.batch_statistics().pending, 3);
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.statistics().stable_models, 0);
    assert!(!search.exhausted());
}

#[test]
fn pending_capacity_and_scalar_mode_refusals_never_claim_completion() {
    let mut search =
        StableModels::new(&choices(), Limits::default(), Cancellation::default()).unwrap();
    let result = search.next_batch(
        BatchLimits {
            max_pending_bytes: 0,
            ..limits(3)
        },
        residual,
    );
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::PendingBytes))
    ));
    assert_eq!(search.statistics().candidates, 0);
    assert!(!search.exhausted());
    let mut search =
        StableModels::new(&choices(), Limits::default(), Cancellation::default()).unwrap();
    let _ = search.next_batch(limits(3), |_, _| Err::<Vec<BatchVerdict>, _>("retry"));
    assert!(matches!(search.next(), Some(Err(Incomplete::PendingBatch))));
    assert_eq!(search.batch_statistics().pending, 3);
    assert!(search.next().is_none());
    assert!(!search.exhausted());
}

#[test]
fn retry_limits_and_typed_causes_remain_enforced_without_dropping_pending_rows() {
    use std::error::Error as _;
    let mut search =
        StableModels::new(&choices(), Limits::default(), Cancellation::default()).unwrap();
    let failure = search
        .next_batch(limits(3), |_, _| {
            Err::<Vec<BatchVerdict>, _>(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        })
        .unwrap_err();
    assert_eq!(
        failure
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .kind(),
        std::io::ErrorKind::BrokenPipe
    );
    for (smaller, expected) in [
        (limits(1), Incomplete::BatchCandidateLimit),
        (
            BatchLimits {
                max_pending_bytes: 0,
                ..limits(3)
            },
            Incomplete::PendingBytes,
        ),
    ] {
        let failure = search
            .next_batch(
                smaller,
                |_, _| -> Result<Vec<BatchVerdict>, std::io::Error> {
                    panic!("undersized retry must not call the checker")
                },
            )
            .unwrap_err();
        assert!(matches!(&failure, BatchError::Limits(error) if *error == expected));
        assert_eq!(
            failure.source().unwrap().downcast_ref::<Incomplete>(),
            Some(&expected)
        );
        assert_eq!(search.batch_statistics().pending, 3);
        assert_eq!(search.batch_statistics().checker_calls, 1);
        assert!(!search.exhausted());
    }
    assert_eq!(search.next_batch(limits(3), residual).unwrap().len(), 3);
    let error = BatchError::<std::io::Error>::Search(Incomplete::WorkLimit);
    assert_eq!(
        error.source().unwrap().downcast_ref::<Incomplete>(),
        Some(&Incomplete::WorkLimit)
    );
    assert!(
        BatchError::<std::io::Error>::Shape {
            expected: 1,
            actual: 0
        }
        .source()
        .is_none()
    );
}
