//! Differential semantic checks against the independent exhaustive reduct kernel.

use std::collections::BTreeSet;

use proptest::prelude::*;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory, Verdict};
use zetesis_sat::{Check, Control, Incomplete, Limits, StableModels, check};

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

fn interpretation(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

fn key(model: &Interpretation) -> Vec<usize> {
    model.atoms().collect()
}

fn compare(theory: &Theory) -> BTreeSet<Vec<usize>> {
    let mut expected = BTreeSet::new();
    for mask in 0..(1 << theory.atom_count()) {
        let candidate = interpretation(theory, mask);
        let exhaustive = zetesis_ferraris::check(
            theory,
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Control::default(),
        )
        .unwrap();
        let native = check(theory, &candidate, Limits::default(), &Control::default());
        assert_eq!(
            native.accepted(),
            exhaustive.accepted(),
            "{mask}: {native:?}"
        );
        match native {
            Check::Stable => {
                expected.insert(key(&candidate));
            }
            Check::NotModel => assert!(matches!(exhaustive.verdict(), Verdict::NotModel { .. })),
            Check::NonMinimal(subset) => {
                assert!(matches!(exhaustive.verdict(), Verdict::NonMinimal { .. }));
                assert_ne!(key(&subset), key(&candidate));
                assert!(subset.atoms().all(|atom| candidate.contains(atom)));
                assert!(
                    zetesis_ferraris::models_reduct(
                        theory,
                        &candidate,
                        &subset,
                        zetesis_ferraris::Limits::default(),
                        &Control::default()
                    )
                    .unwrap()
                );
            }
            Check::Inconclusive(error) => panic!("small complete membership: {error}"),
        }
    }
    let mut actual = BTreeSet::new();
    let mut models = StableModels::new(theory, Limits::default(), Control::default()).unwrap();
    for model in models.by_ref() {
        assert!(
            actual.insert(key(&model.unwrap())),
            "duplicate projected model"
        );
    }
    assert!(models.exhausted());
    assert!(models.next().is_none());
    assert_eq!(actual, expected);
    assert_eq!(
        models.statistics().stable_models,
        u64::try_from(actual.len()).unwrap()
    );
    actual
}

#[test]
fn disjunction_positive_cycles_and_unmentioned_atoms_preserve_minimality() {
    assert_eq!(
        compare(&theory(
            3,
            vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
            vec![2]
        )),
        BTreeSet::from([vec![0], vec![1]])
    );
    assert_eq!(
        compare(&theory(
            2,
            vec![
                Node::Atom(0),
                Node::Atom(1),
                Node::Implies(0, 1),
                Node::Implies(1, 0)
            ],
            vec![2, 3]
        )),
        BTreeSet::from([vec![]])
    );
    assert_eq!(
        compare(&theory(3, vec![], vec![])),
        BTreeSet::from([vec![]])
    );
}

#[test]
fn choices_double_negation_and_constraints_use_the_frozen_candidate() {
    // a ∨ ¬a; both interpretations stable, unlike the unsupported positive loop.
    assert_eq!(
        compare(&theory(
            1,
            vec![
                Node::Atom(0),
                Node::False,
                Node::Implies(0, 1),
                Node::Or(0, 2)
            ],
            vec![3]
        )),
        BTreeSet::from([vec![], vec![0]])
    );
    // ¬¬a -> a is the normal singleton choice encoding.
    assert_eq!(
        compare(&theory(
            1,
            vec![
                Node::Atom(0),
                Node::False,
                Node::Implies(0, 1),
                Node::Implies(2, 1),
                Node::Implies(3, 0)
            ],
            vec![4]
        )),
        BTreeSet::from([vec![], vec![0]])
    );
    // ¬¬a alone classically requires a, but its reduct cannot support a.
    assert!(
        compare(&theory(
            1,
            vec![
                Node::Atom(0),
                Node::False,
                Node::Implies(0, 1),
                Node::Implies(2, 1)
            ],
            vec![3]
        ))
        .is_empty()
    );
    assert!(
        compare(&theory(
            1,
            vec![Node::Atom(0), Node::False, Node::Implies(0, 1)],
            vec![0, 2]
        ))
        .is_empty()
    );
}

#[test]
fn zero_atoms_and_empty_roots_are_total() {
    assert_eq!(
        compare(&theory(0, vec![], vec![])),
        BTreeSet::from([vec![]])
    );
    assert!(compare(&theory(0, vec![Node::False], vec![0])).is_empty());
    assert_eq!(
        compare(&theory(0, vec![Node::False, Node::Implies(0, 0)], vec![1])),
        BTreeSet::from([vec![]])
    );
}

fn generated(atoms: usize, operations: &[(u8, usize, usize)], roots: &[bool]) -> Theory {
    let mut nodes: Vec<_> = (0..atoms).map(Node::Atom).collect();
    nodes.push(Node::False);
    for &(operation, left, right) in operations {
        let left = left % nodes.len();
        let right = right % nodes.len();
        nodes.push(match operation % 3 {
            0 => Node::And(left, right),
            1 => Node::Or(left, right),
            _ => Node::Implies(left, right),
        });
    }
    let selected = roots
        .iter()
        .enumerate()
        .filter_map(|(index, selected)| selected.then_some(index % nodes.len()))
        .collect();
    theory(atoms, nodes, selected)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn generated_formula_dags_match_every_exhaustive_candidate(
        atoms in 0usize..=4,
        operations in prop::collection::vec((any::<u8>(), 0usize..32, 0usize..32), 0..14),
        roots in prop::collection::vec(any::<bool>(), 0..20),
    ) {
        compare(&generated(atoms, &operations, &roots));
    }
}

#[test]
fn exact_candidate_ceiling_allows_final_unsat_query_and_failure_is_fused() {
    let input = theory(1, vec![Node::Atom(0)], vec![0]);
    let mut exact = StableModels::new(
        &input,
        Limits {
            max_candidates: 1,
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap();
    assert_eq!(key(&exact.next().unwrap().unwrap()), vec![0]);
    assert!(exact.next().is_none());
    assert!(exact.exhausted());
    assert_eq!(exact.statistics().candidate_queries, 2);
    let mut refused = StableModels::new(
        &input,
        Limits {
            max_candidates: 0,
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap();
    assert!(matches!(
        refused.next(),
        Some(Err(Incomplete::CandidateLimit))
    ));
    assert!(!refused.exhausted());
    assert!(refused.next().is_none());
    let contradiction = theory(0, vec![Node::False], vec![0]);
    let mut no_candidates = StableModels::new(
        &contradiction,
        Limits {
            max_candidates: 0,
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap();
    assert!(no_candidates.next().is_none());
    assert!(no_candidates.exhausted());
}

#[test]
fn empty_theory_exhausts_with_one_cnf_clause() {
    // Empty theory uses no original clauses; the independently admitted reduct
    // needs exactly one empty clause to exclude a nonexistent proper subset.
    let input = theory(0, vec![], vec![]);
    let limits = Limits {
        admission: zetesis_sat::AdmissionLimits {
            max_clauses: 1,
            ..Default::default()
        },
        reduct_admission: zetesis_sat::AdmissionLimits {
            max_clauses: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut exact = StableModels::new(&input, limits, Control::default()).unwrap();
    assert!(exact.next().unwrap().unwrap().atoms().next().is_none());
    assert!(exact.next().is_none());
    assert!(exact.exhausted());
    assert_eq!(exact.statistics().projections.entries, 1);
    assert_eq!(exact.statistics().projections.nodes, 0);
}

#[test]
fn verified_models_precede_the_history_limit_stop() {
    // Two independent choices compact to no outer clauses. Reduct construction
    // has its own admission; the history limit independently admits three keys.
    let choice = theory(
        2,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Or(0, 2),
            Node::Atom(1),
            Node::Implies(4, 1),
            Node::Or(4, 5),
        ],
        vec![3, 6],
    );
    let mut capped = StableModels::new(
        &choice,
        Limits {
            admission: zetesis_sat::AdmissionLimits {
                max_clauses: 3,
                ..Default::default()
            },
            projections: zetesis_sat::ProjectionLimits {
                max_entries: 3,
                ..Default::default()
            },
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap();
    let mut family = BTreeSet::new();
    for _ in 0..4 {
        let model = capped.next().unwrap().unwrap();
        assert!(model.theory().same_instance(&choice));
        assert!(family.insert(key(&model)));
    }
    assert_eq!(
        family,
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
    assert_eq!(capped.statistics().stable_models, 4);
    assert_eq!(capped.statistics().projections.entries, 3);
    assert!(matches!(
        capped.next(),
        Some(Err(Incomplete::ProjectionLimit {
            resource: zetesis_sat::ProjectionResource::Entries,
            required: 4,
            limit: 3,
        }))
    ));
    assert!(!capped.exhausted());
    assert!(capped.next().is_none());
}

#[test]
fn verified_model_precedes_the_final_exclusion_work_stop() {
    let input = theory(2, vec![], vec![]);
    let complete = Limits::default();
    let mut reference = StableModels::new(&input, complete, Control::default()).unwrap();
    assert!(reference.next().unwrap().is_ok());
    let after_first = reference.statistics().search.work;
    let bounded = Limits {
        search: zetesis_sat::SearchLimits {
            max_work: after_first - 1,
            ..Default::default()
        },
        ..complete
    };
    let mut limited = StableModels::new(&input, bounded, Control::default()).unwrap();
    assert!(
        limited.next().unwrap().is_ok(),
        "completed proof survives a final blocking-work refusal"
    );
    assert!(matches!(limited.next(), Some(Err(Incomplete::WorkLimit))));
    assert!(!limited.exhausted());
}

#[test]
fn foreign_identity_verification_limits_and_cancelled_enumeration_are_incomplete() {
    let input = theory(1, vec![Node::Atom(0)], vec![0]);
    let foreign = theory(1, vec![Node::Atom(0)], vec![0]);
    assert!(matches!(
        check(
            &input,
            &interpretation(&foreign, 1),
            Limits::default(),
            &Control::default()
        ),
        Check::Inconclusive(Incomplete::WrongTheory)
    ));
    assert!(matches!(
        check(
            &input,
            &interpretation(&input, 1),
            Limits {
                max_verification_work: 0,
                ..Default::default()
            },
            &Control::default()
        ),
        Check::Inconclusive(Incomplete::Verification(_))
    ));
    let control = Control::default();
    let mut models = StableModels::new(&input, Limits::default(), control.clone()).unwrap();
    control.cancel();
    assert!(matches!(models.next(), Some(Err(Incomplete::Cancelled))));
    assert!(!models.exhausted());
    assert!(models.next().is_none());
}

#[test]
fn repeated_commuted_classical_gates_fit_one_auxiliary_without_changing_reducts() {
    let mut nodes = vec![Node::Atom(0), Node::Atom(1)];
    for index in 0..512 {
        nodes.push(if index % 2 == 0 {
            Node::Or(0, 1)
        } else {
            Node::Or(1, 0)
        });
    }
    let input = theory(2, nodes, (2..514).collect());
    let mut cursor = StableModels::new(
        &input,
        Limits {
            admission: zetesis_sat::AdmissionLimits {
                max_variables: 3,
                ..Default::default()
            },
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap();
    let actual: BTreeSet<_> = cursor.by_ref().map(|model| key(&model.unwrap())).collect();
    assert!(cursor.exhausted());
    assert_eq!(actual, BTreeSet::from([vec![0], vec![1]]));
    assert_eq!(actual, compare(&input));
}
