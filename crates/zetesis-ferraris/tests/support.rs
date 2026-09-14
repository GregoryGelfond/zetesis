//! Candidate support restrictions retain the full reduct answer-set family.

use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Limits, Node, SupportError, SupportLimits, Theory, check,
    models, support_restriction,
};

fn limits() -> SupportLimits {
    SupportLimits {
        admission: AdmissionLimits::default(),
        max_work: 100_000,
    }
}

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

fn restrictions(original: &Theory) -> Theory {
    support_restriction(original, limits(), &Control::default())
        .result
        .unwrap()
        .unwrap()
}

fn satisfies(theory: &Theory, mask: usize) -> bool {
    let interpretation = Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|a| mask & (1 << a) != 0),
    )
    .unwrap();
    models(
        theory,
        &interpretation,
        Limits::default(),
        &Control::default(),
    )
    .unwrap()
}

#[test]
fn independent_disjunctions_have_only_supported_selections() {
    let original = theory(
        4,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::Atom(3),
            Node::Or(0, 1),
            Node::Or(2, 3),
        ],
        vec![4, 5],
    );
    let restriction = restrictions(&original);
    let retained: Vec<_> = (0..16)
        .filter(|&mask| satisfies(&original, mask) && satisfies(&restriction, mask))
        .collect();
    assert_eq!(retained, [5, 6, 9, 10]);
    assert!(!restriction.same_instance(&original));
}

#[test]
fn independent_facts_can_support_both_disjuncts() {
    let original = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![0, 1, 2],
    );
    assert!(satisfies(&restrictions(&original), 3));
}

#[test]
fn duplicate_head_occurrences_do_not_exclude_support() {
    let original = theory(
        1,
        vec![Node::Atom(0), Node::Or(0, 0), Node::Or(1, 1)],
        vec![2],
    );
    assert!(satisfies(&restrictions(&original), 1));
}

#[test]
fn false_rule_bodies_cannot_supply_support() {
    let original = theory(
        2,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Atom(1),
            Node::Or(1, 2),
            Node::Implies(0, 3),
        ],
        vec![4],
    );
    let restriction = restrictions(&original);
    assert_eq!(
        (0..4)
            .filter(|&mask| satisfies(&restriction, mask))
            .collect::<Vec<_>>(),
        [0]
    );
}

#[test]
fn rich_asserted_heads_decline_the_certificate() {
    let original = theory(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Or(0, 1),
            Node::And(0, 1),
        ],
        vec![2, 3],
    );
    let attempt = support_restriction(&original, limits(), &Control::default());
    assert!(attempt.result.unwrap().is_none());
    assert!(attempt.work > 0);
}

#[test]
fn ordinary_atoms_do_not_trigger_disjunctive_construction() {
    let original = theory(1, vec![Node::Atom(0)], vec![0]);
    assert!(
        support_restriction(&original, limits(), &Control::default())
            .result
            .unwrap()
            .is_none()
    );
}

#[test]
fn work_exhaustion_retains_charged_construction() {
    let original = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
    );
    let completed = support_restriction(&original, limits(), &Control::default());
    let boundary = completed.work;
    assert!(completed.result.unwrap().is_some());
    for max_work in 0..boundary {
        let attempt = support_restriction(
            &original,
            SupportLimits {
                max_work,
                ..limits()
            },
            &Control::default(),
        );
        assert!(matches!(
            attempt.result,
            Err(SupportError::Stopped(Stop::WorkLimit))
        ));
        assert_eq!(attempt.work, max_work);
    }
    assert!(
        support_restriction(
            &original,
            SupportLimits {
                max_work: boundary,
                ..limits()
            },
            &Control::default()
        )
        .result
        .unwrap()
        .is_some()
    );
}

#[test]
fn cancelled_construction_returns_no_restriction() {
    let original = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
    );
    let control = Control::default();
    control.cancel();
    let attempt = support_restriction(&original, limits(), &control);
    assert!(matches!(
        attempt.result,
        Err(SupportError::Stopped(Stop::Cancelled))
    ));
    assert_eq!(attempt.work, 0);
}

