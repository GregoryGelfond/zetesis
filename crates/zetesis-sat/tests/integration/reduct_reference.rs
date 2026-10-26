//! Membership by the frozen reduct's region query agrees with the exhaustive
//! reference on every candidate of small theories, native aggregate nodes
//! among them, and every countermodel it returns is a validated proper subset.

use proptest::prelude::*;
use zetesis_ferraris::{Node, Theory, Verdict};
use zetesis_sat::{Cancellation, Check, Incomplete, Limits, StableModels};
use zetesis_theory_support::theories::{interpretation as candidate, theory};

fn compare(input: &Theory) {
    let cancellation = Cancellation::default();
    for mask in 0..1 << input.atom_count() {
        let current = candidate(input, mask);
        let actual = zetesis_sat::check(input, &current, Limits::default(), &cancellation);
        let exhaustive = zetesis_ferraris::check(
            input,
            &current,
            zetesis_ferraris::Limits::default(),
            &cancellation,
        )
        .unwrap();
        assert_eq!(
            actual.accepted(),
            exhaustive.accepted(),
            "exhaustive: {mask}: {actual:?}"
        );
        match actual {
            Check::Stable => assert!(matches!(exhaustive.verdict(), Verdict::Stable)),
            Check::NotModel => {
                assert!(matches!(exhaustive.verdict(), Verdict::NotModel { .. }));
            }
            Check::NonMinimal(witness) => {
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
                        &cancellation
                    )
                    .unwrap()
                );
            }
            Check::Inconclusive(error) => panic!("complete small query: {error}"),
        }
    }
}

#[test]
fn false_original_implication_does_not_retain_classical_equivalence() {
    // At M={a}, (not a -> a) freezes to (bottom -> a). Empty is a
    // countermodel; retaining the old not-a equivalence would lose it.
    let input = theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::implies(2, 0),
        ],
        vec![3],
    );
    let result = zetesis_sat::check(
        &input,
        &candidate(&input, 1),
        Limits::default(),
        &Cancellation::default(),
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
    compare(&theory(0, vec![Node::falsum()], vec![0]));
}

#[test]
fn choice_reducts_preserve_the_complete_family() {
    compare(&theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::or_pair([0, 3]),
            Node::implies(1, 2),
            Node::or_pair([1, 5]),
        ],
        vec![4, 6],
    ));
}

