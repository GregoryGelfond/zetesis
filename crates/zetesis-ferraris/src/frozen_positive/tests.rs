//! Compare completed closure certificates with independent finite subset checking.

use super::*;
use crate::test_support::{theory, world};
use crate::{EvaluationWorkspace, Limits, Node, Verdict};

fn attempt(
    workspace: &mut FrozenPositiveWorkspace,
    candidate: &Interpretation,
    limits: EvaluationLimits,
) -> FrozenPositiveAttempt {
    let control = Cancellation::default();
    let mut evaluation = EvaluationWorkspace::default();
    let truth = evaluation
        .evaluate(candidate, EvaluationLimits::default(), &control)
        .result
        .unwrap();
    workspace.check(&truth, limits, &control)
}

fn complete(owner: &Theory, mask: usize) -> FrozenPositiveOutcome {
    attempt(
        &mut FrozenPositiveWorkspace::default(),
        &world(owner, mask),
        EvaluationLimits::default(),
    )
    .result
    .unwrap()
}

#[test]
fn unseeded_positive_cycle_returns_its_empty_reduct_model() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![2, 3],
    );
    let FrozenPositiveOutcome::NonMinimal(witness) = complete(&owner, 3) else {
        panic!("cycle needs a strict-subset witness")
    };
    assert!(witness.theory().same_instance(&owner));
    assert_eq!(witness.atoms().count(), 0);
    assert!(
        crate::models_reduct(
            &owner,
            &world(&owner, 3),
            &witness,
            Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
    );
}

#[test]
fn repeated_conjunction_occurrences_all_receive_their_signal() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::and_pair([0, 0]),
            Node::implies(2, 1),
        ],
        vec![0, 3],
    );
    assert!(matches!(complete(&owner, 3), FrozenPositiveOutcome::Stable));
}

#[test]
fn repeated_live_head_occurrences_name_one_semantic_atom() {
    let owner = theory(
        1,
        vec![Node::atom(0), Node::atom(0), Node::or_pair([0, 1])],
        vec![2],
    );
    assert!(matches!(complete(&owner, 1), FrozenPositiveOutcome::Stable));
}

#[test]
fn two_distinct_live_heads_decline_the_optional_certificate() {
    let owner = theory(
        2,
        vec![Node::atom(0), Node::atom(1), Node::or_pair([0, 1])],
        vec![2],
    );
    assert!(matches!(
        complete(&owner, 3),
        FrozenPositiveOutcome::Declined
    ));
    assert!(matches!(complete(&owner, 1), FrozenPositiveOutcome::Stable));
}

#[test]
fn true_negated_head_supplies_no_positive_support() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(1, 2),
            Node::or_pair([0, 3]),
        ],
        vec![4],
    );
    let FrozenPositiveOutcome::NonMinimal(witness) = complete(&owner, 1) else {
        panic!("true negation makes the root tautological")
    };
    assert_eq!(witness.atoms().count(), 0);
}

#[test]
fn false_unsupported_child_is_falsum_in_a_live_body() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::or_pair([2, 0]),
            Node::implies(3, 0),
        ],
        vec![4],
    );
    assert!(matches!(
        complete(&owner, 1),
        FrozenPositiveOutcome::NonMinimal(_)
    ));
}

#[test]
fn active_unnegated_implication_body_declines() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::implies(2, 1),
        ],
        vec![3],
    );
    assert!(matches!(
        complete(&owner, 3),
        FrozenPositiveOutcome::Declined
    ));
}

#[test]
fn satisfied_support_guards_supply_no_founded_atoms() {
    let owner = theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 0),
            Node::implies(2, 1),
            Node::implies(3, 1),
        ],
        vec![2, 4],
    );
    assert!(matches!(
        complete(&owner, 1),
        FrozenPositiveOutcome::NonMinimal(_)
    ));
}

