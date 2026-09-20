//! One prepared owner must preserve every candidate's frozen-reduct decision.

use proptest::prelude::*;
use zetesis_ferraris::{Interpretation, Node, Theory, Verdict};
use zetesis_sat::{
    Check, Control, Incomplete, Limits, PreparedReduct, ReductPreparationLimits, ReductWorkspace,
    SearchLimits,
};

const BYTES: u64 = 4 * 1024 * 1024;

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        atoms,
        nodes,
        roots,
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

fn candidate(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

fn prepare(theory: &Theory) -> PreparedReduct {
    PreparedReduct::prepare(
        theory,
        ReductPreparationLimits::default(),
        &Control::default(),
    )
    .result
    .unwrap()
}

fn compare(input: &Theory) {
    let prepared = prepare(input);
    let retained = prepared.statistics();
    let control = Control::default();
    let mut workspace = ReductWorkspace::default();
    // Descending then ascending candidates forces previously true/false
    // implication parameters and subset units to be replaced in both directions.
    for mask in (0..1 << input.atom_count())
        .rev()
        .chain(0..1 << input.atom_count())
    {
        let current = candidate(input, mask);
        let (actual, receipt) = prepared.check(
            &current,
            &mut workspace,
            Limits {
                max_reduct_bytes: BYTES,
                ..Limits::default()
            },
            &control,
        );
        let fresh = zetesis_sat::check_with(
            input,
            &current,
            zetesis_sat::SearchMethod::Clauses,
            Limits::default(),
            &control,
        );
        let exhaustive = zetesis_ferraris::check(
            input,
            &current,
            zetesis_ferraris::Limits::default(),
            &control,
        )
        .unwrap();
        assert_eq!(
            actual.accepted(),
            fresh.accepted(),
            "fresh: {mask}: {actual:?}"
        );
        assert_eq!(
            actual.accepted(),
            exhaustive.accepted(),
            "exhaustive: {mask}: {actual:?}"
        );
        match actual {
            Check::Stable => assert!(matches!(exhaustive.verdict(), Verdict::Stable)),
            Check::NotModel => {
                assert!(matches!(fresh, Check::NotModel));
                assert!(matches!(exhaustive.verdict(), Verdict::NotModel { .. }));
                assert_eq!(receipt.statistics.countermodel_queries, 0);
            }
            Check::NonMinimal(witness) => {
                assert!(matches!(fresh, Check::NonMinimal(_)));
                assert!(matches!(exhaustive.verdict(), Verdict::NonMinimal { .. }));
                assert!(witness.theory().same_instance(input));
                assert!(witness.atoms().all(|atom| current.contains(atom)));
                assert!(witness.atoms().count() < current.atoms().count());
                assert!(
                    zetesis_ferraris::models_reduct(
                        input,
                        &current,
                        &witness,
                        zetesis_ferraris::Limits::default(),
                        &control
                    )
                    .unwrap()
                );
                assert_eq!(receipt.statistics.countermodels, 1);
            }
            Check::Inconclusive(error) => panic!("complete small query: {error}"),
        }
        assert_eq!(
            prepared.statistics(),
            retained,
            "queries cannot mutate the immutable owner"
        );
        assert_eq!(receipt.retained_bytes, workspace.retained_bytes());
        assert!(receipt.statistics.reduct.parameter_work <= receipt.statistics.search.work);
    }
}

#[test]
fn false_original_implication_does_not_retain_classical_equivalence() {
    // At M={a}, (not a -> a) freezes to (bottom -> a). Empty is a
    // countermodel; retaining the old not-a equivalence would lose it.
    let input = theory(
        1,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Implies(2, 0),
        ],
        vec![3],
    );
    let prepared = prepare(&input);
    let (result, _) = prepared.check(
        &candidate(&input, 1),
        &mut ReductWorkspace::default(),
        Limits {
            max_reduct_bytes: BYTES,
            ..Limits::default()
        },
        &Control::default(),
    );
    let Check::NonMinimal(witness) = result else {
        panic!("expected empty witness: {result:?}")
    };
    assert_eq!(witness.atoms().count(), 0);
    compare(&input);
}

#[test]
fn unmentioned_atoms_and_empty_universe_preserve_strict_subset() {
    compare(&theory(3, vec![], vec![]));
    compare(&theory(0, vec![], vec![]));
    compare(&theory(0, vec![Node::False], vec![0]));
}

