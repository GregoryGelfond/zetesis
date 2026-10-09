//! Independent reduct agreement and complete-certificate boundaries.

use super::*;
use crate::{AdmissionLimits, FormulaParts, Limits, Node, OperandSpan};

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        atoms,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn plan(theory: &Theory) -> StratifiedPlan {
    StratifiedPlan::compile(
        theory,
        StratifiedPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn agrees_with_reduct(theory: &Theory, plan: &StratifiedPlan) {
    for mask in 0..(1 << theory.atom_count()) {
        let candidate = Interpretation::new(
            theory,
            (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
        )
        .unwrap();
        let reference = crate::check(
            theory,
            &candidate,
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            reference.accepted(),
            plan.failed_constraint().is_none() && candidate.atoms().eq(plan.consequences().atoms()),
            "candidate={mask}"
        );
    }
}

#[test]
fn lower_positive_cycles_finish_before_negation() {
    for fact in [false, true] {
        // a :- b. b :- a. c :- not a. d :- not c.
        let mut roots = vec![5, 6, 8, 10];
        if fact {
            roots.push(0);
        }
        let owner = theory(
            4,
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::atom(3),
                Node::falsum(),
                Node::implies(1, 0),
                Node::implies(0, 1),
                Node::implies(0, 4),
                Node::implies(7, 2),
                Node::implies(2, 4),
                Node::implies(9, 3),
            ],
            roots,
        );
        let complete = plan(&owner);
        let expected = if fact { vec![0, 1, 3] } else { vec![2] };
        assert_eq!(
            complete.consequences().atoms().collect::<Vec<_>>(),
            expected
        );
        agrees_with_reduct(&owner, &complete);
    }
}

#[test]
fn all_small_certified_families_agree_with_reduct() {
    // Enumerate every subset of six positive/negative rules, four fact sets and
    // four constraint selections. The reference independently searches subsets.
    for selected in 0..64 {
        for facts in 0..4 {
            for constraint in [None, Some(3), Some(4), Some(11)] {
                let mut roots: Vec<_> = (0..6)
                    .filter(|rule| selected & (1 << rule) != 0)
                    .map(|rule| rule + 5)
                    .collect();
                for atom in 0..2 {
                    if facts & (1 << atom) != 0 {
                        roots.push(atom);
                    }
                }
                roots.extend(constraint);
                let owner = theory(
                    2,
                    vec![
                        Node::atom(0),
                        Node::atom(1),
                        Node::falsum(),
                        Node::implies(0, 2),
                        Node::implies(1, 2),
                        Node::implies(1, 0),
                        Node::implies(0, 1),
                        Node::implies(4, 0),
                        Node::implies(3, 1),
                        Node::implies(3, 0),
                        Node::implies(4, 1),
                        Node::implies(3, 2),
                    ],
                    roots,
                );
                match StratifiedPlan::compile(
                    &owner,
                    StratifiedPlanLimits::default(),
                    &Cancellation::default(),
                ) {
                    Ok(complete) => agrees_with_reduct(&owner, &complete),
                    Err(StratifiedError::NegativeCycle { .. }) => {
                        assert!(selected & 0b11_1100 != 0);
                    }
                    Err(error) => panic!("unexpected refusal: {error}"),
                }
            }
        }
    }
}

#[test]
fn signed_conjunctions_preserve_complete_families() {
    for facts in 0..4 {
        for body in [0, 1, 2, 3, 4, 5, 6, 7] {
            let mut roots = vec![10, 11];
            for atom in 0..2 {
                if facts & (1 << atom) != 0 {
                    roots.push(atom);
                }
            }
            let owner = theory(
                4,
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::falsum(),
                    Node::implies(2, 2),
                    Node::implies(0, 2),
                    Node::implies(1, 2),
                    Node::and_pair([0, 5]),
                    Node::and_pair([4, 5]),
                    Node::atom(2),
                    Node::atom(3),
                    Node::implies(body, 8),
                    Node::implies(8, 9),
                ],
                roots,
            );
            agrees_with_reduct(&owner, &plan(&owner));
        }
    }
}

#[test]
fn repeated_nary_operands_keep_their_incidence_counts() {
    let owner = Theory::new(
        2,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 2),
                Node::and_span(OperandSpan {
                    start: 0,
                    length: 3,
                }),
                Node::implies(4, 1),
            ],
            vec![3, 3, 3],
        )
        .unwrap(),
        vec![5],
        AdmissionLimits::default(),
    )
    .unwrap();
    let complete = plan(&owner);
    assert_eq!(complete.consequences().atoms().collect::<Vec<_>>(), [1]);
    agrees_with_reduct(&owner, &complete);
}