#[test]
fn every_tiny_completed_certificate_agrees_with_subset_checking() {
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::falsum(),
        Node::implies(0, 2),
        Node::implies(1, 2),
        Node::implies(3, 2),
        Node::or_pair([0, 1]),
        Node::and_pair([0, 1]),
        Node::implies(3, 1),
        Node::implies(1, 0),
        Node::implies(0, 1),
        Node::implies(0, 2),
        Node::implies(10, 2),
        Node::implies(12, 2),
        Node::or_pair([0, 3]),
        Node::and_pair([10, 0]),
        Node::implies(15, 1),
        Node::and_pair([0, 0]),
        Node::or_pair([0, 0]),
        Node::implies(4, 0),
    ];
    let roots = [0, 1, 2, 6, 8, 9, 10, 11, 13, 14, 16, 18, 19];
    let mut decided = 0;
    let mut declined = 0;
    let mut workspace = FrozenPositiveWorkspace::default();
    for (position, &left) in roots.iter().enumerate() {
        for &right in &roots[position..] {
            let owner = theory(2, nodes.clone(), vec![left, right]);
            for mask in 0..4 {
                let candidate = world(&owner, mask);
                let reference = crate::check(
                    &owner,
                    &candidate,
                    Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                let result = attempt(&mut workspace, &candidate, EvaluationLimits::default());
                assert!(u128::from(result.work) <= FrozenPositiveWorkspace::work_bound(&owner));
                match result.result.unwrap() {
                    FrozenPositiveOutcome::Declined => declined += 1,
                    FrozenPositiveOutcome::NotModel => {
                        decided += 1;
                        assert!(matches!(reference.verdict(), Verdict::NotModel { .. }));
                    }
                    FrozenPositiveOutcome::Stable => {
                        decided += 1;
                        assert!(reference.accepted(), "roots={left},{right}; M={mask}");
                    }
                    FrozenPositiveOutcome::NonMinimal(witness) => {
                        decided += 1;
                        assert!(matches!(reference.verdict(), Verdict::NonMinimal { .. }));
                        assert!(witness.theory().same_instance(&owner));
                        assert!(witness.atoms().count() < candidate.atoms().count());
                        assert!(witness.atoms().all(|atom| candidate.contains(atom)));
                        assert!(
                            crate::models_reduct(
                                &owner,
                                &candidate,
                                &witness,
                                Limits::default(),
                                &Cancellation::default()
                            )
                            .unwrap()
                        );
                    }
                }
            }
        }
    }
    assert!(decided > 100);
    assert!(declined > 0);
}

#[test]
fn every_interrupted_prefix_can_reuse_its_workspace() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![0, 2, 3],
    );
    let candidate = world(&owner, 3);
    let completed = attempt(
        &mut FrozenPositiveWorkspace::default(),
        &candidate,
        EvaluationLimits::default(),
    );
    assert!(matches!(
        completed.result,
        Ok(FrozenPositiveOutcome::Stable)
    ));
    assert!(completed.preparation_work > 0);
    for max_work in 0..completed.work {
        let mut workspace = FrozenPositiveWorkspace::default();
        let stopped = attempt(
            &mut workspace,
            &candidate,
            EvaluationLimits {
                max_work,
                ..EvaluationLimits::default()
            },
        );
        assert!(matches!(
            stopped.result,
            Err(EvaluationError::Stopped(Stop::WorkLimit))
        ));
        assert_eq!(stopped.work, max_work);
        assert!(matches!(
            attempt(&mut workspace, &candidate, EvaluationLimits::default()).result,
            Ok(FrozenPositiveOutcome::Stable)
        ));
    }
}

#[test]
fn returned_witness_is_admitted_with_the_live_workspace() {
    let owner = theory(1, vec![Node::atom(0), Node::implies(0, 0)], vec![1]);
    let candidate = world(&owner, 1);
    let mut workspace = FrozenPositiveWorkspace::default();
    let baseline = attempt(&mut workspace, &candidate, EvaluationLimits::default());
    assert!(matches!(
        baseline.result,
        Ok(FrozenPositiveOutcome::NonMinimal(_))
    ));
    assert!(baseline.peak_bytes > baseline.retained_bytes);
    let max_bytes = usize::try_from(baseline.retained_bytes).unwrap();
    let limited = attempt(
        &mut workspace,
        &candidate,
        EvaluationLimits {
            max_bytes,
            ..EvaluationLimits::default()
        },
    );
    assert!(
        matches!(limited.result, Err(EvaluationError::Storage { required, .. }) if required > max_bytes as u128)
    );
}

#[test]
fn repeated_queries_reuse_only_the_original_incidence_structure() {
    let owner = theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::or_pair([0, 2]),
        ],
        vec![3],
    );
    let mut workspace = FrozenPositiveWorkspace::default();
    let first = attempt(
        &mut workspace,
        &world(&owner, 1),
        EvaluationLimits::default(),
    );
    assert!(first.preparation_work > 0);
    for mask in [0, 1, 0] {
        let next = attempt(
            &mut workspace,
            &world(&owner, mask),
            EvaluationLimits::default(),
        );
        assert_eq!(next.preparation_work, 0);
        assert!(matches!(next.result, Ok(FrozenPositiveOutcome::Stable)));
    }
}

#[test]
fn cancellation_precedes_nonmodel_and_storage_decisions() {
    let owner = theory(1, vec![Node::atom(0)], vec![0]);
    let candidate = world(&owner, 0);
    let control = Cancellation::default();
    let mut evaluation = EvaluationWorkspace::default();
    let truth = evaluation
        .evaluate(&candidate, EvaluationLimits::default(), &control)
        .result
        .unwrap();
    control.cancel();
    let result = FrozenPositiveWorkspace::default().check(
        &truth,
        EvaluationLimits {
            max_bytes: 0,
            ..EvaluationLimits::default()
        },
        &control,
    );
    assert!(matches!(
        result.result,
        Err(EvaluationError::Stopped(Stop::Cancelled))
    ));
    assert_eq!(result.work, 0);
}

