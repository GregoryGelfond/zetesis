//! Whole-root grammar, linear incidence and exact reduct-family controls.

use super::*;
use crate::{AdmissionLimits, Limits, Node};

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

fn plan(theory: &Theory) -> PositivePlan {
    PositivePlan::compile(
        theory,
        PositivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn aliased() -> Theory {
    theory(
        3,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::False,
            Node::Implies(3, 3),
            Node::And(0, 0),
            Node::Or(1, 1),
            Node::Implies(4, 0),
            Node::Implies(5, 1),
            Node::Implies(6, 2),
        ],
        vec![7, 8, 9],
    )
}

#[test]
fn unsupported_atoms_do_not_enter_positive_cycles() {
    for fact in [false, true] {
        let mut roots = vec![3, 4];
        if fact {
            roots.push(0);
        }
        let owner = theory(
            3,
            vec![
                Node::Atom(0),
                Node::Atom(1),
                Node::Atom(2),
                Node::Implies(0, 1),
                Node::Implies(1, 0),
            ],
            roots,
        );
        let complete = plan(&owner);
        let expected = if fact { vec![0, 1] } else { vec![] };
        assert_eq!(
            complete.least_consequences().atoms().collect::<Vec<_>>(),
            expected
        );
        assert_eq!(complete.failed_constraint(), None);
    }
}

#[test]
fn aliased_children_keep_both_conjunction_signals() {
    let complete = plan(&aliased());
    assert_eq!(
        complete.least_consequences().atoms().collect::<Vec<_>>(),
        [0, 1, 2]
    );
    // Three atom-occurrence edges, two per binary body, three producer edges.
    assert_eq!(complete.statistics().dependencies, 10);
    assert_eq!(complete.statistics().propagated_dependencies, 10);
    assert_eq!(complete.statistics().derived_atoms, 3);
    assert_eq!(complete.statistics().activated_nodes, 6);
}

#[test]
fn failed_constraint_keeps_the_complete_least_consequences() {
    let owner = theory(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::False,
            Node::Implies(0, 1),
            Node::Implies(1, 2),
        ],
        vec![2, 0, 3, 4],
    );
    let complete = plan(&owner);
    assert_eq!(complete.failed_constraint(), Some(2));
    assert_eq!(
        complete.least_consequences().atoms().collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(
        !crate::models(
            &owner,
            complete.least_consequences(),
            Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
    );
}

#[test]
fn arbitrary_constraints_filter_answers_without_supplying_support() {
    for constraint in [3, 4, 6, 7] {
        for facts in 0..4 {
            let mut roots = vec![constraint];
            for atom in 0..2 {
                if facts & (1 << atom) != 0 {
                    roots.push(atom);
                }
            }
            let owner = theory(
                2,
                vec![
                    Node::Atom(0),
                    Node::Atom(1),
                    Node::False,
                    Node::Implies(0, 2),
                    Node::Implies(3, 2),
                    Node::Implies(0, 1),
                    Node::Implies(5, 2),
                    Node::Implies(6, 2),
                ],
                roots,
            );
            let complete = plan(&owner);
            assert_eq!(
                complete.least_consequences().atoms().collect::<Vec<_>>(),
                (0..2)
                    .filter(|atom| facts & (1 << atom) != 0)
                    .collect::<Vec<_>>()
            );
            for mask in 0..4 {
                let candidate =
                    Interpretation::new(&owner, (0..2).filter(|atom| mask & (1 << atom) != 0))
                        .unwrap();
                let reference = crate::check(
                    &owner,
                    &candidate,
                    Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                assert_eq!(
                    reference.accepted(),
                    complete.failed_constraint().is_none() && mask == facts,
                    "constraint={constraint}, facts={facts}, candidate={mask}"
                );
            }
        }
    }
}

#[test]
fn failed_nonmonotone_constraint_can_have_a_larger_classical_model() {
    // :- not a. The original theory is not not a. It classically requires a,
    // but supplies no producer for a, so neither interpretation is stable.
    let owner = theory(
        1,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Implies(2, 1),
        ],
        vec![3],
    );
    let complete = plan(&owner);
    assert_eq!(complete.least_consequences().atoms().count(), 0);
    assert_eq!(complete.failed_constraint(), Some(3));
    let larger = Interpretation::new(&owner, [0]).unwrap();
    assert!(crate::models(&owner, &larger, Limits::default(), &Cancellation::default()).unwrap());
    for candidate in [complete.least_consequences(), &larger] {
        assert!(
            !crate::check(
                &owner,
                candidate,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
            .accepted()
        );
    }
}

#[test]
fn final_original_evaluation_is_charged_and_cannot_publish_a_partial_plan() {
    let owner = theory(
        1,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(1, 1),
            Node::Implies(0, 2),
            Node::Implies(3, 1),
            Node::Implies(4, 1),
        ],
        vec![0, 5],
    );
    let complete = plan(&owner);
    assert_eq!(complete.failed_constraint(), None);
    let mut workspace = crate::EvaluationWorkspace::default();
    let evaluated = workspace.evaluate(
        complete.least_consequences(),
        crate::EvaluationLimits::default(),
        &Cancellation::default(),
    );
    assert!(evaluated.result.unwrap().is_model());
    let before_validation = complete.statistics().work - evaluated.work;
    for max_work in [before_validation, complete.statistics().work - 1] {
        let attempt = PositivePlan::compile_accounted(
            &owner,
            PositivePlanLimits {
                max_work,
                ..PositivePlanLimits::default()
            },
            &Cancellation::default(),
        );
        assert!(matches!(attempt.result, Err(PositiveError::Limit {
            resource: PositiveResource::Work, observed, limit,
        }) if observed == u128::from(max_work) + 1 && limit == u128::from(max_work)));
        assert_eq!(attempt.statistics.work, max_work);
        assert_eq!(attempt.statistics.retained_bytes, 0);
    }
}

#[test]
fn false_producer_from_an_invalid_closure_is_a_refusal() {
    let owner = theory(1, vec![Node::Atom(0)], vec![0]);
    let not_closed = Interpretation::new(&owner, []).unwrap();
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        limits: PositivePlanLimits::default(),
        cancellation: &cancellation,
        statistics: PositivePlanStatistics::default(),
        current_bytes: 0,
    };
    assert_eq!(
        validation::complete(&not_closed, &mut budget),
        Err(PositiveError::InvalidClosure { root: 0 })
    );
    assert_eq!(budget.statistics.work, 2); // One node and its false original root.
    assert_eq!(budget.statistics.retained_bytes, 0);
}

#[test]
fn complete_families_agree_with_general_reduct_checking() {
    // This finite family has positive cycles, both binary connectives, constants,
    // unsupported carrier atoms and positive constraints. The reference checker
    // enumerates proper subsets; it does not consume this plan or its graph.
    for body in [0, 1, 4, 5, 6, 7] {
        for facts in 0..4 {
            for constraint in [false, true] {
                let nodes = vec![
                    Node::Atom(0),
                    Node::Atom(1),
                    Node::Atom(2),
                    Node::Atom(3),
                    Node::False,
                    Node::Implies(4, 4),
                    Node::And(0, 1),
                    Node::Or(0, 1),
                    Node::Implies(body, 2),
                    Node::Implies(2, 0),
                    Node::Implies(1, 4),
                ];
                let mut roots = vec![8, 9];
                for atom in 0..2 {
                    if facts & (1 << atom) != 0 {
                        roots.push(atom);
                    }
                }
                if constraint {
                    roots.push(10);
                }
                let owner = theory(4, nodes, roots);
                let complete = plan(&owner);
                for mask in 0..16 {
                    let candidate =
                        Interpretation::new(&owner, (0..4).filter(|atom| mask & (1 << atom) != 0))
                            .unwrap();
                    let actual = crate::check(
                        &owner,
                        &candidate,
                        Limits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap();
                    let same = candidate.atoms().eq(complete.least_consequences().atoms());
                    assert_eq!(
                        actual.accepted(),
                        complete.failed_constraint().is_none() && same,
                        "body={body}, facts={facts}, constraint={constraint}, candidate={mask}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_original_root_must_have_a_supported_form() {
    let owner = theory(
        1,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Or(0, 2),
        ],
        vec![0, 3],
    );
    let attempt = PositivePlan::compile_accounted(
        &owner,
        PositivePlanLimits::default(),
        &Cancellation::default(),
    );
    assert!(matches!(
        attempt.result,
        Err(PositiveError::UnsupportedRoot { root: 3 })
    ));
    assert!(attempt.statistics.work > 0);
    assert_eq!(attempt.statistics.retained_bytes, 0);
}

#[test]
fn implication_bodies_cannot_enter_positive_plans() {
    for body in [3, 4, 5] {
        let owner = theory(
            2,
            vec![
                Node::Atom(0),
                Node::Atom(1),
                Node::False,
                Node::Implies(0, 2),
                Node::Implies(3, 2),
                Node::Implies(0, 1),
                Node::Implies(body, 1),
            ],
            vec![6],
        );
        let error = PositivePlan::compile(
            &owner,
            PositivePlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert_eq!(error, PositiveError::UnsupportedBody { root: 6, body });
    }
}

#[test]
fn every_proper_work_prefix_returns_no_certificate() {
    let owner = aliased();
    let complete = plan(&owner);
    let work = complete.statistics().work;
    for maximum in 0..work {
        let attempt = PositivePlan::compile_accounted(
            &owner,
            PositivePlanLimits {
                max_work: maximum,
                ..PositivePlanLimits::default()
            },
            &Cancellation::default(),
        );
        assert!(
            matches!(attempt.result, Err(PositiveError::Limit { resource: PositiveResource::Work, observed, limit }) if observed == u128::from(maximum) + 1 && limit == u128::from(maximum))
        );
        assert_eq!(attempt.statistics.work, maximum);
        assert_eq!(attempt.statistics.retained_bytes, 0);
    }
    let exact = PositivePlan::compile(
        &owner,
        PositivePlanLimits {
            max_work: work,
            ..PositivePlanLimits::default()
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        exact.least_consequences().atoms().collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn actual_dependency_limit_is_inclusive() {
    let owner = aliased();
    for maximum in [0, 9] {
        let attempt = PositivePlan::compile_accounted(
            &owner,
            PositivePlanLimits {
                max_dependencies: maximum,
                ..PositivePlanLimits::default()
            },
            &Cancellation::default(),
        );
        assert!(
            matches!(attempt.result, Err(PositiveError::Limit { resource: PositiveResource::Dependencies, observed, limit }) if observed == maximum as u128 + 1 && limit == maximum as u128)
        );
        assert_eq!(attempt.statistics.dependencies, maximum);
    }
    assert_eq!(
        PositivePlan::compile(
            &owner,
            PositivePlanLimits {
                max_dependencies: 10,
                ..PositivePlanLimits::default()
            },
            &Cancellation::default()
        )
        .unwrap()
        .statistics()
        .dependencies,
        10
    );
}

#[test]
fn construction_peak_covers_the_retained_owner() {
    let owner = aliased();
    let complete = plan(&owner);
    let peak = usize::try_from(complete.statistics().peak_bytes).unwrap();
    assert!(complete.statistics().retained_bytes < complete.statistics().peak_bytes);
    let exact = PositivePlan::compile(
        &owner,
        PositivePlanLimits {
            max_bytes: peak,
            ..PositivePlanLimits::default()
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(exact.statistics().peak_bytes, peak as u128);
    let short = PositivePlan::compile_accounted(
        &owner,
        PositivePlanLimits {
            max_bytes: peak - 1,
            ..PositivePlanLimits::default()
        },
        &Cancellation::default(),
    );
    assert!(
        matches!(short.result, Err(PositiveError::Limit { resource: PositiveResource::Bytes, observed, limit }) if observed > limit && limit == (peak - 1) as u128)
    );
    assert_eq!(short.statistics.retained_bytes, 0);
}

#[test]
fn cancellation_precedes_empty_shape_limits() {
    let owner = theory(0, vec![], vec![]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let attempt = PositivePlan::compile_accounted(
        &owner,
        PositivePlanLimits {
            max_work: 0,
            max_bytes: 0,
            max_dependencies: 0,
        },
        &cancellation,
    );
    assert!(matches!(
        attempt.result,
        Err(PositiveError::Stopped(Stop::Cancelled))
    ));
    assert_eq!(attempt.statistics, PositivePlanStatistics::default());
}

#[test]
fn empty_theory_keeps_its_exact_owner() {
    let owner = theory(65, vec![], vec![]);
    let foreign = theory(65, vec![], vec![]);
    let complete = plan(&owner);
    assert!(complete.theory().same_instance(&owner));
    assert!(!complete.theory().same_instance(&foreign));
    drop(owner);
    assert_eq!(complete.least_consequences().atoms().count(), 0);
    assert_eq!(complete.failed_constraint(), None);
    assert_eq!(complete.statistics().derived_atoms, 0);
}
