//! Aggregate assignment values are proposals whose equality remains in the reduct.

use std::collections::BTreeSet;

use zetesis_core::{Atom, Model};
use zetesis_cpu::Control;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, admit_formula,
};

type Models = BTreeSet<BTreeSet<Atom>>;
fn admit(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}
fn input(source: &str) -> AdmittedFormula {
    admit(source).unwrap_or_else(|error| panic!("{source}: {error}"))
}
fn models(input: &AdmittedFormula) -> Models {
    let mut search = StableModels::new(input.theory(), Limits::default(), Control::default())
        .expect("search admission");
    let result = search
        .by_ref()
        .map(|model| {
            model
                .expect("complete search")
                .atoms()
                .map(|index| input.atoms()[index].clone())
                .collect()
        })
        .collect();
    assert!(search.exhausted());
    result
}
fn expected(sources: &[&str]) -> Models {
    sources
        .iter()
        .flat_map(|source| models(&input(source)))
        .collect()
}

#[test]
fn finite_results_extend_the_source_domain_and_empty_aggregates_produce_zero() {
    for (source, result) in [
        (
            "e(a,2). e(b,3). n(N):-N=#sum{W,X:e(X,W)}.",
            "e(a,2).e(b,3).n(5).",
        ),
        ("n(N):-N=#sum{}.", "n(0)."),
        ("n(N):-N=#count{}.", "n(0)."),
        (
            "e(a,2).e(b,2).n(N):-#sum{W:e(X,W)}=N.",
            "e(a,2).e(b,2).n(2).",
        ),
        (
            "e(a,2).e(b,2).n(N):-#count{X:e(X,W)}=N.",
            "e(a,2).e(b,2).n(2).",
        ),
    ] {
        assert_eq!(models(&input(source)), expected(&[result]), "{source}");
    }
}

#[test]
fn recursive_assignment_conditions_do_not_become_extensional_facts() {
    for (source, result) in [
        ("n(N):-N=#count{1:n(1)}.", vec!["n(0)."]),
        ("n(N):-N=#count{1:not n(0)}.", vec!["n(0).", "n(1)."]),
        ("n(N):-N=#sum{-1:n(-1)}.", vec!["n(0)."]),
        ("{a}.n(N):-N=#sum{2,k:a;3,l:a}.", vec!["n(0).", "a.n(5)."]),
    ] {
        assert_eq!(models(&input(source)), expected(&result), "{source}");
    }
}

#[test]
fn outer_bindings_limit_aggregate_eligibility() {
    assert_eq!(
        models(&input(
            "g(a).p(a,2).p(b,3).total(G,N):-g(G),N=#sum{W:p(G,W)}."
        )),
        expected(&["g(a).p(a,2).p(b,3).total(a,2)."])
    );
}

#[test]
fn bound_targets_remain_aggregate_tests() {
    assert_eq!(
        models(&input("d(2).p.n(N):-d(N),N=#sum{2:p}.")),
        expected(&["d(2).p.n(2)."])
    );
}

#[test]
fn own_targets_cannot_bind_local_witnesses() {
    for source in ["n(N):-N=#count{X:p(N,X)}.", "n(N):-N=#count{N:p}."] {
        assert!(
            matches!(admit(source), Err(FormulaFailure::UnsafeVariable { .. })),
            "{source}"
        );
    }
}

#[test]
fn aggregate_producers_consume_prior_values() {
    let source = "n(A,B):-A=#count{},B=#count{A:p}.";
    assert_eq!(models(&input(source)), expected(&["n(0,0)."]));
}