#[test]
fn changing_choice_parameters_preserves_the_complete_family() {
    compare(&theory(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::False,
            Node::Implies(0, 2),
            Node::Or(0, 3),
            Node::Implies(1, 2),
            Node::Or(1, 5),
        ],
        vec![4, 6],
    ));
}

#[test]
fn foreign_equal_theory_is_refused_without_changing_workspace() {
    let input = theory(1, vec![Node::Atom(0)], vec![0]);
    let foreign = theory(1, vec![Node::Atom(0)], vec![0]);
    let prepared = prepare(&input);
    assert!(prepared.same_owner(&prepared.clone()));
    assert!(!prepared.same_owner(&prepare(&input)));
    let mut workspace = ReductWorkspace::default();
    let before = workspace.retained_bytes();
    let (result, receipt) = prepared.check(
        &candidate(&foreign, 1),
        &mut workspace,
        Limits {
            max_reduct_bytes: BYTES,
            ..Limits::default()
        },
        &Control::default(),
    );
    assert!(matches!(
        result,
        Check::Inconclusive(Incomplete::WrongTheory)
    ));
    assert_eq!(receipt.statistics.reduct.original_work, 0);
    assert_eq!(receipt.statistics.search.work, 0);
    assert_eq!(workspace.retained_bytes(), before);
}

#[test]
fn every_preparation_work_refusal_preserves_the_exact_prefix() {
    let input = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1)],
        vec![2],
    );
    let required = prepare(&input).statistics().work;
    for max_work in 0..required {
        let failure = PreparedReduct::prepare(
            &input,
            ReductPreparationLimits {
                max_work,
                ..Default::default()
            },
            &Control::default(),
        );
        assert_eq!(failure.result.unwrap_err(), Incomplete::WorkLimit);
        assert_eq!(failure.statistics.work, max_work);
        assert_eq!(failure.statistics.retained_bytes, 0);
    }
    let complete = PreparedReduct::prepare(
        &input,
        ReductPreparationLimits {
            max_work: required,
            ..Default::default()
        },
        &Control::default(),
    );
    let receipt = complete.statistics;
    let prepared = complete.result.unwrap();
    assert_eq!(receipt, prepared.statistics());
    assert_eq!(receipt.work, required);
}

#[test]
fn refused_preparation_capacity_is_not_reported_as_allocated_peak() {
    let input = theory(1, vec![Node::Atom(0)], vec![0]);
    let full = prepare(&input).statistics();
    let failure = PreparedReduct::prepare(
        &input,
        ReductPreparationLimits {
            max_bytes: 0,
            ..Default::default()
        },
        &Control::default(),
    );
    let Incomplete::ReductStorage { required, limit: 0 } = *failure.result.as_ref().unwrap_err()
    else {
        panic!("{failure:?}")
    };
    assert!(failure.statistics.peak_bytes < required);
    assert!(failure.statistics.peak_bytes < full.peak_bytes);
    assert_eq!(failure.statistics.retained_bytes, 0);
}

#[test]
fn every_cold_query_work_stop_allows_a_complete_retry() {
    let input = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
    );
    let prepared = prepare(&input);
    let current = candidate(&input, 3);
    let mut workspace = ReductWorkspace::default();
    let control = Control::default();
    let (complete, full) = prepared.check(
        &current,
        &mut workspace,
        Limits {
            max_reduct_bytes: BYTES,
            ..Limits::default()
        },
        &control,
    );
    assert!(matches!(complete, Check::NonMinimal(_)));
    let retained = workspace.retained_bytes();
    for max_work in 0..full.statistics.search.work {
        // Cold and warm queries deliberately perform different work. Recreate
        // the same cold prestate for each exact refusal boundary.
        let mut workspace = ReductWorkspace::default();
        let (result, prefix) = prepared.check(
            &current,
            &mut workspace,
            Limits {
                search: SearchLimits {
                    max_work,
                    ..Default::default()
                },
                ..Default::default()
            },
            &control,
        );
        assert!(
            matches!(result, Check::Inconclusive(Incomplete::WorkLimit)),
            "{max_work}: {result:?}"
        );
        assert_eq!(prefix.statistics.search.work, max_work);
        assert_eq!(workspace.retained_bytes(), retained);
        let (retried, _) = prepared.check(
            &candidate(&input, 1),
            &mut workspace,
            Limits {
                max_reduct_bytes: BYTES,
                ..Limits::default()
            },
            &control,
        );
        assert!(matches!(retried, Check::Stable));
    }
    let (exact, receipt) = prepared.check(
        &current,
        &mut ReductWorkspace::default(),
        Limits {
            search: SearchLimits {
                max_work: full.statistics.search.work,
                ..Default::default()
            },
            ..Default::default()
        },
        &control,
    );
    assert!(matches!(exact, Check::NonMinimal(_)));
    assert_eq!(receipt.statistics.search.work, full.statistics.search.work);
}

