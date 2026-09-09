//! Unsigned Boolean elements retain activity without introducing producer atoms.

use std::collections::BTreeSet;

use super::{Models, native};
use themelios_base::source::SourceId;
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, AnalysisBasis, CountPlanLimits,
    CountPlanStatus, ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, ProfileFeature, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(191);

fn limited(source: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions {
            source_id: SOURCE,
            ..Default::default()
        },
        ExpansionLimits::default(),
        *limits,
    )
}

fn models(source: &str) -> Models {
    native(
        &limited(source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{source}: {error}")),
    )
}

fn expected(records: &[&[&str]]) -> Models {
    records
        .iter()
        .map(|atoms| atoms.iter().map(|atom| (*atom).to_owned()).collect())
        .collect()
}

#[test]
fn constants_do_not_introduce_atoms() {
    for source in [
        "{#true}.",
        "{#false}.",
        "2{#true;#true}2.",
        "1#count{1:#true}1.",
        "3#sum{1:#true;2:#true}3.",
        "1#min{1:#true}1.",
        "1#max{1:#true}1.",
    ] {
        let input = limited(source, &FormulaLimits::default()).unwrap();
        assert!(input.atoms().is_empty(), "{source}");
        assert_eq!(native(&input), Models::from([BTreeSet::new()]), "{source}");
    }
}

#[test]
fn boolean_choice_occurrences_have_distinct_keys() {
    assert_eq!(models("2{#true;#true}2."), expected(&[&[]]));
    assert!(models("2{a;a}2.").is_empty());
    assert_eq!(
        models("{a;b}.1{#true:a;#true:b}1."),
        expected(&[&["a"], &["b"]])
    );
}

#[test]
fn boolean_choice_witnesses_share_one_occurrence() {
    assert_eq!(
        models("d(1..2).1{#true:d(X)}1."),
        expected(&[&["d(1)", "d(2)"]])
    );
    assert!(models("d(1..2).2{#true:d(X)}2.").is_empty());
    assert_eq!(
        models("{d(1);d(2)}.1{#true:d(X)}1."),
        expected(&[&["d(1)"], &["d(2)"], &["d(1)", "d(2)"]])
    );
}

#[test]
fn pooling_preserves_boolean_occurrences() {
    assert_eq!(
        models("3{#true;#true;p(1;2)}3."),
        expected(&[&["p(1)"], &["p(2)"]])
    );
    assert_eq!(models("2{#true;#true}2:-X=(1;2)."), expected(&[&[]]));
}

#[test]
fn boolean_choice_analysis_declares_its_projection() {
    let input = limited("2{#true;#true}2.", &FormulaLimits::default()).unwrap();
    assert_eq!(input.analysis_basis(), AnalysisBasis::DependencyProjection);
    let tuple = limited("1#count{1:#true;1:#true}1.", &FormulaLimits::default()).unwrap();
    assert_eq!(tuple.analysis_basis(), AnalysisBasis::NormalizedProgram);
}

#[test]
fn complete_rules_keep_separate_occurrence_groups() {
    for source in [
        "1{#true}1.1{#true}1.",
        "2{#true;#true}2.2{#true;#true}2.",
        "d(1..2).1{#true:d(X)}1.1{#true:d(Y)}1.",
    ] {
        assert_eq!(models(source).len(), 1, "{source}");
    }
    for source in [
        "1{#true}1.1{#true;#true}1.",
        "1{#true;#true}1.1{#true}1.",
        "2{#true;#true}2.2{#true}2.",
    ] {
        assert!(models(source).is_empty(), "{source}");
    }
}

#[test]
fn boolean_choice_activity_uses_constant_truth() {
    assert_eq!(models("1{#true;a}1."), expected(&[&[]]));
    assert_eq!(models("1{#false;a}1."), expected(&[&["a"]]));
    assert_eq!(models("1{#true:#false;a}1."), expected(&[&["a"]]));
}

#[test]
fn boolean_bounds_cannot_supply_recursive_support() {
    for source in [
        "1{#true:a}1.",
        "a:-a.1{#true:a}1.",
        "1{#true:not not a}1.",
        "1{#true:a;b}1.a:-b.",
    ] {
        assert!(models(source).is_empty(), "{source}");
    }
    assert_eq!(models("{a}.1{#true:a}1."), expected(&[&["a"]]));
}

#[test]
fn explicit_boolean_tuples_coalesce_with_atom_rows() {
    assert_eq!(
        models("{a}.1#count{1:#true:a;1:b}1."),
        expected(&[&["a"], &["b"], &["a", "b"]])
    );
    assert!(models("2#sum{1:#true;1:#true}2.").is_empty());
    assert_eq!(models("1#count{1:#false;1:a}1."), expected(&[&["a"]]));
}

#[test]
fn boolean_extrema_retain_atom_permission() {
    assert_eq!(models("1#min{1:#true;2:a}1."), expected(&[&[], &["a"]]));
    assert_eq!(models("2#max{1:#true;2:a}2."), expected(&[&["a"]]));
}

fn refused(source: &str, expected: ProfileFeature) {
    let error = limited(source, &FormulaLimits::default()).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Admission(
        AdmissionFailure::Profile { feature, .. })) if feature == expected),
        "{source}: {error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn signed_boolean_elements_remain_refused() {
    for value in ["#true", "#false"] {
        for sign in ["not", "not not"] {
            refused(&format!("{{{sign} {value}}}."), ProfileFeature::NegatedHead);
            for function in ["#count", "#sum", "#sum+", "#min", "#max"] {
                refused(
                    &format!("0{function}{{1:{sign} {value}}}1."),
                    ProfileFeature::NegatedHead,
                );
            }
        }
    }
}

#[test]
fn false_boolean_heads_cannot_hide_weight_refusals() {
    for source in [
        "0#sum{word:#false}0:-#false.",
        "0#sum+{-1:#false}0:-#false.",
        "0#min{word:#false}0:-#false.",
        "0#max{: #false}0:-#false.",
    ] {
        refused(source, ProfileFeature::HeadAggregateWeight);
    }
}

#[test]
fn boolean_occurrence_limits_are_inclusive() {
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_elements = 2;
    assert!(limited("2{#true;#true}2.", &limits).is_ok());
    limits.aggregate.max_elements = 1;
    let error = limited("2{#true;#true}2.", &limits).unwrap_err();
    assert!(
        matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::AggregateElements,
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    assert!(limited("d(1..2).1{#true:d(X)}1.", &limits).is_ok());
}

#[test]
fn boolean_elements_preserve_other_count_certificates() {
    let source = "2{a;b;c;d}2.{a;b}1.{c;d}1.1{#true;e}1.";
    let ordinary = limited(source, &FormulaLimits::default()).unwrap();
    let planned = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(CountPlanLimits::default(), &Control::default(), None)
    .unwrap();
    let CountPlanStatus::Ready(plan) = planned.count_plan() else {
        panic!("the independent atom partition remains applicable");
    };
    assert_eq!(plan.consequence_count(), 2);
    assert!(planned.theory().same_instance(plan.original_theory()));
    assert_eq!(ordinary.atoms(), planned.atoms());
    assert_eq!(ordinary.theory().nodes(), planned.theory().nodes());
    assert_eq!(ordinary.theory().roots(), planned.theory().roots());
    assert_eq!(native(&ordinary), native(&planned));
}