#[test]
fn generative_recursion_and_candidate_rows_have_typed_independent_limits() {
    let limits = FormulaLimits {
        max_support_rounds: 2,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        admit_formula(
            "n(N):-N=#count{X:n(X)}.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            ..
        })
    ));
    let limits = FormulaLimits {
        max_assignment_values: 0,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        admit_formula(
            "n(N):-N=#sum{}.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::AssignmentValues,
            ..
        })
    ));
    let limits = FormulaLimits {
        max_assignment_values: 2,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        admit_formula(
            "{a;b}.n(N):-N=#sum{1:a;2:b}.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::AssignmentValues,
            ..
        })
    ));
    assert!(matches!(
        admit("{a;b}.n(N):-N=#sum{2147483647,a:a;2147483647,b:b}."),
        Err(FormulaFailure::Expansion(
            ExpansionFailure::Evaluation { .. }
        ))
    ));
}

#[test]
fn direct_observers_use_structural_positions_and_score_verified_models() {
    let source = "g(a).p(a,2).total(G,N):-g(G),N=#sum{W:p(G,W)}. #minimize{N@3,G:total(G,N);0@1,G:total(G,N)}.";
    let input = input(source);
    assert!(input.objectives().is_present());
    assert_eq!(input.objectives().priorities(), &[3, 1]);
    for model in models(&input) {
        let model = Model::new(model);
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &Control::default(),
        )
        .expect("verified model objective");
        assert_eq!(evaluation.score().costs(), &[(3, 2), (1, 0)]);
    }
    // The pinned upstream ASP-Core-2 reading requires every aggregate guard
    // variable. Keep that fact visible while our explicit clingo binder checks
    // justify this narrowly admitted extension.
    assert!(!input.source_analysis().safety().is_safe());
    assert_eq!(input.source().text(), source);
}

#[test]
fn constants_filters_shared_outputs_and_downstream_producers_are_not_direct_observers() {
    for suffix in [
        "#minimize{1@7:n(0)}.",
        "#minimize{N@7:n(N),N!=0}.",
        "d(0).#minimize{N@7:n(N),d(N)}.",
        "x:-n(0).#minimize{1@7:x}.",
    ] {
        let source = format!("{{a}}. n(N):-N=#sum{{2:a}}. {suffix}");
        assert!(
            matches!(
                admit(&source),
                Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
                    AdmissionFailure::Profile {
                        feature: ProfileFeature::ObjectiveAggregateDependency,
                        ..
                    }
                )))
            ),
            "{source}"
        );
    }
}

#[test]
fn generated_positions_union_across_producers_without_changing_full_model_identity() {
    let source =
        "{a}.p(N,7):-N=#sum{2:a}.p(5,N):-N=#count{1:a}.p(9,9).{p(8,8)}.#minimize{X@1,Y:p(X,Y)}.";
    let input = input(source);
    assert_eq!(
        models(&input),
        expected(&[
            "p(0,7).p(5,0).p(9,9).",
            "p(0,7).p(5,0).p(9,9).p(8,8).",
            "a.p(2,7).p(5,1).p(9,9).",
            "a.p(2,7).p(5,1).p(9,9).p(8,8).",
        ])
    );
    let costs: BTreeSet<_> = models(&input)
        .into_iter()
        .map(|model| {
            zetesis_objective::evaluate(
                input.objectives(),
                &Model::new(model),
                zetesis_objective::Limits::default(),
                &Control::default(),
            )
            .expect("verified model objective")
            .score()
            .costs()[0]
                .1
        })
        .collect();
    assert_eq!(costs, BTreeSet::from([14, 16, 22, 24]));
    let same = input_source("{a}.p(N,N):-N=#count{1:a}.#minimize{X@1,Y:p(X,Y)}.");
    assert_eq!(same, expected(&["p(0,0).", "a.p(1,1)."]));
}
fn input_source(source: &str) -> Models {
    models(&input(source))
}

#[test]
fn unchanged_layered_dag_shares_sum_thresholds_within_original_default_budgets() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../validation/corpus/kr-domains/scenarios/shortest-path/variant-01/06-layered-dag.lp",
    );
    let bundle =
        zetesis_themelios::SourceBundle::load(&path, zetesis_themelios::BundleLimits::default())
            .expect("original include graph");
    let input = zetesis_themelios::admit_bundle_formula(
        bundle,
        zetesis_themelios::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("bounded original aggregate assignment");
    assert_eq!(bundle_optimum(&input), (vec![4, 4], 1));
    assert_eq!(input.bundle().sources().len(), 2);
}

