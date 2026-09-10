//! Source normalization precedes affine binding without replacing checked evaluation.

#[path = "support/finite_bindings.rs"]
mod reference;

use reference::{Models, exhaustive, native};
use themelios_base::source::SourceId;
use themelios_program::term::EvalError;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, FormulaResource, admit_formula,
};
use zetesis_validation::answers;

struct Case {
    source: &'static str,
    models: &'static [&'static [&'static str]],
}

const RIGHT_FACTORS: Case = Case {
    source: "p(X,Y):-0<X*2<Y*3<7.",
    models: &[&["p(1,1)", "p(1,2)", "p(2,2)"]],
};
const CLOSED_FACTORS: Case = Case {
    source: "p(X,Y):-(3-3)<X*(1+1)<Y*|~(-4)|<(2**3-1).",
    models: &[&["p(1,1)", "p(1,2)", "p(2,2)"]],
};
const BOUND_OFFSET: Case = Case {
    source: "p(X,Y):-Y=1,0<X+Y<3.",
    models: &[&["p(0,1)", "p(1,1)"]],
};
const ABSOLUTE_FILTER: Case = Case {
    source: "p(X,Y):-0<X<Y<3,|X|<2.",
    models: &[&["p(1,2)"]],
};
const COMPLEMENT_FILTER: Case = Case {
    source: "p(X,Y):-0<X<Y<3,~X=(-2).",
    models: &[&["p(1,2)"]],
};
const BOUND_PRODUCT: Case = Case {
    source: "p(X,Y):-X=1..2,Y=1..2,X*Y<3.",
    models: &[&["p(1,1)", "p(1,2)", "p(2,1)"]],
};
const LOCAL_CHOICE: Case = Case {
    source: "1{p(X,Y):0<X*2<Y*3<4}1.",
    models: &[&["p(1,1)"]],
};
const LOCAL_CONDITIONAL: Case = Case {
    source: "{p(1,1)}.q:-p(X,Y):0<X*2<Y*3<4.",
    models: &[&[], &["p(1,1)", "q"]],
};
const POOLED_BINDING: Case = Case {
    source: "p(X,Y):-Y=(1+0;1+1),0<X<Y+1<4.",
    models: &[&["p(1,1)", "p(1,2)", "p(2,2)"]],
};
const INTERVAL_BINDING: Case = Case {
    source: "p(X,Y):-Y=1..(1+1),0<X<Y+1<4.",
    models: &[&["p(1,1)", "p(1,2)", "p(2,2)"]],
};
const CONSEQUENT_INTERVAL: Case = Case {
    source: "{p(1);p(2)}.q:-p(2*(1..2)-2):0<X<Y<3.",
    models: &[&[], &["p(1)"], &["p(2)", "q"], &["p(1)", "p(2)", "q"]],
};

fn admit(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions {
            source_id: SourceId::new(29),
            ..AdmissionOptions::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}

fn expected(case: &Case) -> Models {
    case.models
        .iter()
        .map(|atoms| atoms.iter().map(|atom| (*atom).to_owned()).collect())
        .collect()
}

fn assert_models(case: &Case) {
    let input = admit(case.source).unwrap_or_else(|error| panic!("{}: {error}", case.source));
    let expected = expected(case);
    assert_eq!(native(&input), expected, "{}", case.source);
    assert_eq!(exhaustive(&input), expected, "{}", case.source);
}

#[test]
fn right_constant_factors_preserve_integer_solutions() {
    assert_models(&RIGHT_FACTORS);
}

#[test]
fn closed_arithmetic_factors_preserve_solutions() {
    assert_models(&CLOSED_FACTORS);
}

#[test]
fn a_closed_binding_supplies_an_offset_envelope() {
    assert_models(&BOUND_OFFSET);
}

#[test]
fn absolute_values_filter_independent_bindings() {
    assert_models(&ABSOLUTE_FILTER);
}

#[test]
fn complements_filter_independent_bindings() {
    assert_models(&COMPLEMENT_FILTER);
}

#[test]
fn bound_products_use_the_original_guard() {
    assert_models(&BOUND_PRODUCT);
}

#[test]
fn local_choices_receive_normalized_factors() {
    assert_models(&LOCAL_CHOICE);
}

#[test]
fn conditional_scopes_receive_normalized_factors() {
    assert_models(&LOCAL_CONDITIONAL);
}

#[test]
fn pooled_bindings_retain_normalized_alternatives() {
    assert_models(&POOLED_BINDING);
}

#[test]
fn interval_bindings_retain_normalized_endpoints() {
    assert_models(&INTERVAL_BINDING);
}

#[test]
fn consequent_intervals_retain_their_private_slots() {
    assert_models(&CONSEQUENT_INTERVAL);
}

fn assert_unsafe(source: &str) {
    let error = admit(source).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::UnsafeVariable { location, .. }
            if location.source == SourceId::new(29)),
        "{source}: {error}"
    );
}