#[test]
fn support_preserves_every_small_reduct_answer_set() {
    // Exercise every subset of these asserted rules, including cycles,
    // disjunction, arbitrary implication bodies, and frozen constraints.
    let nodes = vec![
        Node::False,
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::Or(1, 2),
        Node::Or(2, 3),
        Node::Implies(1, 2),
        Node::Implies(2, 1),
        Node::Implies(1, 0),
        Node::Implies(8, 4),
        Node::Implies(6, 5),
        Node::Implies(4, 0),
    ];
    let optional = [1, 3, 5, 6, 7, 9, 10, 11];
    for chosen in 0..256 {
        let mut roots = vec![4];
        roots.extend(
            optional
                .iter()
                .enumerate()
                .filter(|(bit, _)| chosen & (1 << bit) != 0)
                .map(|(_, root)| *root),
        );
        let original = theory(3, nodes.clone(), roots);
        let restriction = restrictions(&original);
        for mask in 0..8 {
            let candidate =
                Interpretation::new(&original, (0..3).filter(|a| mask & (1 << a) != 0)).unwrap();
            let checked = check(
                &original,
                &candidate,
                Limits::default(),
                &Control::default(),
            )
            .unwrap();
            if checked.accepted() {
                assert!(
                    satisfies(&restriction, mask),
                    "roots={chosen}, interpretation={mask}"
                );
            }
        }
    }
}

#[test]
fn atomic_choices_supply_selected_head_support() {
    let original = theory(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::False,
            Node::Or(0, 1),
            Node::Implies(0, 2),
            Node::Or(0, 4),
        ],
        vec![3, 5],
    );
    let restriction = restrictions(&original);
    // a has independent choice permission; b still needs the ordinary a-or-b
    // producer with a false. An empty original model is not added by the guard.
    assert_eq!((0..4).filter(|&mask| satisfies(&original, mask)
        && satisfies(&restriction, mask)).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn exact_choices_without_ordinary_disjunction_decline() {
    let original = theory(1, vec![Node::False, Node::Atom(0),
        Node::Implies(1, 0), Node::Or(1, 2)], vec![3]);
    let attempt = support_restriction(&original, limits(), &Control::default());
    assert!(attempt.result.unwrap().is_none());
    // Four node classifications and one complete root inspection. The bounded
    // atomic-choice shape read is part of that charged root operation.
    assert_eq!(attempt.work, 5);
}

#[test]
fn conditional_choices_require_the_original_body() {
    for reversed in [false, true] {
        // (a or b), and an independent choice of c only when a is true.
        // The negative c uses a separate occurrence with the same atom index.
        let choice = if reversed { Node::Or(6, 2) } else { Node::Or(2, 6) };
        let original = theory(3, vec![Node::Atom(0), Node::Atom(1),
            Node::Atom(2), Node::False, Node::Or(0, 1), Node::Atom(2),
            Node::Implies(5, 3), choice, Node::Implies(0, 7)], vec![4, 8]);
        let restriction = restrictions(&original);
        let retained: Vec<_> = (0..8).filter(|&mask| satisfies(&original, mask)
            && satisfies(&restriction, mask)).collect();
        assert_eq!(retained, [1, 2, 5]);
    }
}

#[test]
fn opaque_choice_alternatives_decline_complete_extraction() {
    let nodes = vec![Node::Atom(0), Node::Atom(1), Node::False,
        Node::Or(0, 1), Node::Implies(0, 2), Node::Implies(1, 2),
        Node::Or(0, 4), Node::Or(0, 5), Node::Implies(4, 2),
        Node::Or(0, 8), Node::And(6, 6), Node::Or(6, 1)];
    // Cross-atom, double-negated, conjunction and nested-choice alternatives.
    // The ordinary disjunction remains first, so no partial root inventory is
    // allowed to produce a restriction before the later opaque root declines.
    for opaque in [7, 9, 10, 11] {
        let original = theory(2, nodes.clone(), vec![3, opaque]);
        assert!(support_restriction(&original, limits(), &Control::default())
            .result.unwrap().is_none());
    }
}

#[test]
fn mixed_support_preserves_every_small_reduct_answer_set() {
    let nodes = vec![Node::False, Node::Atom(0), Node::Atom(1), Node::Atom(2),
        Node::Or(1, 2), Node::Implies(3, 0), Node::Or(3, 5),
        Node::Or(5, 3), Node::Implies(1, 6), Node::Implies(1, 2),
        Node::Implies(9, 7), Node::Implies(2, 1), Node::Implies(4, 0),
        Node::Or(4, 4), Node::Implies(0, 6)];
    let optional = [1, 3, 6, 8, 10, 11, 12, 14];
    for chosen in 0..256 {
        let mut roots = vec![13];
        roots.extend(optional.iter().enumerate()
            .filter(|(bit, _)| chosen & (1 << bit) != 0).map(|(_, root)| *root));
        let original = theory(3, nodes.clone(), roots);
        let restriction = restrictions(&original);
        for mask in 0..8 {
            let candidate = Interpretation::new(&original,
                (0..3).filter(|a| mask & (1 << a) != 0)).unwrap();
            if check(&original, &candidate, Limits::default(), &Control::default())
                .unwrap().accepted() {
                assert!(satisfies(&restriction, mask), "roots={chosen}, interpretation={mask}");
            }
        }
    }
}

#[test]
fn support_admission_dimensions_are_inclusive() {
    use zetesis_ferraris::AdmissionError;
    let original = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
    );
    let result = restrictions(&original);
    let exact = AdmissionLimits {
        max_atoms: 2,
        max_nodes: result.nodes().len(),
        max_roots: 2,
    };
    assert!(
        support_restriction(
            &original,
            SupportLimits {
                admission: exact,
                ..limits()
            },
            &Control::default()
        )
        .result
        .unwrap()
        .is_some()
    );
    for admission in [
        AdmissionLimits {
            max_atoms: 1,
            ..exact
        },
        AdmissionLimits {
            max_nodes: exact.max_nodes - 1,
            ..exact
        },
        AdmissionLimits {
            max_roots: 1,
            ..exact
        },
    ] {
        assert!(matches!(
            support_restriction(
                &original,
                SupportLimits {
                    admission,
                    ..limits()
                },
                &Control::default()
            )
            .result,
            Err(SupportError::Admission(AdmissionError::Limit))
        ));
    }
}

