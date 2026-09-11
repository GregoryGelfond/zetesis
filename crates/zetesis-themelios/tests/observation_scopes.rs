//! Finite local query scopes over an already supplied complete model.

use zetesis_core::Model;
use zetesis_cpu::{Control, Stop};
use zetesis_themelios::observation::{ErrorKind, Feature, Limits, Resource};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

fn admit(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}
fn rendered(source: &str) -> String {
    let input = admit(source);
    let model = Model::new(input.atoms().iter().cloned());
    input
        .metadata()
        .observations()
        .render(
            &model,
            input.metadata().output(),
            Limits::default(),
            &Control::default(),
        )
        .unwrap()
        .text()
        .into()
}
#[test]
fn aggregate_count_deduplicates_the_complete_tuple() {
    assert_eq!(
        rendered("p(1).p(2).q(1). #show. #show x:#count{X:p(X);X:q(X)}=2."),
        "x"
    );
}
#[test]
fn aggregate_keys_retain_tuple_tail_identity() {
    assert_eq!(
        rendered("p(1). #show. #show x:#count{X,a:p(X);X,b:p(X)}=2."),
        "x"
    );
}
#[test]
fn sum_counts_each_distinct_enabled_tuple_once() {
    assert_eq!(
        rendered("p(2).q(2). #show. #show x:#sum{X,a:p(X);X,a:q(X);X,b:p(X)}=4."),
        "x"
    );
}
#[test]
fn positive_sum_ignores_nonpositive_numeric_contributions() {
    assert_eq!(
        rendered("p(-2).p(0).p(3). #show. #show x:#sum+{X:p(X)}=3."),
        "x"
    );
}
#[test]
fn extrema_use_the_full_structural_value_order() {
    assert_eq!(
        rendered("p(2).p(a).p(f(1)). #show. #show x:#min{X:p(X)}=2,#max{X:p(X)}=f(1)."),
        "x"
    );
}
#[test]
fn empty_aggregate_families_have_their_neutral_values() {
    assert_eq!(
        rendered("#show. #show x:#count{}=0,#sum{}=0,#min{}=#sup,#max{}=#inf."),
        "x"
    );
}
#[test]
fn aggregate_negation_tests_the_supplied_model() {
    assert_eq!(
        rendered("p(1). #show. #show x:not #count{X:p(X)}=0. #show y:not not #count{X:p(X)}=1."),
        "x y"
    );
}
#[test]
fn aggregate_local_names_do_not_escape_their_elements() {
    let result = admit_formula(
        "p(1).#show X:#count{X:p(X)}=1.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(
        matches!(result, Err(FormulaFailure::Observation { error }) if error.kind() == &ErrorKind::Unsupported(Feature::UnsafeVariable))
    );
}
#[test]
fn aggregate_elements_inherit_the_outer_value() {
    assert_eq!(
        rendered("p(1).p(2).q(1). #show. #show X:p(X),#count{X:q(X)}=1."),
        "1"
    );
}
#[test]
fn conditional_literal_is_universal_over_its_local_bindings() {
    assert_eq!(rendered("p(1).p(2).q(1). #show. #show x:q(X):p(X)."), "");
}
#[test]
fn conditional_literal_is_true_for_an_empty_local_family() {
    assert_eq!(rendered("#show. #show x:q(X):p(X)."), "x");
}
#[test]
fn conditional_literal_can_read_outer_values() {
    assert_eq!(
        rendered("p(1).p(2).q(1). #show. #show X:p(X),q(X):p(X)."),
        "1"
    );
}
#[test]
fn local_equalities_construct_aggregate_keys() {
    assert_eq!(
        rendered("p(2). #show. #show x:#count{Y:p(X),Y=f(X+1)}=1."),
        "x"
    );
}
#[test]
fn independent_aggregate_queries_release_their_keys() {
    let input = admit("p(1). #show. #show x:#count{X:p(X)}=1,#count{Y:p(Y)}=1.");
    let model = Model::new(input.atoms().iter().cloned());
    let result = input
        .metadata()
        .observations()
        .evaluate(
            &model,
            Limits {
                max_local_bytes: 32,
                ..Limits::default()
            },
            &Control::default(),
        )
        .unwrap();
    assert_eq!(result.symbols().len(), 1);
}
#[test]
fn aggregate_keys_obey_the_inclusive_local_payload_ceiling() {
    let input = admit("p(1). #show. #show x:#count{X:p(X)}=1.");
    let model = Model::new(input.atoms().iter().cloned());
    let run = |max_local_bytes| {
        input.metadata().observations().evaluate(
            &model,
            Limits {
                max_local_bytes,
                ..Limits::default()
            },
            &Control::default(),
        )
    };
    assert_eq!(run(32).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(31).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::LocalBytes,
            observed: 32,
            ..
        }
    ));
}
#[test]
fn local_substitutions_share_the_binding_ceiling() {
    let input = admit("p(1).p(2). #show. #show x:#count{X:p(X)}=2.");
    let model = Model::new(input.atoms().iter().cloned());
    let run = |max_bindings| {
        input.metadata().observations().evaluate(
            &model,
            Limits {
                max_bindings,
                ..Limits::default()
            },
            &Control::default(),
        )
    };
    assert_eq!(run(3).unwrap().statistics().bindings, 3);
    assert!(matches!(
        run(2).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::Bindings,
            observed: 3,
            ..
        }
    ));
}
#[test]
fn cancellation_refuses_the_whole_observation() {
    let input = admit("p(1). #show. #show x:#count{X:p(X)}=1.");
    let control = Control::default();
    control.cancel();
    let error = input
        .metadata()
        .observations()
        .evaluate(&Model::default(), Limits::default(), &control)
        .unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::Stopped(Stop::Cancelled));
}