#[test]
fn absolute_values_do_not_establish_affine_bindings() {
    assert_unsafe("p(X):-0<|X|<3.");
}

#[test]
fn complements_do_not_establish_affine_bindings() {
    assert_unsafe("p(X):-0<~X<3.");
}

#[test]
fn bound_variables_are_not_closed_source_factors() {
    // A binding environment can evaluate Y, but the affine reader has no such
    // environment: it must not treat a safe slot as a closed source constant.
    assert_unsafe("p(X,Y):-Y=2,0<X*Y<5.");
}

#[test]
fn relational_bindings_are_not_closed_endpoints() {
    assert_unsafe("d(1).p(X,Y):-d(Y),0<X+(Y+0)<3.");
}

#[test]
fn cancellation_does_not_create_closed_factors() {
    assert_unsafe("p(X,Y):-X=1,0<(X-X+1)*Y<3.");
}

fn assert_capacity(source: &str) {
    let error = admit(source).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Limit {
            resource: FormulaResource::BindingCoefficientBits,
            limit: 64,
            observed: 94,
            location,
        } if location.source == SourceId::new(29)),
        "{source}: {error}"
    );
    let location = error.diagnostics()[0].primary().location;
    assert!(
        source[location.span.start().get() as usize..location.span.end().get() as usize]
            .contains("p(")
    );
}

#[test]
fn negative_coefficient_capacity_remains_explicit() {
    assert_capacity("p(X):-0<(-2147483647)*(2147483647*(2147483647*X))<2.");
}

#[test]
fn intercept_capacity_remains_explicit() {
    assert_capacity("p(X):-0<2147483647*(2147483647*(X+2147483647))<2.");
}

fn assert_evaluation(source: &str, expected: &EvalError) {
    let error = admit(source).unwrap_err();
    assert!(
        matches!(&error, FormulaFailure::Expansion(
            ExpansionFailure::Evaluation { error, location }
        ) if error == expected && location.source == SourceId::new(29)),
        "{source}: {error}"
    );
}

#[test]
fn closed_undefined_arithmetic_is_not_binding_failure() {
    assert_evaluation("p(X):-0<X<(1/0).", &EvalError::Undefined);
}

#[test]
fn closed_overflow_is_not_binding_failure() {
    assert_evaluation("p(X):-0<X<(2147483647+1).", &EvalError::Overflow);
}

#[test]
fn closed_absolute_overflow_remains_a_source_error() {
    assert_evaluation("p(X):-X=0,|(-2147483647-1)|>X.", &EvalError::Overflow);
}

#[test]
fn failed_guards_do_not_hide_closed_errors() {
    assert_evaluation("p(X):-0<X<3,0=1,1/0=0.", &EvalError::Undefined);
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_normalized_sources_match_complete_clingo() {
    // Only the admitted semantic fixtures belong here. Deliberate affine
    // profile refusals and checked overflow policies are independent contracts.
    for case in [
        &RIGHT_FACTORS,
        &CLOSED_FACTORS,
        &BOUND_OFFSET,
        &ABSOLUTE_FILTER,
        &COMPLEMENT_FILTER,
        &BOUND_PRODUCT,
        &LOCAL_CHOICE,
        &LOCAL_CONDITIONAL,
        &POOLED_BINDING,
        &INTERVAL_BINDING,
        &CONSEQUENT_INTERVAL,
    ] {
        let record = reference::external(case.source, true);
        let bytes = serde_json::to_vec(&record).unwrap();
        let report = answers::clingo_json(&bytes, answers::Limits::default())
            .expect("complete original-source output");
        assert!(report.cost().is_none());
        let actual: Models = report
            .displays()
            .iter()
            .map(|(atoms, multiplicity)| {
                assert_eq!(*multiplicity, 1, "full atoms distinguish each answer");
                atoms.iter().cloned().collect()
            })
            .collect();
        assert_eq!(actual, expected(case), "{}", case.source);
        assert_eq!(
            native(&admit(case.source).unwrap()),
            actual,
            "{}",
            case.source
        );
    }
}