#[test]
fn mixed_work_refusals_publish_no_partial_restriction() {
    let original = theory(3, vec![Node::Atom(0), Node::Atom(1), Node::Atom(2),
        Node::False, Node::Or(0, 1), Node::Implies(2, 3), Node::Or(2, 5)], vec![4, 6]);
    let complete = support_restriction(&original, limits(), &Control::default());
    assert!(complete.result.unwrap().is_some());
    for max_work in 0..=complete.work {
        let attempt = support_restriction(&original, SupportLimits { max_work, ..limits() },
            &Control::default());
        assert_eq!(attempt.work, max_work);
        if max_work == complete.work {
            assert!(attempt.result.unwrap().is_some());
        } else {
            assert!(matches!(attempt.result, Err(SupportError::Stopped(Stop::WorkLimit))));
        }
    }
}

#[test]
fn mixed_node_limits_publish_no_partial_restriction() {
    let original = theory(3, vec![Node::Atom(0), Node::Atom(1), Node::Atom(2),
        Node::False, Node::Or(0, 1), Node::Implies(2, 3), Node::Or(2, 5)], vec![4, 6]);
    let complete = restrictions(&original);
    for max_nodes in original.nodes().len()..=complete.nodes().len() {
        let mut bound = limits();
        bound.admission.max_nodes = max_nodes;
        let attempt = support_restriction(&original, bound, &Control::default());
        if max_nodes == complete.nodes().len() {
            let restriction = attempt.result.unwrap().unwrap();
            assert_eq!(restriction.nodes(), complete.nodes());
            assert_eq!(restriction.roots(), complete.roots());
        } else {
            assert!(matches!(attempt.result, Err(SupportError::Admission(
                zetesis_ferraris::AdmissionError::Limit))));
        }
    }
}

#[test]
fn shared_head_dags_do_not_expand_into_occurrence_trees() {
    let mut nodes = vec![Node::Atom(0)];
    for _ in 0..128 {
        let previous = nodes.len() - 1;
        nodes.push(Node::Or(previous, previous));
    }
    let original = theory(1, nodes, vec![128]);
    let result = support_restriction(
        &original,
        SupportLimits {
            max_work: 1_000,
            ..limits()
        },
        &Control::default(),
    )
    .result
    .unwrap()
    .unwrap();
    assert!(satisfies(&result, 1));
}
