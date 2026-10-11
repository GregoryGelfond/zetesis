use crate::{
    AdmissionLimits, EvaluationError, EvaluationLimits, EvaluationWorkspace, ExtremumCut,
    ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaTransaction, Interpretation, Limits,
    NodeView, PositiveError, PositivePlan, PositivePlanLimits, PositiveResource, StratifiedError,
    StratifiedPlan, StratifiedPlanLimits, SupportLimits, Theory, TightCheckLimits, TightPlan,
    TightPlanLimits, TightVerdict, models, models_reduct, support_restriction,
};
use zetesis_cpu::{Cancellation, Stop};

fn push(transaction: &mut FormulaTransaction<'_>, node: NodeView<'_>) -> usize {
    transaction.push(node, usize::MAX, usize::MAX).unwrap()
}

fn theory(
    atoms: usize,
    construct: impl FnOnce(&mut FormulaTransaction<'_>) -> Vec<usize>,
) -> Theory {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..atoms {
        push(&mut transaction, NodeView::Atom(atom));
    }
    let roots = construct(&mut transaction);
    transaction.commit();
    Theory::new(atoms, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

fn world(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

// Independent witness expansion, used only as the small semantic test oracle.
// The native branch must retain its ordinal row rather than calling this branch.
fn cut(
    transaction: &mut FormulaTransaction<'_>,
    native: bool,
    row: ExtremumRow<'_>,
    rule: ExtremumCutRule,
) -> usize {
    if native {
        return push(
            transaction,
            NodeView::ExtremumCut(ExtremumCut { row, rule: &rule }),
        );
    }
    let truth = match rule {
        ExtremumCutRule::Prefix(0) => Some(false),
        ExtremumCutRule::Prefix(boundary) if boundary == row.rank_count => Some(true),
        ExtremumCutRule::Suffix(0) => Some(true),
        ExtremumCutRule::Suffix(boundary) if boundary == row.rank_count => Some(false),
        _ => None,
    };
    if let Some(truth) = truth {
        let falsum = push(transaction, NodeView::False);
        return if truth {
            push(transaction, NodeView::Implies(falsum, falsum))
        } else {
            falsum
        };
    }
    let witnesses: Vec<_> = row
        .conditions
        .iter()
        .zip(row.ranks)
        .filter_map(|(&child, &rank)| {
            match rule {
                ExtremumCutRule::Prefix(boundary) => rank < boundary,
                ExtremumCutRule::Suffix(boundary) => rank >= boundary,
            }
            .then_some(child)
        })
        .collect();
    push(transaction, NodeView::Or(&witnesses))
}

fn nested(native: bool, rule: ExtremumCutRule, empty: bool) -> Theory {
    theory(3, |transaction| {
        let falsum = push(transaction, NodeView::False);
        let implication = push(transaction, NodeView::Implies(0, 1));
        let negative = push(transaction, NodeView::Implies(2, falsum));
        let conditions = [0, implication, negative, 0, 2];
        let ranks = [0, 2, 4, 3, 1];
        let inner = cut(
            transaction,
            native,
            ExtremumRow {
                conditions: if empty { &[] } else { &conditions },
                ranks: if empty { &[] } else { &ranks },
                rank_count: 5,
            },
            rule,
        );
        let outer = cut(
            transaction,
            native,
            ExtremumRow {
                conditions: &[inner, negative, 1],
                ranks: &[1, 3, 0],
                rank_count: 5,
            },
            ExtremumCutRule::Suffix(2),
        );
        // Keep the implication that participates in extrema not-equal emission.
        vec![push(transaction, NodeView::Implies(inner, outer))]
    })
}

#[test]
fn nested_cuts_match_witnesses_in_arbitrary_frozen_worlds() {
    for empty in [false, true] {
        for boundary in 0..=5 {
            for rule in [
                ExtremumCutRule::Prefix(boundary),
                ExtremumCutRule::Suffix(boundary),
            ] {
                let native = nested(true, rule, empty);
                let boolean = nested(false, rule, empty);
                assert_eq!(
                    (0..native.nodes().len())
                        .filter(|&index| matches!(
                            native.view().node(index).unwrap(),
                            NodeView::ExtremumCut(_)
                        ))
                        .count(),
                    2
                );
                for outer in 0..8 {
                    let left = world(&native, outer);
                    let right = world(&boolean, outer);
                    assert_eq!(
                        models(&native, &left, Limits::default(), &Cancellation::default())
                            .unwrap(),
                        models(
                            &boolean,
                            &right,
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap()
                    );
                    for inner in 0..8 {
                        assert_eq!(
                            models_reduct(
                                &native,
                                &left,
                                &world(&native, inner),
                                Limits::default(),
                                &Cancellation::default()
                            )
                            .unwrap(),
                            models_reduct(
                                &boolean,
                                &right,
                                &world(&boolean, inner),
                                Limits::default(),
                                &Cancellation::default()
                            )
                            .unwrap(),
                            "{rule:?} empty={empty} M={outer} J={inner}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn original_cut_truth_preserves_full_width_carriers() {
    for rank_count in [2, 5, u64::MAX] {
        for boundary in [0, 1, rank_count - 1, rank_count] {
            for rule in [
                ExtremumCutRule::Prefix(boundary),
                ExtremumCutRule::Suffix(boundary),
            ] {
                let owner = theory(2, |transaction| {
                    vec![cut(
                        transaction,
                        true,
                        ExtremumRow {
                            conditions: &[0, 1, 0],
                            ranks: &[0, rank_count - 1, rank_count - 1],
                            rank_count,
                        },
                        rule,
                    )]
                });
                for mask in 0..4 {
                    let expected = match rule {
                        ExtremumCutRule::Prefix(cut) => {
                            if mask & 1 == 0 {
                                rank_count - 1 < cut
                            } else {
                                0 < cut
                            }
                        }
                        ExtremumCutRule::Suffix(cut) => {
                            if mask == 0 {
                                cut == 0
                            } else {
                                rank_count > cut
                            }
                        }
                    };
                    assert_eq!(
                        models(
                            &owner,
                            &world(&owner, mask),
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap(),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn endpoint_cuts_charge_every_physical_occurrence() {
    for rule in [
        ExtremumCutRule::Prefix(0),
        ExtremumCutRule::Prefix(5),
        ExtremumCutRule::Suffix(0),
        ExtremumCutRule::Suffix(5),
    ] {
        let owner = theory(3, |transaction| {
            vec![cut(
                transaction,
                true,
                ExtremumRow {
                    conditions: &[0, 0, 1, 2],
                    ranks: &[0, 4, 2, 1],
                    rank_count: 5,
                },
                rule,
            )]
        });
        let candidate = world(&owner, 7);
        // Three atoms, one cut, four physical occurrences, one asserted root,
        // plus one validity reset and one new measure initialization.
        for max_work in 0..11 {
            let mut workspace = EvaluationWorkspace::default();
            let attempt = workspace.evaluate(
                &candidate,
                EvaluationLimits {
                    max_work,
                    ..EvaluationLimits::default()
                },
                &Cancellation::default(),
            );
            assert_eq!(
                attempt.result.unwrap_err(),
                EvaluationError::Stopped(Stop::WorkLimit)
            );
            assert_eq!(attempt.work, max_work);
        }
        let mut workspace = EvaluationWorkspace::default();
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits {
                max_work: 11,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
        );
        assert!(attempt.result.is_ok());
        assert_eq!(attempt.work, 11);
    }
}

#[test]
fn tight_ranks_ignore_nonqualifying_self_edges() {
    for rule in [ExtremumCutRule::Prefix(2), ExtremumCutRule::Suffix(3)] {
        let owner = theory(2, |transaction| {
            let opaque = push(transaction, NodeView::Implies(1, 0));
            let ranks = match rule {
                ExtremumCutRule::Prefix(_) => [1, 4, 4],
                ExtremumCutRule::Suffix(_) => [4, 1, 1],
            };
            let body = cut(
                transaction,
                true,
                ExtremumRow {
                    conditions: &[0, 1, opaque],
                    ranks: &ranks,
                    rank_count: 5,
                },
                rule,
            );
            vec![0, push(transaction, NodeView::Implies(body, 1))]
        });
        let plan = TightPlan::compile(&owner, TightPlanLimits::default(), &Cancellation::default())
            .unwrap();
        assert!(plan.ranks()[1] > plan.ranks()[0]);
        for mask in 0..4 {
            let checked = plan
                .check(
                    &world(&owner, mask),
                    TightCheckLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(checked.verdict == TightVerdict::Stable, mask == 3);
        }
    }
}

#[test]
fn endpoint_cut_bodies_ignore_opaque_children() {
    for rule in [
        ExtremumCutRule::Prefix(0),
        ExtremumCutRule::Prefix(5),
        ExtremumCutRule::Suffix(0),
        ExtremumCutRule::Suffix(5),
    ] {
        let expected = matches!(
            rule,
            ExtremumCutRule::Prefix(5) | ExtremumCutRule::Suffix(0)
        );
        let owner = theory(2, |transaction| {
            let opaque = push(transaction, NodeView::Implies(0, 1));
            let body = cut(
                transaction,
                true,
                ExtremumRow {
                    conditions: &[1, opaque],
                    ranks: &[0, 4],
                    rank_count: 5,
                },
                rule,
            );
            vec![push(transaction, NodeView::Implies(body, 1))]
        });
        let positive = PositivePlan::compile(
            &owner,
            PositivePlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let stratified = StratifiedPlan::compile(
            &owner,
            StratifiedPlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(positive.least_consequences().contains(1), expected);
        assert_eq!(stratified.consequences().contains(1), expected);
        let tight =
            TightPlan::compile(&owner, TightPlanLimits::default(), &Cancellation::default())
                .unwrap();
        let mask = if expected { 2 } else { 0 };
        assert_eq!(
            tight
                .check(
                    &world(&owner, mask),
                    TightCheckLimits::default(),
                    &Cancellation::default()
                )
                .unwrap()
                .verdict,
            TightVerdict::Stable
        );
    }
}

fn positive_cycle(fact: bool) -> Theory {
    theory(3, |transaction| {
        let opaque = push(transaction, NodeView::Implies(2, 0));
        let body = cut(
            transaction,
            true,
            ExtremumRow {
                conditions: &[0, 0, opaque],
                ranks: &[1, 2, 4],
                rank_count: 5,
            },
            ExtremumCutRule::Prefix(3),
        );
        let mut roots = vec![
            push(transaction, NodeView::Implies(body, 1)),
            push(transaction, NodeView::Implies(1, 0)),
        ];
        if fact {
            roots.push(0);
        }
        roots
    })
}

#[test]
fn positive_cut_cycles_need_external_support() {
    for fact in [false, true] {
        let owner = positive_cycle(fact);
        let plan = PositivePlan::compile(
            &owner,
            PositivePlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            plan.least_consequences().atoms().collect::<Vec<_>>(),
            if fact { vec![0, 1] } else { vec![] }
        );
        assert!(plan.theory().same_instance(&owner));
    }
}

#[test]
fn positive_cut_work_refusals_keep_charged_prefixes() {
    let owner = positive_cycle(true);
    let complete = PositivePlan::compile(
        &owner,
        PositivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    for max_work in 0..complete.statistics().work {
        let attempt = PositivePlan::compile_accounted(
            &owner,
            PositivePlanLimits {
                max_work,
                ..PositivePlanLimits::default()
            },
            &Cancellation::default(),
        );
        assert!(matches!(
            attempt.result,
            Err(PositiveError::Limit {
                resource: PositiveResource::Work,
                ..
            })
        ));
        assert_eq!(attempt.statistics.work, max_work);
    }
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let attempt =
        PositivePlan::compile_accounted(&owner, PositivePlanLimits::default(), &cancellation);
    assert!(matches!(
        attempt.result,
        Err(PositiveError::Stopped(Stop::Cancelled))
    ));
    assert_eq!(attempt.statistics.work, 0);
}

#[test]
fn stratified_cut_aliases_keep_default_negation() {
    for fact in [false, true] {
        let owner = theory(2, |transaction| {
            let falsum = push(transaction, NodeView::False);
            let negative = push(transaction, NodeView::Implies(1, falsum));
            let opaque = push(transaction, NodeView::Implies(0, 1));
            let body = cut(
                transaction,
                true,
                ExtremumRow {
                    conditions: &[negative, negative, opaque],
                    ranks: &[1, 2, 4],
                    rank_count: 5,
                },
                ExtremumCutRule::Prefix(3),
            );
            let mut roots = vec![push(transaction, NodeView::Implies(body, 0))];
            if fact {
                roots.push(1);
            }
            roots
        });
        let plan = StratifiedPlan::compile(
            &owner,
            StratifiedPlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            plan.consequences().atoms().collect::<Vec<_>>(),
            if fact { vec![1] } else { vec![0] }
        );
    }
}

#[test]
fn stratified_distinct_cut_witnesses_decline() {
    let owner = theory(3, |transaction| {
        let body = cut(
            transaction,
            true,
            ExtremumRow {
                conditions: &[0, 1],
                ranks: &[1, 2],
                rank_count: 5,
            },
            ExtremumCutRule::Prefix(3),
        );
        vec![push(transaction, NodeView::Implies(body, 2))]
    });
    assert!(matches!(
        StratifiedPlan::compile(
            &owner,
            StratifiedPlanLimits::default(),
            &Cancellation::default()
        ),
        Err(StratifiedError::UnsupportedBody { .. })
    ));
}

fn support(native: bool) -> Theory {
    let owner = theory(3, |transaction| {
        let opaque = push(transaction, NodeView::Implies(2, 0));
        vec![cut(
            transaction,
            native,
            ExtremumRow {
                conditions: &[0, 1, 0, opaque],
                ranks: &[1, 2, 1, 4],
                rank_count: 5,
            },
            ExtremumCutRule::Prefix(3),
        )]
    });
    support_restriction(
        &owner,
        SupportLimits {
            admission: AdmissionLimits::default(),
            max_work: u64::MAX,
        },
        &Cancellation::default(),
    )
    .result
    .unwrap()
    .unwrap()
}

#[test]
fn support_heads_use_only_qualifying_distinct_atoms() {
    let native = support(true);
    let boolean = support(false);
    for mask in 0..8 {
        let left = models(
            &native,
            &world(&native, mask),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let right = models(
            &boolean,
            &world(&boolean, mask),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(left, right);
        assert_eq!(left, mask & 4 == 0 && mask != 3);
    }
}