#[test]
fn negative_self_cycles_are_not_certificates() {
    let owner = theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::implies(2, 0),
        ],
        vec![3],
    );
    assert!(matches!(
        StratifiedPlan::compile(
            &owner,
            StratifiedPlanLimits::default(),
            &Cancellation::default()
        ),
        Err(StratifiedError::NegativeCycle { atom: 0, body: 2 })
    ));
}

#[test]
fn negative_cycles_through_positive_edges_are_refused() {
    // a :- not b. b :- c. c :- a.
    let owner = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(1, 3),
            Node::implies(4, 0),
            Node::implies(2, 1),
            Node::implies(0, 2),
        ],
        vec![5, 6, 7],
    );
    assert!(matches!(
        StratifiedPlan::compile(
            &owner,
            StratifiedPlanLimits::default(),
            &Cancellation::default()
        ),
        Err(StratifiedError::NegativeCycle { atom: 1, body: 4 })
    ));
}

#[test]
fn constraints_cannot_supply_missing_support() {
    // :- not a. It classically requires a, but has no answer set.
    let owner = theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::implies(2, 1),
        ],
        vec![3],
    );
    let complete = plan(&owner);
    assert_eq!(complete.failed_constraint(), Some(3));
    assert_eq!(complete.consequences().atoms().count(), 0);
    let larger = Interpretation::new(&owner, [0]).unwrap();
    assert!(crate::models(&owner, &larger, Limits::default(), &Cancellation::default()).unwrap());
    agrees_with_reduct(&owner, &complete);
}

#[test]
fn arbitrary_constraints_filter_stratified_consequences() {
    for constraint in [3, 6, 7, 8] {
        let owner = theory(
            2,
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 2),
                Node::implies(3, 1),
                Node::implies(1, 0),
                Node::implies(5, 2),
                Node::implies(6, 2),
                Node::implies(4, 2),
            ],
            vec![4, constraint],
        );
        agrees_with_reduct(&owner, &plan(&owner));
    }
}

#[test]
fn every_root_is_checked_after_a_false_constraint() {
    let owner = theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::or_pair([0, 2]),
        ],
        vec![1, 3],
    );
    assert_eq!(
        StratifiedPlan::compile(
            &owner,
            StratifiedPlanLimits::default(),
            &Cancellation::default()
        )
        .unwrap_err(),
        StratifiedError::UnsupportedRoot { root: 3 }
    );
}

#[test]
fn choices_cannot_be_normal_producers() {
    for root in [3, 5] {
        let owner = theory(
            2,
            vec![
                Node::atom(0),
                Node::falsum(),
                Node::implies(0, 1),
                Node::or_pair([0, 2]),
                Node::atom(1),
                Node::implies(4, 3),
            ],
            vec![root],
        );
        assert_eq!(
            StratifiedPlan::compile(
                &owner,
                StratifiedPlanLimits::default(),
                &Cancellation::default()
            )
            .unwrap_err(),
            StratifiedError::UnsupportedRoot { root }
        );
    }
}

#[test]
fn nonnormal_bodies_are_refused() {
    for body in [4, 5, 6, 7] {
        let owner = theory(
            2,
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 2),
                Node::implies(3, 2),
                Node::implies(0, 1),
                Node::or_pair([0, 1]),
                Node::and_pair([1, 5]),
                Node::implies(body, 1),
            ],
            vec![8],
        );
        assert_eq!(
            StratifiedPlan::compile(
                &owner,
                StratifiedPlanLimits::default(),
                &Cancellation::default()
            )
            .unwrap_err(),
            StratifiedError::UnsupportedBody { root: 8, body }
        );
    }
}

fn small() -> Theory {
    theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::implies(3, 1),
        ],
        vec![4],
    )
}

