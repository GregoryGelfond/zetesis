//! Original-only necessary-cost evidence for native ordinal witnesses.
use super::*;
use std::cmp::Ordering;
use zetesis_core::{Term, Value};
use zetesis_ferraris::{ExtremumCut, ExtremumCutRule, ExtremumRow};
use zetesis_objective::ObjectiveTemplate;

fn fixture(native: bool) -> (ObjectivePlan, crate::RequiredChoices) {
    let mut original = formula_nodes();
    let mut transaction = original.transaction();
    let falsum = push(&mut transaction, NodeView::False);
    let truth = push(&mut transaction, NodeView::Implies(falsum, falsum));
    let choice = push(&mut transaction, NodeView::Or(&[0, 1]));
    transaction.commit();
    let original = Theory::new(
        3,
        original.into_parts(),
        vec![choice, 2],
        AdmissionLimits::default(),
    )
    .unwrap();
    // Internal fixture premises are independently entailed by the asserted
    // original disjunction; no public source-certificate constructor is added.
    let required = crate::RequiredChoices {
        original: original.clone(),
        groups: vec![crate::formula_count_plan::RequiredGroup {
            body: truth,
            unconditional_members: true,
            members: vec![0, 1],
            lower: 1,
            upper: 2,
            bound_root: choice,
            asserted_root: choice,
            origins: vec![],
            location: crate::ProgramSite::program(),
        }],
        capture: crate::formula_count_plan::CountPlanStatistics::default(),
        failure: None,
    };
    let mut nodes = formula_nodes();
    let mut transaction = nodes.transaction();
    let both = push(&mut transaction, NodeView::And(&[0, 2]));
    let falsum = push(&mut transaction, NodeView::False);
    let (inclusive, strict) = if native {
        let nodes = transaction
            .push_extremum_cut_family_range(
                ExtremumRow {
                    conditions: &[both, 1, both, 0],
                    ranks: &[1, 1, 1, 3],
                    rank_count: 4,
                },
                &[ExtremumCutRule::Prefix(2), ExtremumCutRule::Prefix(1)],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        (nodes.start, nodes.start + 1)
    } else {
        (
            push(&mut transaction, NodeView::Or(&[both, 1, both])),
            falsum,
        )
    };
    let no_strict = push(&mut transaction, NodeView::Implies(strict, falsum));
    let equality = push(&mut transaction, NodeView::And(&[inclusive, no_strict]));
    transaction.commit();
    let objectives = ObjectiveProgram::new(
        vec![ObjectiveTemplate::new(
            Term::Constant(Value::Number(1)),
            0,
            vec![],
            vec![],
            vec![],
        )],
        zetesis_objective::AdmissionLimits::default(),
    )
    .unwrap();
    let plan = ObjectivePlan {
        original,
        objectives,
        nodes: nodes.into_parts(),
        levels: BTreeMap::from([(
            0,
            vec![
                AggregateElement {
                    weight: 5,
                    condition: equality,
                },
                AggregateElement {
                    weight: 3,
                    condition: 0,
                },
            ],
        )]),
        choices: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    (plan, required)
}

fn candidate(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(theory, (0..3).filter(|atom| mask & (1 << atom) != 0)).unwrap()
}
fn holds(theory: &Theory, mask: usize) -> bool {
    models(
        theory,
        &candidate(theory, mask),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}
fn score(plan: &ObjectivePlan, mask: usize) -> Score {
    plan.score(
        &candidate(plan.original(), mask),
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .unwrap()
    .into_score()
}

#[test]
fn complete_cost_and_optimum_parity_keeps_duplicate_composite_witness_proofs() {
    let mut all_costs = Vec::new();
    for native in [false, true] {
        let (mut plan, required) = fixture(native);
        plan.prepare_choice_bounds(
            &required,
            ObjectivePlanLimits::default(),
            u128::MAX,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            plan.choice_bound_groups(),
            1,
            "necessary-cost evidence must survive"
        );
        let masks: Vec<_> = (0..8)
            .filter(|&mask| holds(plan.original(), mask))
            .collect();
        assert_eq!(masks, [5, 6, 7]);
        let scores: Vec<_> = masks.iter().map(|&mask| score(&plan, mask)).collect();
        assert_eq!(scores.iter().map(|score| score.costs()[0].1).min(), Some(5));
        for incumbent in &scores {
            let bound = plan
                .bound(
                    incumbent,
                    ObjectiveBoundLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert!(bound.choice_failure().is_none());
            for (&mask, score) in masks.iter().zip(&scores) {
                assert_eq!(
                    holds(bound.theory(), mask),
                    score.compare_costs(incumbent) != Ordering::Greater
                );
            }
        }
        all_costs.push(
            scores
                .iter()
                .map(|score| score.costs().to_vec())
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(all_costs[0], all_costs[1]);
}

#[test]
fn witness_cursor_charges_each_physical_occurrence_once() {
    let (plan, _) = fixture(true);
    let cancellation = Cancellation::default();
    let mut work = Work {
        limits: ObjectivePlanLimits::default(),
        cancellation: &cancellation,
        template: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    let mut memory = Storage {
        bytes: 0,
        limit: u128::MAX,
    };
    let graph = Implications::new(&plan, &mut memory, &mut work).unwrap();
    let cut = (0..plan.nodes.view().len())
        .find(|&node| {
            matches!(
                plan.nodes.view().node(node).unwrap(),
                NodeView::ExtremumCut(_)
            )
        })
        .unwrap();
    let start = work.statistics.work;
    let mut cursor = 0;
    let mut children = Vec::new();
    while let Some(child) = graph
        .child(graph.objective + cut, &mut cursor, &mut work)
        .unwrap()
    {
        children.push(child - graph.objective);
    }
    assert_eq!(children, [3, 1, 3]);
    assert_eq!(cursor, 4);
    assert_eq!(work.statistics.work - start, 4);
    work.limits.max_work = work.statistics.work + 2;
    let mut cursor = 0;
    assert!(
        graph
            .child(graph.objective + cut, &mut cursor, &mut work)
            .unwrap()
            .is_some()
    );
    assert!(
        graph
            .child(graph.objective + cut, &mut cursor, &mut work)
            .unwrap()
            .is_some()
    );
    assert!(
        graph
            .child(graph.objective + cut, &mut cursor, &mut work)
            .is_err()
    );
    assert_eq!(
        cursor, 2,
        "refusal must not consume the uncharged occurrence"
    );
}

#[test]
fn assumed_cut_forces_only_a_unique_distinct_composite_witness() {
    let cancellation = Cancellation::default();
    for (ranks, rule, expected) in [
        ([1, 3, 1], ExtremumCutRule::Prefix(2), Some(3)),
        ([1, 1, 1], ExtremumCutRule::Prefix(2), None),
        ([1, 3, 1], ExtremumCutRule::Prefix(4), None),
        ([1, 3, 1], ExtremumCutRule::Prefix(0), None),
        ([3, 1, 3], ExtremumCutRule::Suffix(2), Some(3)),
    ] {
        let mut work = Work {
            limits: ObjectivePlanLimits::default(),
            cancellation: &cancellation,
            template: None,
            statistics: ObjectiveBoundStatistics::default(),
        };
        let cut = ExtremumCut {
            row: ExtremumRow {
                conditions: &[3, 0, 3],
                ranks: &ranks,
                rank_count: 4,
            },
            rule: &rule,
        };
        assert_eq!(unique_witness(cut, &mut work).unwrap(), expected);
    }
}

#[test]
fn endpoint_truth_never_requires_a_qualifying_child() {
    let (mut plan, required) = fixture(true);
    let mut nodes = FormulaNodes::new(plan.nodes);
    let mut transaction = nodes.transaction();
    let cuts = transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[],
                ranks: &[],
                rank_count: 2,
            },
            &[
                ExtremumCutRule::Prefix(0),
                ExtremumCutRule::Prefix(2),
                ExtremumCutRule::Suffix(0),
                ExtremumCutRule::Suffix(2),
            ],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    transaction.commit();
    plan.nodes = nodes.into_parts();
    let cancellation = Cancellation::default();
    let mut work = Work {
        limits: ObjectivePlanLimits::default(),
        cancellation: &cancellation,
        template: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    let mut memory = Storage {
        bytes: 0,
        limit: u128::MAX,
    };
    let graph = Implications::new(&plan, &mut memory, &mut work).unwrap();
    let mut proof = Proof::new(&graph, &mut memory, &mut work).unwrap();
    for (cut, expected) in cuts.zip([false, true, true, false]) {
        proof
            .assume(&graph, required.groups[0].body, 0, &mut work)
            .unwrap();
        assert_eq!(
            proof
                .proves(&graph, graph.objective + cut, &mut work)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn true_native_activation_transports_a_unique_composite_child() {
    for (ranks, boundary, forced) in [
        ([1, 3, 1], 2, true),
        ([1, 1, 1], 2, false),
        ([1, 3, 1], 4, false),
    ] {
        let (mut plan, _) = fixture(true);
        let mut original = formula_nodes();
        let mut transaction = original.transaction();
        let both = push(&mut transaction, NodeView::And(&[0, 1]));
        let activation = push(
            &mut transaction,
            NodeView::ExtremumCut(ExtremumCut {
                row: ExtremumRow {
                    conditions: &[both, 2, both],
                    ranks: &ranks,
                    rank_count: 4,
                },
                rule: &ExtremumCutRule::Prefix(boundary),
            }),
        );
        transaction.commit();
        plan.original =
            Theory::new(3, original.into_parts(), vec![], AdmissionLimits::default()).unwrap();
        let cancellation = Cancellation::default();
        let mut work = Work {
            limits: ObjectivePlanLimits::default(),
            cancellation: &cancellation,
            template: None,
            statistics: ObjectiveBoundStatistics::default(),
        };
        let mut memory = Storage {
            bytes: 0,
            limit: u128::MAX,
        };
        let graph = Implications::new(&plan, &mut memory, &mut work).unwrap();
        let mut proof = Proof::new(&graph, &mut memory, &mut work).unwrap();
        proof.assume(&graph, activation, 2, &mut work).unwrap();
        for atom in 0..2 {
            assert_eq!(
                proof.proves(&graph, graph.atoms + atom, &mut work).unwrap(),
                forced
            );
        }
    }
}