fn bundle_optimum(input: &zetesis_themelios::AdmittedFormulaBundle) -> (Vec<i64>, usize) {
    let mut search = StableModels::new(input.theory(), Limits::default(), Control::default())
        .expect("search admission");
    let mut best: Option<Vec<i64>> = None;
    let mut optimal = 0;
    for model in search.by_ref() {
        let model = model.expect("complete layered DAG search");
        let atoms = Model::new(model.atoms().map(|index| input.atoms()[index].clone()));
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &atoms,
            zetesis_objective::Limits::default(),
            &Control::default(),
        )
        .expect("verified model objective");
        let costs: Vec<_> = evaluation
            .score()
            .costs()
            .iter()
            .map(|(_, value)| *value)
            .collect();
        if best.as_ref().is_none_or(|best| costs < *best) {
            best = Some(costs);
            optimal = 1;
        } else if best.as_ref() == Some(&costs) {
            optimal += 1;
        }
    }
    assert!(search.exhausted());
    (best.expect("at least one verified model"), optimal)
}

#[test]
fn one_threshold_family_per_binding_meets_the_original_cli_work_ceiling() {
    for (path, cost, count) in [
        ("variant-03/06-layered-dag-cap.lp", [7, 4], 2),
        ("variant-03/07-layered-dag-tight-cap.lp", [12, 4], 4),
        ("variant-04/06-layered-dag-ordering-cap.lp", [8, 4], 2),
        ("variant-04/07-layered-dag-combined.lp", [10, 4], 1),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../validation/corpus/kr-domains/scenarios/shortest-path")
            .join(path);
        let bundle = zetesis_themelios::SourceBundle::load(
            &path,
            zetesis_themelios::BundleLimits::default(),
        )
        .expect("original include graph");
        let input = zetesis_themelios::admit_bundle_formula(
            bundle,
            zetesis_themelios::BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_work: 1_048_576,
                ..FormulaLimits::default()
            },
        )
        .expect("complete bounded family admission");
        assert_eq!(bundle_optimum(&input), (cost.to_vec(), count), "{path:?}");
    }
}

#[test]
fn eligibility_cache_separates_source_aggregates_and_outer_bindings() {
    assert_eq!(
        models(&input("{a}.n(N):-N=#sum{1:a}.m(M):-M=#sum{1:not a}.")),
        expected(&["n(0).m(1).", "a.n(1).m(0)."])
    );
    assert_eq!(
        models(&input(
            "g(x;y).e(x,1).e(y,2).n(G,N):-g(G),N=#sum{V:e(G,V)}."
        )),
        expected(&["g(x;y).e(x,1).e(y,2).n(x,1).n(y,2)."])
    );
    let source = "{a}.n(N):-N=#sum{1:a}.";
    for (limits, resource) in [
        (
            FormulaLimits {
                max_aggregate_cache_rows: 0,
                ..FormulaLimits::default()
            },
            FormulaResource::AggregateCacheRows,
        ),
        (
            FormulaLimits {
                max_aggregate_cache_elements: 0,
                ..FormulaLimits::default()
            },
            FormulaResource::AggregateCacheElements,
        ),
        (
            FormulaLimits {
                max_aggregate_cache_key_bytes: 0,
                ..FormulaLimits::default()
            },
            FormulaResource::AggregateCacheKeys,
        ),
        (
            FormulaLimits {
                max_aggregate_cache_roots: 0,
                ..FormulaLimits::default()
            },
            FormulaResource::AggregateCacheRoots,
        ),
    ] {
        let error = admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .expect_err("explicit cache ceiling");
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, .. } if actual == resource)
        );
        assert!(!error.diagnostics().is_empty());
    }
}