#[test]
fn sums_ignore_defined_nonnumeric_heads() {
    assert_eq!(rendered("#show. #show x:#sum{a;2}=2,#sum+{a;-2;2}=2."), "x");
}
#[test]
fn sums_do_not_depend_on_intermediate_accumulation_order() {
    assert_eq!(
        rendered("#show. #show x:#sum{2147483647,a;2147483647,b;-2147483647,c;-2147483647,d}=0."),
        "x"
    );
}
#[test]
fn numeric_aggregate_guards_use_the_existing_wide_measure_contract() {
    assert_eq!(
        rendered("#show. #show x:#sum{2147483647,a;2147483647,b}>2147483647."),
        "x"
    );
}

#[test]
fn set_cardinality_keeps_distinct_default_negations() {
    assert_eq!(rendered("p. #show. #show x:2{p;not not p}=2."), "x");
}

fn references() -> impl Iterator<Item = serde_json::Value> {
    include_str!("fixtures/observation-scopes.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
}
fn reference_values(case: &serde_json::Value) -> Vec<String> {
    assert_eq!(case["witnesses"].as_array().unwrap().len(), 1);
    let mut values: Vec<_> = case["witnesses"][0]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect();
    values.sort();
    values
}
#[test]
fn scoped_queries_match_recorded_complete_reference_displays() {
    let mut checked = 0;
    for case in references().filter(|case| case["reference_difference"].is_null()) {
        let mut actual: Vec<_> = rendered(case["source"].as_str().unwrap())
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        actual.sort();
        assert_eq!(actual, reference_values(&case), "{}", case["source"]);
        checked += 1;
    }
    assert_eq!(checked, 15);
}
#[test]
fn wide_observation_measures_agree_with_original_formula_truth() {
    let source = "#sum{2147483647,a;2147483647,b;-1,c}>2147483647";
    let input = admit(&format!("ok:-{source}."));
    let candidate =
        zetesis_ferraris::Interpretation::new(input.theory(), 0..input.atoms().len()).unwrap();
    let original = zetesis_ferraris::check(
        input.theory(),
        &candidate,
        zetesis_ferraris::Limits::default(),
        &Control::default(),
    )
    .unwrap()
    .accepted();
    assert_eq!(
        rendered(&format!("#show. #show x:{source}.")) == "x",
        original
    );
}
#[test]
fn clingo_integer_wrapping_is_recorded_as_a_known_reference_difference() {
    let differences: Vec<_> = references()
        .filter(|case| !case["reference_difference"].is_null())
        .collect();
    assert_eq!(differences.len(), 1);
    let case = &differences[0];
    assert!(reference_values(case).is_empty());
    assert_eq!(
        rendered(case["source"].as_str().unwrap()),
        case["expected_native"][0].as_str().unwrap()
    );
}
#[test]
#[ignore = "requires an absolute CLINGO executable; bounded fresh complete references"]
fn unchanged_scoped_sources_match_fresh_clingo_evidence() {
    use std::ffi::OsString;
    use zetesis_validation::process::{
        Invocation, Limits as ProcessLimits, Stop as ProcessStop, invoke,
    };
    let executable = std::path::PathBuf::from(
        std::env::var_os("CLINGO").expect("set absolute CLINGO executable"),
    );
    let directory = tempfile::tempdir().unwrap();
    for case in references() {
        let input = directory.path().join("source.lp");
        std::fs::write(&input, case["source"].as_str().unwrap()).unwrap();
        let arguments = [
            input.into_os_string(),
            OsString::from("0"),
            OsString::from("--outf=2"),
        ];
        let outcome = invoke(
            Invocation {
                executable: &executable,
                arguments: &arguments,
                directory: directory.path(),
            },
            ProcessLimits::default(),
        )
        .unwrap();
        let (capture, pending) = outcome.into_parts();
        if let Some(pending) = pending {
            let cleanup = pending.retry(std::time::Duration::from_secs(1));
            panic!("clingo cleanup remained pending: {cleanup:?}");
        }
        assert_eq!(capture.stop(), ProcessStop::Completed);
        let actual: serde_json::Value = serde_json::from_slice(capture.stdout()).unwrap();
        assert_eq!(actual["Models"]["More"], "no");
        let witnesses: Vec<_> = actual["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().unwrap())
            .map(|witness| witness["Value"].clone())
            .collect();
        let fresh = serde_json::json!({"witnesses":witnesses});
        assert_eq!(
            reference_values(&fresh),
            reference_values(&case),
            "{}",
            case["source"]
        );
    }
}

#[test]
fn extremum_elements_require_a_measure_at_admission() {
    for function in ["#min", "#max"] {
        let result = admit_formula(
            format!("#show x:{function}{{:}}=0."),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        let Err(FormulaFailure::Observation { error }) = result else {
            panic!("missing extremum measure was admitted")
        };
        assert_eq!(
            error.kind(),
            &ErrorKind::Unsupported(Feature::AggregateMeasure)
        );
        assert!(error.location().is_some());
    }
}

#[test]
fn aggregate_tuple_pools_preserve_distinct_alternatives() {
    assert_eq!(rendered("#show. #show x:#count{(1;2)}=2."), "x");
}
#[test]
fn local_ranges_generate_the_aggregate_tuple_family() {
    assert_eq!(rendered("#show. #show x:#sum{X:X=1..3}=6."), "x");
}
#[test]
fn conditional_ranges_quantify_over_each_generated_binding() {
    assert_eq!(rendered("p(1).p(2). #show. #show x:p(X):X=1..2."), "x");
}

#[test]
fn aggregate_equalities_bind_the_actual_model_measure() {
    assert_eq!(rendered("p(1).p(2). #show. #show N:N=#count{X:p(X)}."), "2");
}
#[test]
fn scalar_assignments_can_depend_on_aggregate_results() {
    assert_eq!(
        rendered("p(1).p(2). #show. #show K:K=N+10,N=#count{X:p(X)}."),
        "12"
    );
}
#[test]
fn aggregate_result_dependencies_are_scheduled_before_their_consumers() {
    assert_eq!(
        rendered("p(1).p(2). #show. #show (N,S):S=#sum{X+N:p(X)},N=#count{X:p(X)}."),
        "(2,7)"
    );
}
#[test]
fn aggregate_results_can_supply_evaluated_atom_arguments() {
    assert_eq!(
        rendered("p(1).p(2).q(3). #show. #show N:q(N+1),N=#count{X:p(X)}."),
        "2"
    );
}
#[test]
fn structured_extrema_can_bind_an_observed_value() {
    assert_eq!(
        rendered("p(f(1)).p(f(2)). #show. #show X:X=#max{Y:p(Y)}."),
        "f(2)"
    );
}
#[test]
fn empty_extrema_bind_genuine_endpoint_symbols() {
    assert_eq!(
        rendered("#show. #show (X,Y):X=#min{},Y=#max{}."),
        "(#sup,#inf)"
    );
}
#[test]
fn circular_aggregate_result_dependencies_are_refused() {
    let result = admit_formula(
        "#show N:N=#count{X:p(X),X<N}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(
        matches!(result, Err(FormulaFailure::Observation { error }) if error.kind() == &ErrorKind::Unsupported(Feature::UnsafeVariable))
    );
}
#[test]
fn aggregate_bindings_check_the_pinned_scalar_width() {
    let input = admit("#show. #show N:N=#sum{2147483647,a;2147483647,b}.");
    let error = input
        .metadata()
        .observations()
        .evaluate(&Model::default(), Limits::default(), &Control::default())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Evaluation(zetesis_themelios::observation::EvaluationError::Overflow)
    );
}
#[test]
fn guarded_aggregate_bindings_keep_their_additional_comparisons() {
    assert_eq!(
        rendered("p(1).p(2). #show. #show N:N=#count{X:p(X)}<3."),
        "2"
    );
}
