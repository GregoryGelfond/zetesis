//! Independent threshold arithmetic, frozen truth and class-certificate checks.

use crate::{
    AdmissionLimits, EvaluationError, EvaluationLimits, EvaluationWorkspace, FormulaNodes,
    FormulaTransaction, Interpretation, Limits, NodeView, PositivePlan, PositivePlanLimits,
    StratifiedError, StratifiedPlan, StratifiedPlanLimits, Theory, Threshold, TightCheckLimits,
    TightError, TightPlan, TightPlanLimits, TightVerdict,
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

fn threshold(
    transaction: &mut FormulaTransaction<'_>,
    conditions: &[usize],
    weights: &[u64],
    bound: i128,
) -> usize {
    push(
        transaction,
        NodeView::Threshold(Threshold {
            conditions,
            weights,
            bound,
        }),
    )
}

#[test]
fn original_thresholds_keep_weighted_multiplicity() {
    let weights = [2, 3, 0, u64::MAX];
    for bound in [i128::MIN, 0, 1, 5, 6, i128::from(u64::MAX) + 5, i128::MAX] {
        let owner = theory(3, |transaction| {
            vec![threshold(transaction, &[0, 0, 1, 2], &weights, bound)]
        });
        assert!(matches!(
            owner.view().node(owner.roots()[0]).unwrap(),
            NodeView::Threshold(_)
        ));
        for mask in 0..8 {
            let expected = (if mask & 1 != 0 { 5 } else { 0 })
                + if mask & 4 != 0 {
                    i128::from(u64::MAX)
                } else {
                    0
                };
            assert_eq!(
                crate::models(
                    &owner,
                    &world(&owner, mask),
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                expected >= bound,
                "mask={mask}, bound={bound}",
            );
        }
    }
}

#[test]
fn empty_thresholds_keep_the_widened_bound() {
    for bound in [i128::MIN, 0, 1, i128::MAX] {
        let owner = theory(0, |transaction| {
            vec![threshold(transaction, &[], &[], bound)]
        });
        assert_eq!(
            crate::models(
                &owner,
                &world(&owner, 0),
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap(),
            bound <= 0,
        );
    }
}

#[test]
fn frozen_thresholds_match_arbitrary_worlds() {
    for bound in -1..19 {
        let owner = theory(2, |transaction| {
            let falsum = push(transaction, NodeView::False);
            let implication = push(transaction, NodeView::Implies(0, 1));
            let negative = push(transaction, NodeView::Implies(0, falsum));
            vec![threshold(
                transaction,
                &[0, 0, 1, implication, negative],
                &[2, 3, 0, 5, 7],
                bound,
            )]
        });
        for candidate in 0..4 {
            for tested in 0..4 {
                let p = candidate & 1 != 0;
                let q = candidate & 2 != 0;
                let reduced_p = p && tested & 1 != 0;
                let reduced_q = q && tested & 2 != 0;
                let original = i128::from(p) * 5 + i128::from(!p || q) * 5 + i128::from(!p) * 7;
                let reduced = i128::from(reduced_p) * 5
                    + i128::from((!p || q) && (!reduced_p || reduced_q)) * 5
                    + i128::from(!p && !reduced_p) * 7;
                assert_eq!(
                    crate::models_reduct(
                        &owner,
                        &world(&owner, candidate),
                        &world(&owner, tested),
                        Limits::default(),
                        &Cancellation::default()
                    )
                    .unwrap(),
                    original >= bound && reduced >= bound,
                    "M={candidate}, J={tested}, bound={bound}",
                );
            }
        }
    }
}

#[test]
fn every_threshold_occurrence_consumes_work() {
    let owner = theory(3, |transaction| {
        vec![threshold(transaction, &[0, 0, 1, 2], &[2, 3, 0, 1], 1)]
    });
    let candidate = world(&owner, 7);
    // Three atoms, one threshold, four occurrences (including zero), one root,
    // one fresh measure cell and one validity word.
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
    assert!(attempt.result.unwrap().is_model());
    assert_eq!(attempt.work, 11);
}

#[test]
fn reduction_propagates_a_stop_after_decisive_truth() {
    let row = Threshold {
        conditions: &[0, 1, 2],
        weights: &[10, 0, 1],
        bound: 1,
    };
    let mut visited = Vec::new();
    let result = row.evaluate(|child| {
        visited.push(child);
        if child == 2 {
            Err(Stop::Cancelled)
        } else {
            Ok(true)
        }
    });
    assert_eq!(result, Err(Stop::Cancelled));
    assert_eq!(visited, [0, 1, 2]);
}

#[test]
fn tight_threshold_dependencies_require_strict_ranks() {
    let owner = theory(3, |transaction| {
        let body = threshold(transaction, &[0, 0, 1, 2], &[2, 3, 1, 0], 4);
        vec![0, push(transaction, NodeView::Implies(body, 2))]
    });
    let plan =
        TightPlan::compile(&owner, TightPlanLimits::default(), &Cancellation::default()).unwrap();
    assert!(plan.ranks()[2] > plan.ranks()[0]);
    assert!(matches!(
        TightPlan::certify(
            &owner,
            &[0, 0, 0],
            TightPlanLimits::default(),
            &Cancellation::default()
        ),
        Err(TightError::RankOrder { head: 2 })
    ));
    for mask in 0..8 {
        let candidate = world(&owner, mask);
        let result = plan
            .check(
                &candidate,
                TightCheckLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(result.verdict == TightVerdict::Stable, mask == 5);
    }
}

#[test]
fn positive_threshold_cycles_need_external_consequences() {
    for facts in 0..4 {
        let owner = theory(3, |transaction| {
            let body = threshold(transaction, &[0, 0, 1, 2], &[2, 3, 4, 0], 7);
            let mut roots = vec![
                push(transaction, NodeView::Implies(body, 2)),
                push(transaction, NodeView::Implies(2, 0)),
            ];
            roots.extend((0..2).filter(|atom| facts & (1 << atom) != 0));
            roots
        });
        let plan = PositivePlan::compile(
            &owner,
            PositivePlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let expected = if facts == 3 { 7 } else { facts };
        assert_eq!(
            plan.least_consequences().atoms().collect::<Vec<_>>(),
            world(&owner, expected).atoms().collect::<Vec<_>>()
        );
        for mask in 0..8 {
            assert_eq!(
                crate::check(
                    &owner,
                    &world(&owner, mask),
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap()
                .accepted(),
                mask == expected
            );
        }
    }
}

#[test]
fn stratified_conjunction_thresholds_preserve_negation() {
    for fact in [false, true] {
        let owner = theory(3, |transaction| {
            let falsum = push(transaction, NodeView::False);
            let singleton = threshold(transaction, &[0, 0, 1], &[2, 3, 0], 4);
            let negative = push(transaction, NodeView::Implies(singleton, falsum));
            let truth = threshold(transaction, &[], &[], 0);
            let conjunction = threshold(transaction, &[negative, truth], &[3, 2], 4);
            let mut roots = vec![push(transaction, NodeView::Implies(conjunction, 2))];
            if fact {
                roots.push(0);
            }
            roots
        });
        let plan = StratifiedPlan::compile(
            &owner,
            StratifiedPlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let expected = if fact { 1 } else { 4 };
        assert_eq!(
            plan.consequences().atoms().collect::<Vec<_>>(),
            world(&owner, expected).atoms().collect::<Vec<_>>()
        );
    }
}

#[test]
fn stratified_general_thresholds_decline_the_certificate() {
    let owner = theory(3, |transaction| {
        let body = threshold(transaction, &[0, 1], &[2, 3], 2);
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