#[test]
fn original_evaluation_stop_never_enters_subset_search() {
    let input = theory(1, vec![Node::Atom(0)], vec![0]);
    let (result, receipt) = prepare(&input).check(
        &candidate(&input, 1),
        &mut ReductWorkspace::default(),
        Limits {
            max_verification_work: 1,
            ..Default::default()
        },
        &Control::default(),
    );
    assert!(matches!(
        result,
        Check::Inconclusive(Incomplete::Verification(zetesis_cpu::Stop::WorkLimit))
    ));
    assert_eq!(receipt.statistics.reduct.original_work, 1);
    assert_eq!(receipt.statistics.countermodel_queries, 0);
}

#[test]
fn cancellation_precedes_query_storage_admission() {
    let input = theory(1, vec![Node::Atom(0)], vec![0]);
    let prepared = prepare(&input);
    let control = Control::default();
    control.cancel();
    let mut workspace = ReductWorkspace::default();
    let before = workspace.retained_bytes();
    let (result, receipt) = prepared.check(
        &candidate(&input, 1),
        &mut workspace,
        Limits {
            max_reduct_bytes: 0,
            ..Limits::default()
        },
        &control,
    );
    assert!(matches!(result, Check::Inconclusive(Incomplete::Cancelled)));
    assert_eq!(receipt.retained_bytes, before);
    assert_eq!(receipt.statistics.reduct.original_work, 0);
}

#[test]
fn lowered_query_storage_limit_charges_retained_capacity() {
    let input = theory(2, vec![], vec![]);
    let prepared = prepare(&input);
    let mut workspace = ReductWorkspace::default();
    let current = candidate(&input, 3);
    let control = Control::default();
    assert!(matches!(
        prepared
            .check(
                &current,
                &mut workspace,
                Limits {
                    max_reduct_bytes: BYTES,
                    ..Limits::default()
                },
                &control
            )
            .0,
        Check::NonMinimal(_)
    ));
    let retained = workspace.retained_bytes();
    let limit = u64::try_from(retained).unwrap() - 1;
    let (result, receipt) = prepared.check(
        &current,
        &mut workspace,
        Limits {
            max_reduct_bytes: limit,
            ..Limits::default()
        },
        &control,
    );
    assert!(
        matches!(result, Check::Inconclusive(Incomplete::ReductStorage { required, limit: observed }) if required == retained && observed == u128::from(limit))
    );
    assert_eq!(receipt.retained_bytes, retained);
    assert_eq!(receipt.statistics.reduct.original_work, 0);
    assert!(matches!(
        prepared
            .check(
                &current,
                &mut workspace,
                Limits {
                    max_reduct_bytes: limit + 1,
                    ..Limits::default()
                },
                &control
            )
            .0,
        Check::NonMinimal(_)
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn prepared_network_matches_fresh_and_exhaustive_reducts(
        operations in prop::collection::vec((0u8..3, any::<u8>(), any::<u8>()), 0..10),
        roots in prop::collection::vec(any::<u8>(), 0..5),
    ) {
        let mut nodes = vec![Node::Atom(0), Node::Atom(1), Node::False];
        for (kind, left, right) in operations {
            let left = usize::from(left) % nodes.len();
            let right = usize::from(right) % nodes.len();
            nodes.push(match kind { 0 => Node::And(left, right), 1 => Node::Or(left, right), _ => Node::Implies(left, right) });
        }
        let roots = roots.into_iter().map(|root| usize::from(root) % nodes.len()).collect();
        compare(&theory(2, nodes, roots));
    }
}
