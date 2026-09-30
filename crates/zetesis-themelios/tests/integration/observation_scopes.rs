//! Finite local query scopes over an already supplied complete model.

use crate::support::observation_reference;

use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_reference_support::formula;
use zetesis_themelios::observation::{AdmissionLimits, ErrorKind, Feature, Limits, Resource};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, admit_formula,
};

fn rendered(source: &str) -> String {
    let input = formula(source);
    let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
    input
        .metadata()
        .observations()
        .render(
            &model,
            input.metadata().output(),
            Limits::default(),
            &Cancellation::default(),
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
fn cardinality_keys_pay_their_complete_admission_cost() {
    // Each source has three outer nodes: output N, aggregate condition and guard
    // N. Each source set element costs one more. Every expanded key has one tuple
    // root, a template + number tag, and its key reference. Positive patterns pay
    // their existing bounded source/matching walks. Negative keys instead pay
    // their generated function root/children and the atom-pattern condition.
    // Each concrete argument now owns a tagged value: one traversal node plus
    // a tuple, tag template and tag symbol (four additional admitted nodes).
    for (source, nodes, expected) in [
        ("#show. #show N:N={}.", 3, "0"),
        ("#show. #show N:N=#count{1}.", 6, "1"),
        ("p. #show. #show N:N={p}.", 9, "1"),
        ("#show. #show N:N={not p}.", 10, "1"),
        ("p. #show. #show N:N={not not p}.", 10, "1"),
        ("-p. #show. #show N:N={-p}.", 9, "1"),
        ("#show. #show N:N={not -p}.", 10, "1"),
        ("-p. #show. #show N:N={not not -p}.", 10, "1"),
        ("p(f(1)). #show. #show N:N={p(f(1))}.", 14, "1"),
        ("#show. #show N:N={not p(f(1))}.", 17, "1"),
        ("p(1).p(2). #show. #show N:N={p(1;2)}.", 22, "2"),
        ("#show. #show N:N={not p(1;2)}.", 28, "2"),
        ("p(1).q(1). #show. #show N:N={p(X):q(X)}.", 18, "1"),
        ("p.r. #show. #show N:N={p;not q;not not r}.", 23, "3"),
        ("p. #show. #show N:N={p}. #show f(N):N={p}.", 19, "1 f(1)"),
    ] {
        let run = |max_nodes| {
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits {
                    observation: AdmissionLimits {
                        max_nodes,
                        ..AdmissionLimits::default()
                    },
                    ..FormulaLimits::default()
                },
            )
        };
        let input = run(nodes).unwrap_or_else(|error| panic!("{source}: {error}"));
        let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
        let shown = input
            .metadata()
            .observations()
            .render(
                &model,
                input.metadata().output(),
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(shown.text(), expected, "{source}");
        let Err(FormulaFailure::Observation { error }) = run(nodes - 1) else {
            panic!("{source}: every cardinality-key node must be charged");
        };
        assert_eq!(
            error.kind(),
            &ErrorKind::Limit {
                resource: Resource::Nodes,
                observed: u128::from(nodes),
                limit: u128::from(nodes - 1),
            },
            "{source}",
        );
        assert!(error.location().is_some());
    }
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
    let input = formula("p(1). #show. #show x:#count{X:p(X)}=1,#count{Y:p(Y)}=1.");
    let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
    let result = input
        .metadata()
        .observations()
        .evaluate(
            &model,
            Limits {
                max_local_bytes: 32,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result.symbols().len(), 1);
}
#[test]
fn aggregate_keys_obey_the_inclusive_local_payload_ceiling() {
    let input = formula("p(1). #show. #show x:#count{X:p(X)}=1.");
    let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
    let run = |max_local_bytes| {
        input.metadata().observations().evaluate(
            &model,
            Limits {
                max_local_bytes,
                ..Limits::default()
            },
            &Cancellation::default(),
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
    let input = formula("p(1).p(2). #show. #show x:#count{X:p(X)}=2.");
    let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
    let run = |max_bindings| {
        input.metadata().observations().evaluate(
            &model,
            Limits {
                max_bindings,
                ..Limits::default()
            },
            &Cancellation::default(),
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
    let input = formula("p(1). #show. #show x:#count{X:p(X)}=1.");
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = input
        .metadata()
        .observations()
        .evaluate(&Model::default(), Limits::default(), &cancellation)
        .unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::Stopped(Stop::Cancelled));
}

#[test]
fn sums_ignore_defined_nonnumeric_measures() {
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
    include_str!("../fixtures/observation-scopes.jsonl")
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
    let input = formula(&format!("ok:-{source}."));
    let candidate =
        zetesis_ferraris::Interpretation::new(input.theory(), 0..input.atoms().len()).unwrap();
    let original = zetesis_ferraris::check(
        input.theory(),
        &candidate,
        zetesis_ferraris::Limits::default(),
        &Cancellation::default(),
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
#[ignore = "requires clingo: unchanged scoped sources match fresh clingo evidence; bounded fresh complete references"]
fn unchanged_scoped_sources_match_fresh_clingo_evidence() {
    for case in references() {
        observation_reference::compare(case["source"].as_str().unwrap(), &case["witnesses"]);
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
    let input = formula("#show. #show N:N=#sum{2147483647,a;2147483647,b}.");
    let error = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits::default(),
            &Cancellation::default(),
        )
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

#[test]
fn aggregate_secondary_guards_can_read_the_assigned_result() {
    for (guard, shown) in [
        ("=N", true),
        ("<N+1", true),
        (">N+1", false),
        ("!=N", false),
    ] {
        assert_eq!(
            rendered(&format!(
                "p(1).p(2). #show. #show N:N=#count{{X:p(X)}}{guard}."
            )),
            if shown { "2" } else { "" },
            "{guard}"
        );
    }
}

#[test]
fn aggregate_equalities_can_assign_both_guard_variables() {
    assert_eq!(
        rendered("p(1).p(2). #show. #show (N,M):N=#count{X:p(X)}=M."),
        "(2,2)"
    );
}
#[test]
fn aggregate_guard_dependencies_can_be_supplied_after_measure_binding() {
    assert_eq!(
        rendered("p(1).p(2). #show. #show N:N=#count{X:p(X)}<K,K=N+1."),
        "2"
    );
}
#[test]
fn set_cardinality_keys_follow_their_enabled_pool_alternative() {
    for facts in ["p(1).", "p(2)."] {
        assert_eq!(
            rendered(&format!("{facts} #show. #show x:1{{p((1;2))}}1.")),
            "x"
        );
    }
}
#[test]
fn negative_set_keys_follow_their_absent_pool_alternative() {
    assert_eq!(rendered("p(1). #show. #show x:1{not p((1;2))}1."), "x");
}
#[test]
fn mixed_arity_set_keys_preserve_complete_atom_identity() {
    assert_eq!(rendered("p(1).p(1,2). #show. #show x:2{p(1;1,2)}2."), "x");
}
#[test]
fn pooled_consequents_are_alternatives_inside_each_local_substitution() {
    assert_eq!(
        rendered("p(1).p(2).q(2). #show. #show x:q(X;X+1):p(X)."),
        "x"
    );
}

#[test]
fn matched_atom_keys_share_the_live_scope_ceiling() {
    for count in 1..=5 {
        use std::fmt::Write as _;
        let mut facts = String::new();
        for value in 1..=count {
            write!(facts, "p({value}).").unwrap();
        }
        let input = formula(&format!("{facts} #show. #show N:N={{p((X;10))}}."));
        let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
        // The binding and each retained p(number) key charge two semantic
        // nodes plus the one-byte predicate name. Default negation belongs
        // to typed key metadata; it does not fabricate an ASP tag/tuple value.
        let atom_key_bytes = 2 * 16 + 1;
        let exact = atom_key_bytes * (count + 1);
        let run = |max_local_bytes| {
            input.metadata().observations().evaluate(
                &model,
                Limits {
                    max_local_bytes,
                    ..Limits::default()
                },
                &Cancellation::default(),
            )
        };
        assert_eq!(
            run(exact).unwrap().symbols(),
            &[zetesis_themelios::observation::Symbol::Number(
                i32::try_from(count).unwrap()
            )]
        );
        assert!(matches!(
            run(exact - 1).unwrap_err().kind(),
            ErrorKind::Limit {
                resource: Resource::LocalBytes,
                ..
            }
        ));
    }
}
#[test]
fn matched_atom_key_construction_checks_child_depth() {
    let input = formula("p(1). #show N:N={p(_)}.");
    let failure = input
        .metadata()
        .observations()
        .evaluate(
            &Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap(),
            Limits {
                max_symbol_depth: 1,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap_err();
    assert!(matches!(
        failure.kind(),
        ErrorKind::Limit {
            resource: Resource::Depth,
            observed: 2,
            limit: 1
        }
    ));
}
