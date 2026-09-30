//! Pure expression evaluation observes full models without extending logical grounding.

use crate::support::observation_reference;
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_reference_support::formula;
use zetesis_themelios::observation::{
    ConstructionLimits, ErrorKind, EvaluationError, Limits, Resource,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn terms(source: &str) -> Vec<String> {
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
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

#[test]
fn checked_integer_operators_construct_their_scalar_results() {
    assert_eq!(
        terms("p(2). #show. #show (X+3,X-3,X*3,7/X,7\\X,X**3,~X,X&3,X?4,X^3,|-X|):p(X)."),
        ["(5,-1,6,3,1,8,-3,2,6,1,2)"]
    );
}

#[test]
fn arithmetic_inside_constructors_preserves_structural_output() {
    assert_eq!(
        terms("p(2). #show. #show f(X+1,(-X,X*X)):p(X)."),
        ["f(3,(-2,4))"]
    );
}

#[test]
fn symbolic_negation_toggles_the_function_sign() {
    assert_eq!(
        terms("p(a). p(-b). p(f(1)). #show. #show -X:p(X)."),
        ["b", "-a", "-f(1)"]
    );
}

#[test]
fn comparison_chains_use_structural_term_order() {
    assert_eq!(
        terms("p(2). #show. #show x:p(X),1<X+1<f(0). #show y:not 1<2<0. #show z:not not 1<2<3."),
        ["x", "y", "z"]
    );
}

#[test]
fn boolean_default_negation_is_model_relative_truth() {
    assert_eq!(
        terms(
            "#show. #show a:#true. #show b:not #false. #show c:not not #true. #show d:#false. #show e:not #true."
        ),
        ["a", "b", "c"]
    );
}

#[test]
fn invalid_arithmetic_returns_the_pinned_cause() {
    for (expression, expected) in [
        ("1/0", EvaluationError::Undefined),
        ("1\\0", EvaluationError::Undefined),
        ("a+1", EvaluationError::Undefined),
        ("2**-1", EvaluationError::Undefined),
        ("2147483647+1", EvaluationError::Overflow),
        ("-(-2147483647-1)", EvaluationError::Overflow),
        ("|(-2147483647-1)|", EvaluationError::Overflow),
    ] {
        let input = formula(&format!("#show. #show {expression}."));
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
            &ErrorKind::Evaluation(expected),
            "{expression}"
        );
        assert!(error.location().is_some());
    }
}

#[test]
fn comparison_operands_share_the_construction_ceiling() {
    let input = formula("#show. #show x:1<2.");
    let construction = 4 * std::mem::size_of::<zetesis_themelios::observation::Symbol>();
    let run = |max_bytes| {
        input
            .metadata()
            .observations()
            .evaluate_with_construction_limits(
                &Model::default(),
                Limits::default(),
                ConstructionLimits { max_bytes },
                &Cancellation::default(),
            )
    };
    assert_eq!(run(construction).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(construction - 1).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::ConstructionBytes,
            ..
        }
    ));
}

#[test]
fn expression_observations_preserve_the_logical_theory() {
    let source = "p(2). {q}. #minimize {1@2:q}.";
    let plain = formula(source);
    let shown = formula(&format!("{source} #show. #show f(X+10):p(X),X*2>3."));
    assert_eq!(plain.atoms(), shown.atoms());
    assert_eq!(plain.theory().nodes(), shown.theory().nodes());
    assert_eq!(plain.theory().roots(), shown.theory().roots());
}

#[test]
fn expression_observations_preserve_source_priority_presence() {
    let source = "p(2). {q}. #minimize {1@2:q}.";
    let plain = formula(source);
    let shown = formula(&format!("{source} #show. #show f(X+10):p(X),X*2>3."));
    assert_eq!(
        plain.objectives().priorities(),
        shown.objectives().priorities()
    );
}

#[test]
fn directed_equalities_are_ordered_by_their_dependencies() {
    assert_eq!(
        terms("#show. #show f(X,Y,Z):X=Y+1,Z=f(X),Y=2."),
        ["f(3,2,f(3))"]
    );
}

#[test]
fn equality_binding_accepts_the_reversed_scalar_side() {
    assert_eq!(terms("#show. #show X:2=X."), ["2"]);
}

