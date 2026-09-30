//! Reusable reported-display contracts; no solver or corpus is invoked.

use serde_json::json;
use zetesis_validation::answers::{self, Error, Issue, Limits, Resource};

fn report(displays: &[&[&str]]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "Result": if displays.is_empty() { "UNSATISFIABLE" } else { "SATISFIABLE" },
        "Models": { "More": "no", "Number": displays.len() },
        "Call": [{ "Witnesses": displays.iter().map(|display| json!({"Value":display})).collect::<Vec<_>>() }]
    })).unwrap()
}

#[test]
fn input_ceiling_is_checked_before_utf8() {
    let result = answers::clingo_json(&[255], Limits::for_bytes(0));
    assert!(matches!(
        result,
        Err(Error::Limit {
            resource: Resource::InputBytes,
            limit: 0,
            attempted: 1
        })
    ));
}

#[test]
fn admitted_non_utf8_has_a_typed_refusal() {
    assert!(matches!(
        answers::clingo_json(&[255], Limits::for_bytes(1)),
        Err(Error::Utf8(_))
    ));
}

#[test]
fn exact_input_ceiling_accepts_a_report() {
    let bytes = report(&[&["a"]]);
    assert!(answers::clingo_json(&bytes, Limits::for_bytes(bytes.len())).is_ok());
}

#[test]
fn witness_ceiling_counts_equal_empty_displays() {
    let bytes = report(&[&[], &[]]);
    let mut limits = Limits::for_bytes(bytes.len());
    limits.max_witnesses = 1;
    assert!(matches!(
        answers::clingo_json(&bytes, limits),
        Err(Error::Limit {
            resource: Resource::Witnesses,
            limit: 1,
            attempted: 2
        })
    ));
}

#[test]
fn symbol_ceiling_counts_occurrences_inside_a_display() {
    let bytes = report(&[&["a", "a"]]);
    let mut limits = Limits::for_bytes(bytes.len());
    limits.max_symbols = 1;
    assert!(matches!(
        answers::clingo_json(&bytes, limits),
        Err(Error::Limit {
            resource: Resource::Symbols,
            limit: 1,
            attempted: 2
        })
    ));
}

#[test]
fn zero_symbol_ceiling_preserves_empty_display_multiplicity() {
    let bytes = report(&[&[], &[]]);
    let mut limits = Limits::for_bytes(bytes.len());
    limits.max_symbols = 0;
    let answer = answers::clingo_json(&bytes, limits).unwrap();
    assert_eq!(answer.displays(), &[(Vec::new(), 2)]);
    assert_eq!(answer.model_count(), 2);
}

#[test]
fn witness_permutation_preserves_reported_display_agreement() {
    let first =
        answers::clingo_json(&report(&[&["b", "a", "a"], &["b"]]), Limits::default()).unwrap();
    let second =
        answers::clingo_json(&report(&[&["b"], &["a", "b", "a"]]), Limits::default()).unwrap();
    assert!(answers::same_displays(&first, &second));
}

#[test]
fn equal_distinct_displays_do_not_hide_different_model_multiplicities() {
    let first =
        answers::clingo_json(&report(&[&["a"], &["b"], &["b"]]), Limits::default()).unwrap();
    let second =
        answers::clingo_json(&report(&[&["a"], &["a"], &["b"]]), Limits::default()).unwrap();
    assert_eq!(first.model_count(), second.model_count());
    assert!(!answers::same_displays(&first, &second));
}

#[test]
fn incomplete_clingo_enumeration_has_a_typed_refusal() {
    let bytes = report(&[&["a"]]);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["Models"]["More"] = "yes".into();
    assert!(matches!(
        answers::clingo_json(&serde_json::to_vec(&value).unwrap(), Limits::default()),
        Err(Error::Invalid {
            issue: Issue::Incomplete,
            ..
        })
    ));
}

#[test]
fn raw_count_disagreement_is_a_contradiction() {
    let bytes = br#"{"Result":"SATISFIABLE","Models":{"More":"no","Number":2},"Call":[{"Witnesses":[{"Value":["a"]}]}]}"#;
    assert!(matches!(
        answers::clingo_json(bytes, Limits::default()),
        Err(Error::Invalid {
            issue: Issue::Contradiction,
            ..
        })
    ));
}

fn optimal() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "Result":"OPTIMUM FOUND", "Models":{"More":"no","Number":4,"Optimal":2,"Optimum":"yes","Costs":[-1,7]},
        "Call":[{"Witnesses":[
            {"Value":["worse"],"Costs":[0,0]},
            {"Value":[],"Costs":[-1,7]},
            {"Value":[],"Costs":[-1,7]},
            {"Value":[],"Costs":[-1,7]}
        ]}]
    })).unwrap()
}

#[test]
fn optn_removes_only_the_final_incumbent_discovery() {
    let answer = answers::clingo_json(&optimal(), Limits::default()).unwrap();
    assert_eq!(answer.displays(), &[(Vec::new(), 2)]);
    assert_eq!(answer.cost(), Some([-1, 7].as_slice()));
    assert_eq!(answer.model_count(), 2);
}

#[test]
fn raw_witness_limit_includes_discarded_incumbents() {
    let limits = Limits {
        max_witnesses: 3,
        ..Limits::default()
    };
    assert!(matches!(
        answers::clingo_json(&optimal(), limits),
        Err(Error::Limit {
            resource: Resource::Witnesses,
            limit: 3,
            attempted: 4
        })
    ));
}

#[test]
fn objective_dimensions_are_bounded_before_normalization() {
    let limits = Limits {
        max_cost_dimensions: 1,
        ..Limits::default()
    };
    assert!(matches!(
        answers::clingo_json(&optimal(), limits),
        Err(Error::Limit {
            resource: Resource::CostDimensions,
            limit: 1,
            attempted: 2
        })
    ));
}

#[test]
fn malformed_discarded_incumbent_cannot_pass() {
    let mut value: serde_json::Value = serde_json::from_slice(&optimal()).unwrap();
    value["Call"][0]["Witnesses"][0]["Costs"] = json!([0]);
    assert!(matches!(
        answers::clingo_json(&serde_json::to_vec(&value).unwrap(), Limits::default()),
        Err(Error::Invalid {
            issue: Issue::Contradiction,
            ..
        })
    ));
}

#[test]
fn cost_integer_boundaries_remain_exact() {
    let costs = answers::parse_costs(
        "-9223372036854775808 9223372036854775807",
        Limits::default(),
    )
    .unwrap();
    assert_eq!(costs, [i64::MIN, i64::MAX]);
}

#[test]
fn cost_integer_overflow_is_a_typed_refusal() {
    assert!(matches!(
        answers::parse_costs("9223372036854775808", Limits::default()),
        Err(Error::Invalid {
            issue: Issue::MalformedField,
            ..
        })
    ));
}

#[test]
fn display_tokenization_preserves_repeated_structural_spellings() {
    let display = answers::split_display(
        r#"p("a b,c",(1,2)) p("a b,c",(1,2))"#,
        false,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(display, [r#"p("a b,c",(1,2))"#, r#"p("a b,c",(1,2))"#]);
}

#[test]
fn zero_witness_limit_accepts_reported_unsatisfiability() {
    let limits = Limits {
        max_witnesses: 0,
        ..Limits::default()
    };
    let answer = answers::clingo_json(&report(&[]), limits).unwrap();
    assert!(!answer.satisfiable());
    assert_eq!(answer.model_count(), 0);
}
