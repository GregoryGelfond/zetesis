//! Every signed candidate guard uses the same normalized keys as exact scoring.
use std::cmp::Ordering;
use std::collections::BTreeSet;

use zetesis_core::{Model, Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, models};
use zetesis_objective::{ObjectiveProgram, ObjectiveTemplate, Score, WeightPolarity, evaluate};
use zetesis_sat::StableModels;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundLimits, ObjectiveBoundResource, ObjectivePlan,
    ObjectivePlanLimits,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}
fn score(input: &AdmittedFormula, selected: impl IntoIterator<Item = usize>) -> Score {
    let model = Model::from_positions(input.atom_catalog(), selected).unwrap();
    evaluate(
        input.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
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
    .unwrap()
}

#[test]
fn every_candidate_guard_matches_full_normalized_scores_and_keeps_ties() {
    for source in [
        "{z;a;b}. #maximize{2@7,k:a}. #minimize{-2@7,k:b;1@0,k:z}.",
        "{a;b}. #maximize{2@7,k:a}. :~ b. [-2@7,k]",
        "{a;b}. #maximize{2@7,k:a}. #minimize{2@7,k:b}.",
        "{a;b}. #maximize{-2@7,k:a;0@0,k:b}.",
        "{p(-2);p(3)}. #maximize{W@1,k:p(W)}. #minimize{-3@1,k:p(3)}.",
        "{a;b}. #maximize{2@1,k:a;2@2,k:b}. #minimize{-2@1,k:b}.",
        "{a;b}. #maximize{0@1,k:a}. #minimize{0@1,k:b}.",
        "{p(a);p(2);p(\"a\")}. #maximize{X@0,k:p(X)}.",
    ] {
        let input = input(source);
        let plan = plan(&input);
        let masks = 1_usize << input.atoms().len();
        let scores: Vec<_> = (0..masks)
            .map(|mask| {
                score(
                    &input,
                    (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
                )
            })
            .collect();
        for incumbent in &scores {
            let bound = plan
                .bound(
                    incumbent,
                    ObjectiveBoundLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert!(bound.original().same_instance(input.theory()));
            for (mask, score) in scores.iter().enumerate() {
                let candidate = Interpretation::new(
                    bound.theory(),
                    (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
                )
                .unwrap();
                assert_eq!(
                    models(
                        bound.theory(),
                        &candidate,
                        zetesis_ferraris::Limits::default(),
                        &Cancellation::default()
                    )
                    .unwrap(),
                    score.compare_costs(incumbent) != Ordering::Greater,
                    "{source}: {mask}"
                );
            }
        }
    }
}

type Optima = (Vec<(i32, i64)>, BTreeSet<Vec<usize>>);
fn search(input: &AdmittedFormula, pruning: bool) -> Optima {
    let plan = plan(input);
    let mut search = StableModels::new(
        input.theory(),
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut best: Option<Score> = None;
    let mut ties = BTreeSet::new();
    while let Some(model) = search.next() {
        let model = model.unwrap();
        let atoms: Vec<_> = model.atoms().collect();
        let score = score(input, atoms.iter().copied());
        match best
            .as_ref()
            .map_or(Ordering::Less, |best| score.compare_costs(best))
        {
            Ordering::Less => {
                ties.clear();
                ties.insert(atoms);
                if pruning {
                    let bound = plan
                        .bound(
                            &score,
                            ObjectiveBoundLimits::default(),
                            &Cancellation::default(),
                        )
                        .unwrap();
                    search.restrict_candidates(bound.theory()).unwrap();
                }
                best = Some(score);
            }
            Ordering::Equal => {
                ties.insert(atoms);
            }
            Ordering::Greater => {}
        }
    }
    assert!(search.exhausted());
    (best.unwrap().costs().to_vec(), ties)
}

#[test]
fn verified_incumbent_pruning_preserves_every_optimal_model_and_original_reduct() {
    for source in [
        "{a;b;c}. #maximize{2@7,k:a}. #minimize{-2@7,k:b;1@0,k:c}.",
        "{p(-2);p(3)}. #maximize{W@1,k:p(W)}.",
        "1{a;b}1. loop:-loop. #maximize{1@2,k:a;1@2,k:b}.",
    ] {
        let input = input(source);
        let nodes = input.theory().nodes().to_vec();
        let roots = input.theory().roots().to_vec();
        let baseline = search(&input, false);
        assert_eq!(search(&input, true), baseline);
        assert_eq!(input.theory().nodes(), nodes);
        assert_eq!(input.theory().roots(), roots);
    }
}

#[test]
fn optional_plan_refusal_is_typed_and_preserves_work_and_original_theory() {
    let input = input("{a}. #maximize{2@7,k:a}.");
    let plan = plan(&input);
    let limits = ObjectivePlanLimits {
        max_work: plan.statistics().work - 1,
        ..ObjectivePlanLimits::default()
    };
    let error = ObjectivePlan::new(
        input.theory(),
        input.atoms(),
        input.objectives(),
        limits,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work)
    );
    assert!(error.statistics().work <= limits.max_work);
    let invalid = ObjectiveProgram::new(
        vec![
            ObjectiveTemplate::new(
                Term::Constant(Value::Number(i32::MIN)),
                0,
                vec![],
                vec![],
                vec![],
            )
            .with_weight_polarity(WeightPolarity::Negated),
        ],
        zetesis_objective::AdmissionLimits::default(),
    )
    .unwrap();
    let error = ObjectivePlan::new(
        input.theory(),
        input.atoms(),
        &invalid,
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        ObjectiveBoundErrorKind::WeightNormalizationOverflow
    );
    assert_eq!(error.template_index(), Some(0));
    let incumbent = score(&input, 0..input.atoms().len());
    assert!(
        plan.bound(
            &incumbent,
            ObjectiveBoundLimits::default(),
            &Cancellation::default()
        )
        .is_ok()
    );
}

#[test]
fn checked_source_arithmetic_reaches_only_eligible_negation_endpoints() {
    use zetesis_themelios::{ExpansionFailure, FormulaFailure, ProfileFeature};
    for source in [
        "#maximize{(-2147483647-1)@1,k:absent}.",
        "v(-2147483647-1).v(1). #maximize{W@1,k:v(W),W!=(-2147483647-1)}.",
    ] {
        let input = input(source);
        let _ = plan(&input);
    }
    for source in [
        "#maximize{(-2147483647-1)@1,k}.",
        "v(-2147483647-1).v(1). #maximize{W@1,k:v(W)}.",
    ] {
        let error = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(
                    zetesis_themelios::AdmissionFailure::Profile {
                        feature: ProfileFeature::NumericOverflow,
                        ..
                    }
                ))
            ),
            "{source}: {error}"
        );
    }
}