#[test]
fn every_incomplete_work_prefix_returns_no_plan() {
    let owner = small();
    let complete = plan(&owner);
    for max_work in 0..complete.statistics().work {
        let attempt = StratifiedPlan::compile_accounted(
            &owner,
            StratifiedPlanLimits {
                max_work,
                ..StratifiedPlanLimits::default()
            },
            &Cancellation::default(),
        );
        assert!(matches!(attempt.result, Err(StratifiedError::Limit {
            resource: StratifiedResource::Work, observed, limit,
        }) if observed == u128::from(max_work) + 1 && limit == u128::from(max_work)));
        assert_eq!(attempt.statistics.work, max_work);
        assert_eq!(attempt.statistics.retained_bytes, 0);
    }
    let exact = StratifiedPlan::compile(
        &owner,
        StratifiedPlanLimits {
            max_work: complete.statistics().work,
            ..StratifiedPlanLimits::default()
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(exact.consequences().atoms().collect::<Vec<_>>(), [1]);
}

#[test]
fn dependency_limit_is_inclusive() {
    let owner = small();
    let dependencies = plan(&owner).statistics().dependencies;
    for max_dependencies in [0, dependencies - 1] {
        let attempt = StratifiedPlan::compile_accounted(
            &owner,
            StratifiedPlanLimits {
                max_dependencies,
                ..StratifiedPlanLimits::default()
            },
            &Cancellation::default(),
        );
        assert!(matches!(attempt.result, Err(StratifiedError::Limit {
            resource: StratifiedResource::Dependencies, observed, limit,
        }) if observed == max_dependencies as u128 + 1 && limit == max_dependencies as u128));
        assert_eq!(attempt.statistics.dependencies, max_dependencies);
        assert_eq!(attempt.statistics.retained_bytes, 0);
    }
    assert!(
        StratifiedPlan::compile(
            &owner,
            StratifiedPlanLimits {
                max_dependencies: dependencies,
                ..StratifiedPlanLimits::default()
            },
            &Cancellation::default()
        )
        .is_ok()
    );
}

#[test]
fn simultaneous_storage_limit_is_inclusive() {
    let owner = small();
    let complete = plan(&owner);
    let peak = usize::try_from(complete.statistics().peak_bytes).unwrap();
    assert!(complete.statistics().retained_bytes < peak as u128);
    let exact = StratifiedPlan::compile(
        &owner,
        StratifiedPlanLimits {
            max_bytes: peak,
            ..StratifiedPlanLimits::default()
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(exact.statistics().peak_bytes, peak as u128);
    let refused = StratifiedPlan::compile_accounted(
        &owner,
        StratifiedPlanLimits {
            max_bytes: peak - 1,
            ..StratifiedPlanLimits::default()
        },
        &Cancellation::default(),
    );
    assert!(matches!(refused.result, Err(StratifiedError::Limit {
        resource: StratifiedResource::Bytes, observed, limit,
    }) if observed > limit && limit == (peak - 1) as u128));
    assert_eq!(refused.statistics.retained_bytes, 0);
}

#[test]
fn cancellation_precedes_empty_limits() {
    let owner = theory(0, vec![], vec![]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let attempt = StratifiedPlan::compile_accounted(
        &owner,
        StratifiedPlanLimits {
            max_bytes: 0,
            max_work: 0,
            max_dependencies: 0,
        },
        &cancellation,
    );
    assert_eq!(
        attempt.result.unwrap_err(),
        StratifiedError::Stopped(Stop::Cancelled)
    );
    assert_eq!(attempt.statistics, StratifiedPlanStatistics::default());
}

#[test]
fn an_empty_plan_retains_its_exact_owner() {
    let owner = theory(65, vec![], vec![]);
    let foreign = theory(65, vec![], vec![]);
    let complete = plan(&owner);
    assert!(complete.theory().same_instance(&owner));
    assert!(!complete.theory().same_instance(&foreign));
    drop(owner);
    assert_eq!(complete.consequences().atoms().count(), 0);
    assert_eq!(complete.failed_constraint(), None);
    assert_eq!(complete.statistics().derived_atoms, 0);
}

#[test]
fn final_evaluation_cannot_certify_an_invalid_closure() {
    let owner = theory(1, vec![Node::atom(0)], vec![0]);
    let wrong = Interpretation::new(&owner, []).unwrap();
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        limits: StratifiedPlanLimits::default(),
        cancellation: &cancellation,
        statistics: StratifiedPlanStatistics::default(),
        current_bytes: 0,
    };
    assert_eq!(
        evaluate::validate(&wrong, &mut budget),
        Err(StratifiedError::InvalidClosure { root: 0 })
    );
    assert_eq!(budget.statistics.retained_bytes, 0);
}

#[test]
fn deep_shared_bodies_do_not_require_recursive_walks() {
    let mut nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::falsum(),
        Node::implies(0, 2),
    ];
    for _ in 0..4096 {
        nodes.push(Node::and_pair([nodes.len() - 1, 3]));
    }
    nodes.push(Node::implies(nodes.len() - 1, 1));
    let root = nodes.len() - 1;
    let owner = theory(2, nodes, vec![root]);
    let complete = plan(&owner);
    assert_eq!(complete.consequences().atoms().collect::<Vec<_>>(), [1]);
    assert!(complete.statistics().work < 100 * owner.nodes().len() as u64);
}