#[test]
fn generated_bindings_obey_the_live_local_payload_limit() {
    let input = formula("#show. #show X:X=2.");
    let run = |max_local_bytes| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_local_bytes,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
    };
    assert_eq!(run(16).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(15).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::LocalBytes,
            observed: 16,
            ..
        }
    ));
}

#[test]
fn completed_directives_release_their_owned_bindings() {
    let input = formula("#show. #show X:X=2. #show Y:Y=3.");
    let result = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits {
                max_local_bytes: 16,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result.symbols().len(), 2);
}

#[test]
fn circular_equalities_do_not_establish_a_finite_binding() {
    let result = admit_formula(
        "#show X:X=Y,Y=X.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(
        matches!(result, Err(zetesis_themelios::FormulaFailure::Observation { error })
        if error.kind() == &ErrorKind::Unsupported(zetesis_themelios::observation::Feature::UnsafeVariable))
    );
}

#[test]
fn nested_constructor_patterns_bind_their_subvalues() {
    assert_eq!(
        terms("p(f(1,g(2))). #show. #show (X,Y):p(f(X,g(Y)))."),
        ["(1,2)"]
    );
}
#[test]
fn tuple_patterns_enforce_repeated_named_slots() {
    assert_eq!(terms("p((1,1)).p((1,2)). #show. #show X:p((X,X))."), ["1"]);
}
#[test]
fn signed_constructor_patterns_retain_their_sign() {
    assert_eq!(terms("p(-f(1)).p(f(2)). #show. #show X:p(-f(X))."), ["1"]);
}
#[test]
fn anonymous_nested_patterns_do_not_equate_occurrences() {
    assert_eq!(terms("p(f(1,2)). #show. #show x:p(f(_,_))."), ["x"]);
}
#[test]
fn arithmetic_atom_arguments_wait_for_their_bindings() {
    assert_eq!(terms("p(2).q(1). #show. #show X:p(X+1),q(X)."), ["1"]);
}
#[test]
fn constructor_patterns_wait_for_embedded_arithmetic_inputs() {
    assert_eq!(
        terms("p(f(1,3)).q(2). #show. #show (X,Y):p(f(X,Y+1)),q(Y)."),
        ["(1,2)"]
    );
}
#[test]
fn default_negation_tests_nested_patterns_without_rebinding() {
    assert_eq!(
        terms("p(1).p(2).q(f(1)). #show. #show X:p(X),not q(f(X))."),
        ["2"]
    );
}
#[test]
fn failed_nested_matches_release_their_partial_bindings() {
    let input = formula("p(f(1,2)).p(f(3,3)). #show. #show X:p(f(X,X)).");
    let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
    let result = input
        .metadata()
        .observations()
        .evaluate(
            &model,
            Limits {
                max_local_bytes: 16,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        result.symbols(),
        &[zetesis_themelios::observation::Symbol::Number(3)]
    );
}

#[test]
fn finite_intervals_construct_each_integer_inclusively() {
    assert_eq!(terms("#show. #show 1..3."), ["1", "2", "3"]);
}
#[test]
fn pools_deduplicate_only_after_expanding_the_term_channel() {
    assert_eq!(terms("#show. #show (1;2;1)."), ["1", "2"]);
}
#[test]
fn constructors_take_the_cartesian_product_of_finite_arguments() {
    assert_eq!(
        terms("#show. #show f((1;2),(3;4))."),
        ["f(1,3)", "f(1,4)", "f(2,3)", "f(2,4)"]
    );
}
#[test]
fn an_empty_range_does_not_remove_other_pool_alternatives() {
    assert_eq!(terms("#show. #show (3..1;7;9..10)."), ["7", "9", "10"]);
}
#[test]
fn finite_ranges_bind_variables_in_dependency_order() {
    assert_eq!(
        terms("#show. #show (X,Y):Y=X+1,X=1..2."),
        ["(1,2)", "(2,3)"]
    );
}
#[test]
fn arithmetic_uses_each_selected_interval_value() {
    assert_eq!(terms("#show. #show (1..2)+3."), ["4", "5"]);
}
#[test]
fn finite_arguments_are_evaluated_in_observation_atoms() {
    assert_eq!(terms("p(2). #show. #show x:p(1..2)."), ["x"]);
}
#[test]
fn a_pool_comparison_enables_each_matching_source_expansion() {
    assert_eq!(terms("#show. #show x:1=(1;2)."), ["x"]);
}
#[test]
fn generated_alternatives_have_an_inclusive_payload_ceiling() {
    let input = formula("#show. #show (1;2).");
    let run = |max_local_bytes| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_local_bytes,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
    };
    assert_eq!(run(32).unwrap().symbols().len(), 2);
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
fn independent_directives_release_their_finite_alternatives() {
    let input = formula("#show. #show (1;2). #show (3;4).");
    let result = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits {
                max_local_bytes: 32,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result.symbols().len(), 4);
}

#[test]
fn pooled_atom_arities_keep_their_own_captures() {
    assert_eq!(terms("p(1).p(2,3). #show. #show X:p(X;X,3)."), ["1", "2"]);
}
#[test]
fn variables_used_only_in_pool_branches_need_no_common_export() {
    assert_eq!(terms("p(1).p(2,3). #show. #show x:p(X;Y,Z)."), ["x"]);
}
#[test]
fn a_pool_exports_only_variables_captured_in_every_branch() {
    for source in [
        "#show X:p(X;1).",
        "#show x:p(X;1),not q(X).",
        "#show x:p(X;1),#count{Y:q(X,Y)}>0.",
        "#show x:p(X;1),q(X):r.",
    ] {
        let result = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        assert!(
            matches!(result, Err(zetesis_themelios::FormulaFailure::Observation { error }) if error.kind() == &ErrorKind::Unsupported(zetesis_themelios::observation::Feature::UnsafeVariable)),
            "{source}"
        );
    }
}
#[test]
fn another_positive_binder_can_supply_a_missing_pool_capture() {
    assert_eq!(terms("p(1).q(2). #show. #show X:p(X;1),q(X)."), ["2"]);
}
#[test]
fn negation_is_applied_to_each_pooled_atom() {
    assert_eq!(
        terms("p(1). #show. #show x:not p(1;2). #show y:not not p(1;2)."),
        ["x", "y"]
    );
}
#[test]
fn empty_atom_alternatives_do_not_disable_other_pool_branches() {
    assert_eq!(
        terms(
            "p(1). #show. #show x:p((3..2);1). #show y:not p((3..2);2). #show z:not p((3..2);1)."
        ),
        ["x", "y"]
    );
}
#[test]
fn arithmetic_atom_filters_read_completed_structural_captures() {
    assert_eq!(
        terms("p(2,1).p(4,2).p(f(3),2). #show. #show X:p(X+1,X). #show Y:p(f(Y+1),Y)."),
        ["1", "2"]
    );
}
#[test]
fn pooled_arithmetic_filters_read_their_own_branch_captures() {
    assert_eq!(
        terms("p(2,1).p(2,3). #show. #show x:p(X+1,X;Y-1,Y)."),
        ["x"]
    );
}
#[test]
fn scalar_argument_ranges_can_depend_on_same_atom_captures() {
    assert_eq!(terms("p(3,2).p(5,1). #show. #show X:p(X..X+1,X)."), ["2"]);
}

#[test]
fn negated_pool_expansions_release_their_owned_alternatives() {
    let input = formula("p(1). #show. #show x:not p((1;2);3),not p((3;4);5).");
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
    assert_eq!(run(48).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(47).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::LocalBytes,
            observed: 48,
            limit: 47
        }
    ));
}
#[test]
fn failed_pool_rows_release_nested_capture_ownership() {
    let input = formula("p(f(1),0).p(f(2),1). #show. #show X:p(f(X),9;f(X),1).");
    let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
    let full = input
        .metadata()
        .observations()
        .render(
            &model,
            input.metadata().output(),
            Limits {
                max_local_bytes: 16,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(full.text(), "2");
}
#[test]
fn an_undefined_pool_branch_refuses_the_complete_observation() {
    let input = formula("#show. #show (1;1/0).");
    let failure = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(
        failure.kind(),
        &ErrorKind::Evaluation(EvaluationError::Undefined)
    );
    assert!(failure.location().is_some());
}

const ATOM_PATTERN_CASES: &[(&str, &[&str])] = &[
    ("p(1).#show.#show x:p((X;2)).", &["x"]),
    (
        "p(f(1)).p(g(2)).#show.#show x:p(f((X;2));g((Y;1))).",
        &["x"],
    ),
    (
        "p(f(1),2).p(f(2),1).#show.#show x:p(f((X;2)),(1;Y)).",
        &["x"],
    ),
    ("p(f(1,2)).p(f(3,4)).#show.#show X:p(f(X,(2;9))).", &["1"]),
    ("p((1,2)).#show.#show x:p(((X;2),(1;Y))).", &["x"]),
    ("p(-f(1)).#show.#show x:p(-f((X;2))).", &["x"]),
    ("p(f(2),1).#show.#show X:p(f((X+1;9)),X).", &["1"]),
    ("p(1).p(2).#show.#show N:N={p((X;2))}.", &["2"]),
    ("p(f(1)).p(f(2)).#show.#show N:N={p(f((X;2)))}.", &["2"]),
    ("p(1).p(2).#show.#show N:N={p(_)}.", &["2"]),
    ("p(f(1)).p(f(2)).#show.#show N:N={p(f(_))}.", &["2"]),
    ("p(1).p(1,2).#show.#show N:N={p(X;X,Y)}.", &["2"]),
    (
        "p(1).#show.#show N:N={p((X;2));not p(2);not not p(1)}.",
        &["3"],
    ),
];
#[test]
fn atom_pattern_cases_preserve_complete_displays() {
    for (source, expected) in ATOM_PATTERN_CASES {
        assert_eq!(terms(source), *expected, "{source}");
    }
}
#[test]
fn structural_pool_products_are_charged_before_materialization() {
    for (arity, pool, minimum) in [(32, "(1;2)", 1u128 << 32), (64, "(1;2;3;4)", u128::MAX)] {
        let arguments = vec![pool; arity].join(",");
        let result = admit_formula(
            format!("#show x:p({arguments})."),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        assert!(
            matches!(result, Err(zetesis_themelios::FormulaFailure::Observation { error }) if matches!(error.kind(), ErrorKind::Limit { resource: Resource::Nodes, observed, .. } if *observed >= minimum))
        );
    }
}
#[test]
#[ignore = "requires clingo: atom pattern cases match complete clingo displays; bounded structural-pool reference cases"]
fn atom_pattern_cases_match_complete_clingo_displays() {
    for (source, expected) in ATOM_PATTERN_CASES {
        observation_reference::compare(source, &serde_json::json!([expected]));
    }
}

#[test]
fn inverse_atom_patterns_select_the_complete_integer() {
    assert_eq!(terms("p(2). #show. #show X:p(X+1)."), ["1"]);
}

#[test]
fn anonymous_cardinality_patterns_apply_default_negation() {
    for source in [
        "#show. #show N:N={not p(_)}.",
        "p(1). #show. #show N:N={not not p(_)}.",
    ] {
        assert_eq!(terms(source), ["1"], "{source}");
    }
}

fn signed_projection_cases() -> impl Iterator<Item = serde_json::Value> {
    include_str!("../fixtures/observation-strong-anonymous.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
}
#[test]
fn anonymous_projection_keeps_strong_sign_separate_from_default_negation() {
    for case in signed_projection_cases() {
        let expected: Vec<_> = case["native"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(terms(case["source"].as_str().unwrap()), expected);
    }
}
#[test]
#[ignore = "requires clingo: clingo refuses anonymous strongly signed projections; explicit native extension with retained upstream refusal"]
fn clingo_refuses_anonymous_strongly_signed_projections() {
    for case in signed_projection_cases() {
        let run = observation_reference::run(case["source"].as_str().unwrap(), &[65]);
        let report = zetesis_clingo_support::json(&run);
        assert_eq!(report["Solver"], case["solver"]);
        assert_eq!(report["Result"], "UNKNOWN");
        assert_eq!(report["Models"]["More"], "yes");
        assert_eq!(report["Models"]["Number"], 0);
        assert!(
            zetesis_validation::answers::clingo_json(
                run.stdout(),
                zetesis_validation::answers::Limits::default()
            )
            .is_err()
        );
        let input = report["Input"][0].as_str().unwrap();
        assert_eq!(
            std::str::from_utf8(run.stderr())
                .unwrap()
                .replace(input, "-"),
            case["diagnostics"].as_str().unwrap()
        );
    }
}
