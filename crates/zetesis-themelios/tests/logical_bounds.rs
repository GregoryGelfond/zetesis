//! Logical bounds compare finite numeric measures without changing permissions.

#[path = "support/logical_bounds.rs"]
mod cases;
#[path = "support/finite_bindings.rs"]
mod reference;
#[path = "support/logical_bound_semantics.rs"]
mod semantics;

use cases::{BOUNDS, RELATIONS, expected, sources};
use reference::{Models, atom_text, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, CountPlanLimits, CountPlanStatus, ExpansionFailure,
    ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource,
    admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(229);

fn options() -> AdmissionOptions {
    AdmissionOptions {
        source_id: SOURCE,
        ..Default::default()
    }
}

fn limited(source: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        *limits,
    )
}

fn input(source: &str) -> AdmittedFormula {
    limited(source, &FormulaLimits::default()).unwrap_or_else(|error| panic!("{source}: {error}"))
}

#[test]
fn complete_answers_match_declared_contracts() {
    for (source, expected) in sources() {
        assert_eq!(native(&input(&source)), expected, "{source}");
    }
}

#[test]
fn stability_matches_independent_subset_enumeration() {
    for (source, _) in sources() {
        let admitted = input(&source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn original_sources_remain_owned() {
    for (source, _) in sources() {
        assert_eq!(input(&source).source().text(), source);
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_sources_match_declared_answers() {
    let cases = sources();
    let mut total = 0;
    for (source, expected) in &cases {
        let result = external(source, true);
        assert_eq!(result["Models"]["More"], "no");
        let mut models = Models::new();
        let mut count = 0;
        for call in result["Call"].as_array().unwrap() {
            for witness in call["Witnesses"].as_array().into_iter().flatten() {
                assert!(witness["Costs"].is_null());
                assert!(
                    models.insert(
                        witness["Value"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|atom| atom.as_str().unwrap().to_owned())
                            .collect()
                    )
                );
                count += 1;
            }
        }
        assert_eq!(result["Models"]["Number"].as_u64(), Some(count));
        assert_eq!(&models, expected, "{source}");
        total += count;
    }
    println!("complete_sources={} full_models={total}", cases.len());
}

#[test]
fn logical_guards_do_not_certify_numeric_count_plans() {
    for source in [
        "2{a;b;c;d}word.{a;b}1.{c;d}1.",
        "2#count{1:a;2:b;3:c;4:d}word.{a;b}1.{c;d}1.",
        "word>={a;b;c;d}>=2.{a;b}1.{c;d}1.",
    ] {
        let ordinary = input(source);
        let planned = prepare_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .ground_with_count_plan(
            CountPlanLimits::default(),
            &zetesis_cpu::Control::default(),
            None,
        )
        .unwrap();
        assert!(matches!(planned.count_plan(), CountPlanStatus::NoPlan(_)));
        assert_eq!(ordinary.atoms(), planned.atoms());
        assert_eq!(ordinary.theory().nodes(), planned.theory().nodes());
        assert_eq!(ordinary.theory().roots(), planned.theory().roots());
    }
}

#[test]
fn independent_numeric_groups_retain_count_plans() {
    let source = "2{a;b;c;d}2.{a;b}1.{c;d}1.{e}word.";
    let ordinary = input(source);
    let planned = prepare_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(
        CountPlanLimits::default(),
        &zetesis_cpu::Control::default(),
        None,
    )
    .unwrap();
    let CountPlanStatus::Ready(plan) = planned.count_plan() else {
        panic!("the independent numeric partition remains eligible");
    };
    assert_eq!(plan.consequence_count(), 2);
    assert!(planned.theory().same_instance(plan.original_theory()));
    assert_eq!(ordinary.atoms(), planned.atoms());
    assert_eq!(ordinary.theory().nodes(), planned.theory().nodes());
    assert_eq!(ordinary.theory().roots(), planned.theory().roots());
}

#[test]
fn logical_bounds_compare_selected_contributions() {
    for (source, explicit) in [
        ("#sum{word:#false}<=word.", ""),
        ("#sum+{-1:#false}<=word.", ""),
        ("#sum{:a}<=word.", "{a}."),
    ] {
        assert_eq!(native(&input(source)), native(&input(explicit)), "{source}");
    }
}

#[test]
fn logical_bounds_cannot_hide_undefined_arithmetic() {
    for source in ["#sum{1/0:a}<=word.", "{a}word:-Y=1/0."] {
        let error = limited(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn logical_guards_preserve_tuple_element_limits() {
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_elements = 1;
    assert!(limited("#count{1:a;1:b}<=word.", &limits).is_ok());
    let error = limited("#count{1:a;2:b}<=word.", &limits).unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::AggregateElements,
            limit: 1,
            observed: 2,
            ..
        }
    ));
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn grounding_limits_are_inclusive() {
    let source = "d(-1..1).#sum{X:p(X):d(X)}<=f(1).";
    for resource in [
        FormulaResource::AggregateElements,
        FormulaResource::Substitutions,
        FormulaResource::Work,
    ] {
        let limits = |ceiling| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::AggregateElements => {
                    limits.aggregate.max_elements = usize::try_from(ceiling).unwrap();
                }
                FormulaResource::Substitutions => limits.max_substitutions = ceiling,
                FormulaResource::Work => limits.max_work = ceiling,
                _ => unreachable!("selected grounding ceilings"),
            }
            limits
        };
        let mut lower = 0;
        let mut upper = 16_384;
        assert!(limited(source, &limits(upper)).is_ok());
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if limited(source, &limits(middle)).is_ok() {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(lower > 0);
        assert_eq!(
            native(&limited(source, &limits(lower)).unwrap()),
            native(&input(source))
        );
        let error = limited(source, &limits(lower - 1)).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, limit, observed, .. }
            if actual == resource && limit == u128::from(lower - 1) && observed == u128::from(lower)),
            "{error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
        println!("inclusive_{resource:?}={lower}");
    }
}

#[test]
fn preparation_payload_limit_is_inclusive() {
    let source = "d(word).{a}f(X):-d(X).";
    let limits = |ceiling| ExpansionLimits {
        max_scalar_bytes: ceiling,
        ..Default::default()
    };
    let prepare = |ceiling| {
        prepare_formula(
            source.into(),
            options(),
            limits(ceiling),
            FormulaLimits::default(),
        )
    };
    let mut lower = 0;
    let mut upper = 16_384;
    assert!(prepare(upper).is_ok());
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if prepare(middle).is_ok() {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(lower > 0);
    assert!(prepare(lower).is_ok());
    let error = prepare(lower - 1).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::ScalarBytes, limit, observed, ..
    }) if limit == (lower - 1) as u128 && observed == lower as u128),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    println!("inclusive_preparation_payload={lower}");
}

#[test]
fn local_witnesses_cannot_bind_logical_bounds() {
    for source in ["Y{a:Y=word}.", "#count{1:a:Y=word}<=Y."] {
        let error = limited(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn signed_permissions_survive_logical_bounds() {
    for source in [
        "#count{1:a;1:not a;2:not not a;3:#true}<=word.",
        "#sum{1:a;1:not a;2:not not a;0:#true}<=word.",
        "#sum+{1:a;1:not a;2:not not a;0:#true}<=word.",
        "{a;not a;not not a;#true}word.",
    ] {
        assert_eq!(native(&input(source)), expected(&[&[], &["a"]]));
    }
    assert_eq!(native(&input("a:-a.{not not a}word.")), expected(&[&[]]));
}

#[test]
fn frozen_guards_match_canonical_subset_formulas() {
    for (bound, truth) in [BOUNDS[0], BOUNDS[1]] {
        for (relation, truth) in RELATIONS.into_iter().zip(truth) {
            for measure in 0..4 {
                for context in 0..4 {
                    semantics::check(bound, relation, truth, measure, context);
                }
            }
        }
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn logical_term_classes_preserve_frozen_truth(
        bound in 0_usize..BOUNDS.len(), relation in 0_usize..RELATIONS.len(),
        measure in 0_usize..4, context in 0_usize..4,
    ) {
        semantics::check(BOUNDS[bound].0, RELATIONS[relation], BOUNDS[bound].1[relation], measure, context);
    }
}
