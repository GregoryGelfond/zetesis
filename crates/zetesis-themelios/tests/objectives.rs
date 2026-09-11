//! Lifted objectives score verified stable models without supplying support.

use std::collections::BTreeSet;
use std::path::PathBuf;

use zetesis_core::{Atom, Model};
use zetesis_cpu::Control;
use zetesis_objective::{Score, evaluate};
use zetesis_sat::{Limits as SearchLimits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits, ExpansionFailure,
    ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, SourceBundle,
    admit_bundle_formula, admit_extended, admit_formula,
};

fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}
fn scored(
    theory: &zetesis_ferraris::Theory,
    atoms: &[Atom],
    objectives: &zetesis_objective::ObjectiveProgram,
) -> Vec<(Model, Score)> {
    let control = Control::default();
    let mut search =
        StableModels::new(theory, SearchLimits::default(), control.clone()).expect("SAT admission");
    let result = search
        .by_ref()
        .map(|candidate| {
            let candidate = candidate.expect("complete reduct-verified model");
            let model = Model::new(candidate.atoms().map(|atom| atoms[atom].clone()));
            let score = evaluate(
                objectives,
                &model,
                zetesis_objective::Limits::default(),
                &control,
            )
            .expect("complete objective score")
            .score()
            .clone();
            (model, score)
        })
        .collect();
    assert!(
        search.exhausted(),
        "optimality requires complete stable-model exhaustion"
    );
    result
}
fn scores(source: &str) -> Vec<Vec<(i32, i64)>> {
    let admitted = input(source);
    let mut scores: Vec<_> = scored(admitted.theory(), admitted.atoms(), admitted.objectives())
        .into_iter()
        .map(|(_, score)| score.costs().to_vec())
        .collect();
    scores.sort();
    scores
}
fn expected_model(facts: &str) -> BTreeSet<Atom> {
    admit_extended(
        facts.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .expect("expected scalar facts")
    .program()
    .templates()
    .iter()
    .map(|template| {
        template
            .head()
            .expect("fact head")
            .instantiate(&[])
            .expect("ground head")
    })
    .collect()
}

#[test]
fn unchanged_task_allocation_graphs_have_complete_optimal_contracts() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation/corpus/kr-domains/scenarios/task-allocation/variant-01");
    let cases = [
        (
            "01-basic.lp",
            4,
            Some(5),
            20,
            "assigned_to(a1,t1). assigned_to(a2,t2). assigned_cost(a1,t1,3). assigned_cost(a2,t2,2).",
        ),
        (
            "02-agent-reuse.lp",
            8,
            Some(6),
            29,
            "assigned_to(a1,t1). assigned_to(a1,t2). assigned_to(a1,t3). assigned_cost(a1,t1,1). assigned_cost(a1,t2,2). assigned_cost(a1,t3,3).",
        ),
        (
            "03-selective-compatibility.lp",
            4,
            Some(10),
            26,
            "assigned_to(a2,t1). assigned_to(a3,t2). assigned_to(a1,t3). assigned_cost(a2,t1,2). assigned_cost(a3,t2,3). assigned_cost(a1,t3,5).",
        ),
        ("04-no-compatible-agent-unsat.lp", 0, None, 17, ""),
        (
            "05-larger-mix.lp",
            81,
            Some(9),
            55,
            "assigned_to(a3,t1). assigned_to(a1,t2). assigned_to(a2,t3). assigned_to(a3,t4). assigned_cost(a3,t1,2). assigned_cost(a1,t2,3). assigned_cost(a2,t3,1). assigned_cost(a3,t4,3).",
        ),
    ];
    for (path, count, cost, possible, expected) in cases {
        let bundle = SourceBundle::load(base.join(path), BundleLimits::default())
            .expect("unchanged original graph");
        let admitted = admit_bundle_formula(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(
            admitted.atoms().len(),
            possible,
            "{path}: relational support, not the full D product"
        );
        assert_eq!(admitted.objectives().templates().len(), 1);
        assert_eq!(admitted.objective_origins().len(), 1);
        assert_eq!(admitted.objective_declarations().len(), 1);
        for origin in admitted.objective_origins().iter().flatten() {
            assert!(
                admitted
                    .bundle()
                    .get(origin.source)
                    .expect("original objective source")
                    .path()
                    .ends_with("encodings/task-allocation/variant-01.lp")
            );
        }
        let models = scored(admitted.theory(), admitted.atoms(), admitted.objectives());
        assert_eq!(models.len(), count, "{path}: all stable models exhausted");
        if let Some(cost) = cost {
            let best = models
                .iter()
                .map(|(_, score)| score.costs()[0].1)
                .min()
                .expect("satisfiable");
            assert_eq!(best, cost, "{path}: complete optimum");
            let optimal: Vec<_> = models
                .iter()
                .filter(|(_, score)| score.costs() == [(0, cost)])
                .collect();
            assert_eq!(optimal.len(), 1, "{path}: complete optimal count");
            let shown: BTreeSet<_> = optimal[0]
                .0
                .atoms()
                .iter()
                .filter(|atom| admitted.metadata().output().includes(atom))
                .cloned()
                .collect();
            assert_eq!(
                shown,
                expected_model(expected),
                "{path}: full original optimal display contract"
            );
        } else {
            assert!(models.is_empty());
        }
    }
}

#[test]
fn repeated_enabled_tuples_coalesce_across_source_elements_and_statements() {
    assert_eq!(scores("a. b. #minimize{3,x:a;3,x:b}."), vec![vec![(0, 3)]]);
    assert_eq!(
        scores("a. #minimize{3,x:a}. #minimize{3,x:a}."),
        vec![vec![(0, 3)]]
    );
    assert_eq!(
        scores("a. b. #minimize{3,x:a;3,y:b;4,x:b}."),
        vec![vec![(0, 10)]]
    );
    assert_eq!(scores("d(1..2). #minimize{3,x:d(X)}."), vec![vec![(0, 3)]]);
}

#[test]
fn merged_objective_templates_keep_every_original_declaration_span() {
    let admitted = input("a. #minimize{1:a}. #minimize{1:a}.");
    assert_eq!(admitted.objectives().templates().len(), 1);
    assert_eq!(admitted.objective_declarations().len(), 2);
    let spans = &admitted.objective_origins()[0];
    assert!(
        admitted
            .objective_declarations()
            .iter()
            .all(|location| spans.contains(location))
    );
    for location in admitted.objective_declarations() {
        assert_eq!(
            admitted
                .source()
                .slice(location.span)
                .expect("original declaration"),
            "#minimize{1:a}."
        );
    }
}

#[test]
fn priorities_zero_weights_cancellation_and_inactive_models_keep_fixed_slots() {
    assert_eq!(scores("{a}. :- a. #minimize{1@7,k:a}."), vec![vec![(7, 0)]]);
    assert_eq!(
        scores("a. #minimize{0@9,k:a;7@1,j:a}."),
        vec![vec![(9, 0), (1, 7)]]
    );
    assert_eq!(
        scores("a. #minimize{3@7,k:a;-3@7,j:a}."),
        vec![vec![(7, 0)]]
    );
    assert_eq!(
        scores("{a}. #minimize{-3:a}."),
        vec![vec![(0, -3)], vec![(0, 0)]]
    );
    assert_eq!(
        scores("{a}. :- not a. #minimize{1@2,k:a}."),
        vec![vec![(2, 1)]]
    );
}

#[test]
fn objective_declarations_survive_while_unreachable_priority_slots_disappear() {
    for source in [
        "#minimize{}.",
        "#minimize{3:a}.",
        "a :- a. #minimize{3@9,x:a}.",
        "d(1). #minimize{3@7,x:d(X),X!=1}.",
    ] {
        let admitted = input(source);
        assert!(!admitted.objectives().is_present(), "{source}");
        assert!(admitted.objective_origins().is_empty());
        assert_eq!(admitted.objective_declarations().len(), 1);
    }
    let admitted = input("a. #minimize{0:a;7@2:b}.");
    assert!(admitted.objectives().is_present());
    assert_eq!(admitted.objectives().priorities(), &[0]);
}

#[test]
fn objective_conditions_bind_scalars_without_adding_logical_support() {
    assert_eq!(
        scores("d(a,2;b,5). {p(X)} :- d(X,C). #minimize{C,X:p(X),d(X,C)}."),
        vec![vec![(0, 0)], vec![(0, 2)], vec![(0, 5)], vec![(0, 7)]]
    );
    let admitted = input("a :- a. #minimize{1:a}.");
    assert_eq!(
        scored(admitted.theory(), admitted.atoms(), admitted.objectives()).len(),
        1
    );
    assert!(admitted.atoms().is_empty());
}

#[test]
fn original_objective_budgets_count_occurrences_before_canonicalization() {
    let limits = FormulaLimits {
        objective: zetesis_objective::AdmissionLimits {
            max_templates: 1,
            ..zetesis_objective::AdmissionLimits::default()
        },
        ..FormulaLimits::default()
    };
    let error = admit_formula(
        "a. #minimize{1:a;1:a}.".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
    )
    .expect_err("raw objective element budget");
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::ObjectiveElements,
            ..
        }
    ));
    let limits = FormulaLimits {
        objective: zetesis_objective::AdmissionLimits {
            max_tuple_width: 1,
            ..zetesis_objective::AdmissionLimits::default()
        },
        ..FormulaLimits::default()
    };
    assert!(
        matches!(
            admit_formula(
                "#minimize{1,a,b:p}.".to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                limits
            ),
            Err(FormulaFailure::Objective { .. })
        ),
        "independent objective admission runs even for unreachable conditions"
    );
}

#[test]
fn unsupported_objective_syntax_and_unsafe_variables_are_typed_refusals() {
    for source in ["#minimize{X:a}.", "d(1). #minimize{Y+1:d(X)}."] {
        assert!(
            admit_formula(
                source.to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default()
            )
            .is_err(),
            "{source}"
        );
    }
}

#[test]
fn profile_retry_classification_does_not_hide_malformed_arithmetic_or_limits() {
    for source in ["1{a;b}1.", "#minimize{1:a}.", "d(1). p(X) :- d(X), X+1=2."] {
        let error = admit_extended(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .expect_err("requires formula profile");
        assert!(error.needs_formula_admission(), "{source}");
    }
    for source in ["p(1/0).", "p(2147483647+1).", "p(."] {
        let error = admit_extended(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .expect_err("not a profile mismatch");
        assert!(!error.needs_formula_admission(), "{source}");
    }
    let error: ExpansionFailure = admit_extended(
        "p(1..3).".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits {
            max_templates: 1,
            ..ExpansionLimits::default()
        },
    )
    .expect_err("budget");
    assert!(!error.needs_formula_admission());
}