#[test]
fn original_evaluation_stop_never_enters_subset_search() {
    let input = theory(1, vec![Node::atom(0)], vec![0]);
    let mut search = StableModels::new(
        &input,
        Limits {
            max_verification_work: 1,
            ..Default::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert!(matches!(
        search.next(),
        Some(Err(Incomplete::Verification(zetesis_cpu::Stop::WorkLimit)))
    ));
    assert_eq!(search.statistics().reduct.original_work, 1);
    assert_eq!(search.statistics().countermodel_queries, 0);
}

#[test]
fn cancellation_precedes_query_storage_admission() {
    let input = theory(1, vec![Node::atom(0)], vec![0]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let result = zetesis_sat::check(
        &input,
        &candidate(&input, 1),
        Limits {
            max_reduct_bytes: 0,
            ..Limits::default()
        },
        &cancellation,
    );
    assert!(matches!(result, Check::Inconclusive(Incomplete::Cancelled)));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn region_queries_match_the_exhaustive_reduct_on_formula_networks(
        operations in prop::collection::vec((0u8..3, any::<u8>(), any::<u8>()), 0..10),
        roots in prop::collection::vec(any::<u8>(), 0..5),
    ) {
        let mut nodes = vec![Node::atom(0), Node::atom(1), Node::falsum()];
        for (kind, left, right) in operations {
            let left = usize::from(left) % nodes.len();
            let right = usize::from(right) % nodes.len();
            nodes.push(match kind { 0 => Node::and_pair([left, right]), 1 => Node::or_pair([left, right]), _ => Node::implies(left, right) });
        }
        let roots = roots.into_iter().map(|root| usize::from(root) % nodes.len()).collect();
        compare(&theory(2, nodes, roots));
    }
}

#[test]
fn region_queries_preserve_native_wide_duplicate_groups() {
    for width in [3, 65, 129] {
        let input = Theory::new(
            2,
            zetesis_ferraris::FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::falsum(),
                    Node::implies(0, 2),
                    Node::or_span(zetesis_ferraris::OperandSpan {
                        start: 0,
                        length: width,
                    }),
                    Node::and_span(zetesis_ferraris::OperandSpan {
                        start: width,
                        length: width,
                    }),
                    Node::implies(5, 1),
                ],
                (0..width)
                    .map(|index| if index % 2 == 0 { 0 } else { 3 })
                    .chain((0..width).map(|index| if index % 2 == 0 { 4 } else { 1 }))
                    .collect(),
            )
            .unwrap(),
            vec![4, 6],
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap();
        assert_eq!(input.nodes().len(), 7);
        assert_eq!(input.operands().len(), 2 * width);
        compare(&input);
    }
}

#[test]
fn native_threshold_queries_match_the_exhaustive_reduct() {
    use zetesis_ferraris::{FormulaNodes, FormulaParts, NodeView};
    for bound in -1..=8 {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::falsum(),
                    Node::implies(0, 2),
                    Node::implies(3, 1),
                ],
                vec![],
            )
            .unwrap(),
        );
        let mut transaction = nodes.transaction();
        let guards = transaction
            .push_threshold_family(
                &[0, 0, 1, 3, 4],
                &[1, 2, 0, 2, 1],
                &[bound, bound + 1],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        // The nonconvex != wrapper exercises both guarded outputs and nested
        // implication conditions while retaining repeated-position weights.
        let root = transaction
            .push(
                NodeView::Implies(guards[0], guards[1]),
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        transaction.commit();
        let input = Theory::new(
            2,
            nodes.into_parts(),
            vec![root],
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap();
        compare(&input);
    }
}

#[test]
fn ordinal_cut_comparisons_match_the_exhaustive_reduct() {
    use zetesis_ferraris::{ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts, NodeView};
    for empty in [false, true] {
        for prefix in [false, true] {
            for boundary in 0..5 {
                for wrapper in 0..6 {
                    let mut nodes = FormulaNodes::new(
                        FormulaParts::new(
                            vec![
                                Node::atom(0),
                                Node::atom(1),
                                Node::falsum(),
                                Node::implies(0, 2),
                                Node::implies(3, 1),
                            ],
                            vec![],
                        )
                        .unwrap(),
                    );
                    let mut transaction = nodes.transaction();
                    let rule = |bound| {
                        if prefix {
                            ExtremumCutRule::Prefix(bound)
                        } else {
                            ExtremumCutRule::Suffix(bound)
                        }
                    };
                    let (conditions, ranks): (&[usize], &[u64]) = if empty {
                        (&[], &[])
                    } else {
                        (&[0, 0, 1, 3, 4], &[0, 4, 2, 3, 1])
                    };
                    let cuts = transaction
                        .push_extremum_cut_family_range(
                            ExtremumRow {
                                conditions,
                                ranks,
                                rank_count: 5,
                            },
                            &[rule(boundary), rule(boundary + 1)],
                            usize::MAX,
                            usize::MAX,
                        )
                        .unwrap();
                    // Ordered G/H cuts plus the existing Eq/Ne outer grammar
                    // cover all six comparison polarities for either extremum.
                    let (g, h) = if prefix {
                        (cuts.start + 1, cuts.start)
                    } else {
                        (cuts.start, cuts.start + 1)
                    };
                    let root = match wrapper {
                        0 => g,
                        1 => h,
                        2 => transaction
                            .push(NodeView::Implies(g, 2), usize::MAX, usize::MAX)
                            .unwrap(),
                        3 => transaction
                            .push(NodeView::Implies(h, 2), usize::MAX, usize::MAX)
                            .unwrap(),
                        4 => {
                            let negative = transaction
                                .push(NodeView::Implies(h, 2), usize::MAX, usize::MAX)
                                .unwrap();
                            transaction
                                .push(NodeView::And(&[g, negative]), usize::MAX, usize::MAX)
                                .unwrap()
                        }
                        _ => transaction
                            .push(NodeView::Implies(g, h), usize::MAX, usize::MAX)
                            .unwrap(),
                    };
                    transaction.commit();
                    let input = Theory::new(
                        2,
                        nodes.into_parts(),
                        vec![root],
                        zetesis_ferraris::AdmissionLimits::default(),
                    )
                    .unwrap();
                    compare(&input);
                }
            }
        }
    }
}
