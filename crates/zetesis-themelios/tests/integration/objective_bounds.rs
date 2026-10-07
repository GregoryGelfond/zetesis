//! Exact optional cost guards are checked against independent objective evaluation.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use zetesis_core::{Model, Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, models};
use zetesis_objective::{ObjectiveProgram, ObjectiveTemplate, Score, evaluate};
use zetesis_reference_support::formula;
use zetesis_sat::StableModels;
use zetesis_themelios::AdmittedFormula;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundLimits, ObjectiveBoundResource, ObjectivePlan,
    ObjectivePlanLimits,
};

fn score(input: &AdmittedFormula, mask: usize) -> Score {
    let model = Model::from_positions(
        input.atom_catalog(),
        (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
    )
    .unwrap();
    evaluate(
        input.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .expect("independent score")
    .score()
    .clone()
}
fn plan(input: &AdmittedFormula) -> ObjectivePlan {
    ObjectivePlan::new(
        input.theory(),
        input.atoms(),
        input.objectives(),
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .expect("complete eligibility plan")
}
fn compare_all(source: &str) {
    let input = formula(source);
    assert!(input.atoms().len() <= 8);
    let plan = plan(&input);
    let scores: Vec<_> = (0..1 << input.atoms().len())
        .map(|mask| score(&input, mask))
        .collect();
    let mut incumbents = Vec::new();
    for incumbent in &scores {
        if !incumbents.contains(&incumbent) {
            incumbents.push(incumbent);
        }
    }
    for incumbent in incumbents {
        let bound = plan
            .bound(
                incumbent,
                ObjectiveBoundLimits::default(),
                &Cancellation::default(),
            )
            .expect("exact optional bound");
        assert!(bound.original().same_instance(input.theory()));
        assert!(!bound.theory().same_instance(input.theory()));
        for (mask, candidate_score) in scores.iter().enumerate() {
            let candidate = Interpretation::new(
                bound.theory(),
                (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
            )
            .expect("same atom IDs");
            let actual = models(
                bound.theory(),
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .expect("complete truth evaluation");
            assert_eq!(
                actual,
                candidate_score.compare_costs(incumbent) != Ordering::Greater,
                "{source}; mask={mask}; incumbent={:?}",
                incumbent.costs()
            );
        }
    }
}

#[test]
fn all_candidate_interpretations_match_full_key_scores_and_lexicographic_ties() {
    for source in [
        "{z;a;b}.#minimize{2@9,k:a;2@9,k:b;1@0,x:z}.",
        "{a;b;c}.#minimize{-3@2,k:a;2@2,k:b;1@1,k:c}.",
        "{a;b}.#minimize{2@0,k:a;2@0,k:b}.#minimize{2@0,k:a;2@0,other:a;-2@0,k:a}.",
        "{p(1);p(2);q(1);q(2)}.#minimize{X@0,X:p(X),q(X);3@1,X:p(X),X!=2}.",
        "{p(1,2);p(2,2);p(1,1)}.#minimize{1@0,X:p(X,X)}.",
        "{p(\"a b\");p(\"a\\\"b\");p(\"a\\\\b\")}.#minimize{1@0,X:p(X)}.",
        "{p(a);p(2)}.#minimize{X@0,k:p(X)}.",
        "{a}.#minimize{0@7,k:a}.",
        "{a}.",
        "#minimize{}.",
    ] {
        compare_all(source);
    }
}

fn constant_score(priority: i32, weight: i32) -> Score {
    let objectives = ObjectiveProgram::new(
        vec![ObjectiveTemplate::new(
            Term::Constant(Value::Number(weight)),
            priority,
            vec![],
            vec![],
            vec![],
        )],
        zetesis_objective::AdmissionLimits::default(),
    )
    .expect("fixed objective");
    evaluate(
        &objectives,
        &Model::new([]).unwrap(),
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .expect("fixed score")
    .score()
    .clone()
}
#[test]
fn foreign_priority_slots_obey_missing_zero_comparison() {
    let input = formula("{a}.#minimize{2@3,k:a}.");
    let plan = plan(&input);
    for incumbent in [
        constant_score(4, -1),
        constant_score(4, 1),
        constant_score(2, 1),
        constant_score(2, -1),
        constant_score(4, 0),
    ] {
        let bound = plan
            .bound(
                &incumbent,
                ObjectiveBoundLimits::default(),
                &Cancellation::default(),
            )
            .expect("priority union");
        for mask in 0..2 {
            let candidate = Interpretation::new(
                bound.theory(),
                (0..1).filter(|index| mask & (1 << index) != 0),
            )
            .expect("candidate");
            assert_eq!(
                models(
                    bound.theory(),
                    &candidate,
                    zetesis_ferraris::Limits::default(),
                    &Cancellation::default()
                )
                .expect("truth"),
                score(&input, mask).compare_costs(&incumbent) != Ordering::Greater
            );
        }
    }
}

#[test]
fn candidate_restriction_preserves_original_reduct_and_every_optimal_tie() {
    let input = formula("1{a;b}1.p:-p.#minimize{1@0,k:a;1@0,k:b}.");
    let original_nodes = (
        input.theory().nodes().to_vec(),
        input.theory().operands().to_vec(),
    );
    let mut baseline = StableModels::new(
        input.theory(),
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .expect("original search");
    let expected: BTreeSet<_> = baseline
        .by_ref()
        .map(|model| model.expect("verified").atoms().collect::<Vec<_>>())
        .collect();
    assert!(baseline.exhausted());
    assert_eq!(expected.len(), 2);
    let model = Model::from_positions(
        input.atom_catalog(),
        expected.first().expect("incumbent").iter().copied(),
    )
    .unwrap();
    let incumbent = evaluate(
        input.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .expect("verified incumbent score");
    let bound = plan(&input)
        .bound(
            incumbent.score(),
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
        .expect("candidate bound");
    let mut restricted = StableModels::new(
        input.theory(),
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .expect("original search");
    restricted
        .restrict_candidates(bound.theory())
        .expect("candidate-only append");
    let actual: BTreeSet<_> = restricted
        .by_ref()
        .map(|model| {
            let model = model.expect("verified against original reduct");
            assert!(model.theory().same_instance(input.theory()));
            model.atoms().collect::<Vec<_>>()
        })
        .collect();
    assert!(restricted.exhausted());
    assert_eq!(actual, expected);
    assert_eq!(
        (input.theory().nodes(), input.theory().operands()),
        (original_nodes.0.as_slice(), original_nodes.1.as_slice())
    );
}

#[test]
fn optional_plan_limits_are_typed() {
    let input = formula("{p(1);p(2)}.#minimize{-1@0,X:p(X)}.");
    for (limits, resource) in [
        (
            ObjectivePlanLimits {
                max_atoms: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::Atoms,
        ),
        (
            ObjectivePlanLimits {
                max_bindings: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::Bindings,
        ),
        (
            ObjectivePlanLimits {
                max_keys: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::Keys,
        ),
        (
            ObjectivePlanLimits {
                max_key_bytes: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::KeyBytes,
        ),
        (
            ObjectivePlanLimits {
                max_tuple_width: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::TupleWidth,
        ),
        (
            ObjectivePlanLimits {
                max_nodes: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::Nodes,
        ),
        (
            ObjectivePlanLimits {
                max_variables: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::Variables,
        ),
        (
            ObjectivePlanLimits {
                max_body_atoms: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::BodyAtoms,
        ),
        (
            ObjectivePlanLimits {
                max_work: 0,
                ..ObjectivePlanLimits::default()
            },
            ObjectiveBoundResource::Work,
        ),
    ] {
        let error = ObjectivePlan::new(
            input.theory(),
            input.atoms(),
            input.objectives(),
            limits,
            &Cancellation::default(),
        )
        .expect_err("bounded optional refusal");
        assert_eq!(error.kind(), ObjectiveBoundErrorKind::Limit(resource));
    }
}

#[test]
fn optional_bound_failures_do_not_poison_the_plan_or_theory() {
    let input = formula("{p(1);p(2)}.#minimize{-1@0,X:p(X)}.");
    let plan = plan(&input);
    let incumbent = score(&input, 0);
    let limits = ObjectiveBoundLimits {
        aggregate: zetesis_ferraris::AggregateLimits {
            max_states: 0,
            ..zetesis_ferraris::AggregateLimits::default()
        },
        ..ObjectiveBoundLimits::default()
    };
    assert!(matches!(
        plan.bound(&incumbent, limits, &Cancellation::default())
            .expect_err("both exact representations exceed the state ceiling")
            .kind(),
        ObjectiveBoundErrorKind::Aggregate(_)
    ));
    assert!(
        plan.bound(
            &incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default()
        )
        .is_ok()
    );
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(matches!(
        plan.bound(&incumbent, ObjectiveBoundLimits::default(), &cancellation)
            .expect_err("cancelled optional work")
            .kind(),
        ObjectiveBoundErrorKind::Control(_)
    ));
    let atom = input
        .atoms()
        .at(0)
        .unwrap()
        .to_atom(zetesis_core::ValueLimits::default())
        .unwrap();
    let duplicates = zetesis_core::AtomCatalog::new(vec![atom.clone(), atom]).unwrap();
    assert_eq!(
        ObjectivePlan::new(
            input.theory(),
            duplicates.atoms(),
            input.objectives(),
            ObjectivePlanLimits::default(),
            &Cancellation::default()
        )
        .expect_err("duplicate identity")
        .kind(),
        ObjectiveBoundErrorKind::AtomCatalog
    );
}

#[test]
fn cumulative_accounting_respects_inclusive_work_limits_on_every_return() {
    let input = formula("{a;b}.#minimize{2@3,k:a;1@0,k:b}.");
    let complete_plan = plan(&input);
    let plan_work = complete_plan.statistics().work;
    for ceiling in [0, 1, plan_work - 1, plan_work] {
        let result = ObjectivePlan::new(
            input.theory(),
            input.atoms(),
            input.objectives(),
            ObjectivePlanLimits {
                max_work: ceiling,
                ..ObjectivePlanLimits::default()
            },
            &Cancellation::default(),
        );
        if ceiling == plan_work {
            assert_eq!(
                result.expect("inclusive complete plan").statistics().work,
                ceiling
            );
        } else {
            let error = result.expect_err("plan incomplete below exact charged work");
            assert!(error.statistics().work <= ceiling);
            assert_eq!(
                error.kind(),
                ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work)
            );
        }
    }
    let incumbent = score(&input, 3);
    let complete_bound = complete_plan
        .bound(
            &incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default(),
        )
        .expect("complete bound");
    let bound_work = complete_bound.statistics().work;
    for ceiling in [0, 1, bound_work - 1, bound_work] {
        let result = complete_plan.bound(
            &incumbent,
            ObjectiveBoundLimits {
                max_work: ceiling,
                ..ObjectiveBoundLimits::default()
            },
            &Cancellation::default(),
        );
        if ceiling == bound_work {
            assert_eq!(
                result.expect("inclusive complete bound").statistics().work,
                ceiling
            );
        } else {
            assert!(
                result
                    .expect_err("bound incomplete below exact charged work")
                    .statistics()
                    .work
                    <= ceiling
            );
        }
    }
}