#[test]
fn unsupported_roots_decline_before_allocating_incidence_storage() {
    let mut nodes = vec![Node::atom(0), Node::implies(0, 0)];
    for _ in 0..128 {
        let last = nodes.len() - 1;
        nodes.push(Node::and_pair([last, last]));
    }
    let body = nodes.len() - 1;
    nodes.push(Node::implies(body, 0));
    let owner = theory(1, nodes, vec![body + 1]);
    let mut workspace = FrozenPositiveWorkspace::default();
    // Classification fits; the linear incidence graph does not. The active
    // implication inside the body is outside the optional positive fragment.
    let max_bytes = size_of::<FrozenPositiveWorkspace>()
        + owner.nodes().len() * size_of::<Classification>()
        + owner.roots().len() * size_of::<Option<usize>>()
        + 64;
    let result = attempt(
        &mut workspace,
        &world(&owner, 1),
        EvaluationLimits {
            max_bytes,
            ..EvaluationLimits::default()
        },
    );
    assert!(matches!(result.result, Ok(FrozenPositiveOutcome::Declined)));
    assert!(result.work > 0);
    assert_eq!(result.preparation_work, 0);
    assert!(workspace.owner.is_none());
    assert_eq!(workspace.offsets.capacity(), 0);
    assert_eq!(workspace.edges.capacity(), 0);
    assert_eq!(workspace.states.capacity(), 0);
    assert_eq!(workspace.queue.capacity(), 0);
    assert_eq!(workspace.words.capacity(), 0);
    assert!(result.retained_bytes <= max_bytes as u128);
}

#[test]
fn a_declined_other_theory_preserves_reusable_incidence_identity() {
    let supported = theory(1, vec![Node::atom(0)], vec![0]);
    let unsupported = theory(
        2,
        vec![Node::atom(0), Node::atom(1), Node::or_pair([0, 1])],
        vec![2],
    );
    let mut workspace = FrozenPositiveWorkspace::default();
    let first = attempt(
        &mut workspace,
        &world(&supported, 1),
        EvaluationLimits::default(),
    );
    assert!(matches!(first.result, Ok(FrozenPositiveOutcome::Stable)));
    assert!(first.preparation_work > 0);
    let declined = attempt(
        &mut workspace,
        &world(&unsupported, 3),
        EvaluationLimits::default(),
    );
    assert!(matches!(
        declined.result,
        Ok(FrozenPositiveOutcome::Declined)
    ));
    assert_eq!(declined.preparation_work, 0);
    let reused = attempt(
        &mut workspace,
        &world(&supported, 1),
        EvaluationLimits::default(),
    );
    assert!(matches!(reused.result, Ok(FrozenPositiveOutcome::Stable)));
    assert_eq!(reused.preparation_work, 0);
}

#[test]
fn interrupted_owner_changes_preserve_exact_incidence_authority() {
    let first = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(1, 0),
        ],
        vec![1, 3],
    );
    let second = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(2, 0),
        ],
        vec![1, 3],
    );
    let first_candidate = world(&first, 7);
    let second_candidate = world(&second, 7);
    let cold = attempt(
        &mut FrozenPositiveWorkspace::default(),
        &second_candidate,
        EvaluationLimits::default(),
    );
    assert!(matches!(
        cold.result,
        Ok(FrozenPositiveOutcome::NonMinimal(_))
    ));
    for max_work in 0..cold.work {
        let mut workspace = FrozenPositiveWorkspace::default();
        assert!(matches!(
            attempt(
                &mut workspace,
                &first_candidate,
                EvaluationLimits::default()
            )
            .result,
            Ok(FrozenPositiveOutcome::NonMinimal(_))
        ));
        // Equal arena sizes do not authenticate the changed implication body.
        // This also reaches interruptions during the returned-witness copy.
        let interrupted = attempt(
            &mut workspace,
            &second_candidate,
            EvaluationLimits {
                max_work,
                ..EvaluationLimits::default()
            },
        );
        assert!(matches!(
            interrupted.result,
            Err(EvaluationError::Stopped(Stop::WorkLimit))
        ));
        assert_eq!(interrupted.work, max_work);
        for (owner, candidate, expected) in [
            (&first, &first_candidate, &[0, 1][..]),
            (&second, &second_candidate, &[1][..]),
        ] {
            let result = attempt(&mut workspace, candidate, EvaluationLimits::default());
            let Ok(FrozenPositiveOutcome::NonMinimal(witness)) = result.result else {
                panic!("the owner needs its own strict-subset witness")
            };
            assert!(witness.theory().same_instance(owner));
            assert_eq!(witness.atoms().collect::<Vec<_>>(), expected);
        }
    }
}
