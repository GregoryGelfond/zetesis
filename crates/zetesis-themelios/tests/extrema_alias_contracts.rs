//! Aliased numeric extrema retain tuple activity and separate head permissions.

#[path = "support/head_element_reference.rs"]
mod reference;

use reference::{Selection, expected, external, input, models};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    admit_formula,
};

const CASES: &[(&str, &[&[&str]])] = &[
    ("1#min{1:a;1:b}1.", &[&["a"], &["b"], &["a", "b"]]),
    ("1#max{0:a;1:a}1.", &[&["a"]]),
    ("0#min{0:a;0:b}0.", &[&["a"], &["b"], &["a", "b"]]),
    ("0#max{0:a;0:b}0.", &[&["a"], &["b"], &["a", "b"]]),
    ("-1#min{-1,k:a;0,l:a}-1.", &[&["a"]]),
    ("0#max{-1,k:a;0,l:a}0.", &[&["a"]]),
    ("0#min{0:a;0:b;0:a}0.", &[&["a"], &["b"], &["a", "b"]]),
    ("0#max{0:b;0:a;0:b}0.", &[&["a"], &["b"], &["a", "b"]]),
    ("0#min{0:a:a;0:b}0.", &[&["b"]]),
    ("0#max{0:a:a;0:b}0.", &[&["b"]]),
    ("0#min{0:a:not not a;0:b}0.", &[&["a"], &["b"], &["a", "b"]]),
    ("0#max{0:a:not not a;0:b}0.", &[&["a"], &["b"], &["a", "b"]]),
    ("{d}.0#min{0:a:d;0:b:not d}0.", &[&["a", "d"], &["b"]]),
    ("{d}.0#max{0:a:d;0:b:not d}0.", &[&["a", "d"], &["b"]]),
    ("1#min{0:a;0:b}1:-#false.", &[&[]]),
    ("1#max{0:a;0:b}1:-#false.", &[&[]]),
    ("#min{0:a;0:b}>0.", &[&[]]),
    ("#max{0:a;0:b}<0.", &[&[]]),
    ("#min{0:a;0:b}<0.", &[]),
    ("#max{0:a;0:b}>0.", &[]),
    (
        "{o}.0#min{0:a;0:b}0:-o.",
        &[&[], &["a", "o"], &["b", "o"], &["a", "b", "o"]],
    ),
    (
        "{o}.0#max{0:a;0:b}0:-not o.",
        &[&["o"], &["a"], &["b"], &["a", "b"]],
    ),
    (
        "d(-1;0).-1#min{X:a:d(X);X:b:d(X)}-1.",
        &[
            &["a", "d(-1)", "d(0)"],
            &["b", "d(-1)", "d(0)"],
            &["a", "b", "d(-1)", "d(0)"],
        ],
    ),
    (
        "d(-1;0).0#max{X:a:d(X);X:b:d(X)}0.",
        &[
            &["a", "d(-1)", "d(0)"],
            &["b", "d(-1)", "d(0)"],
            &["a", "b", "d(-1)", "d(0)"],
        ],
    ),
];

#[test]
fn aliases_preserve_complete_model_contracts() {
    for &(source, records) in CASES {
        assert_eq!(models(source), expected(records), "{source}");
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_alias_sources_match_clingo() {
    for &(source, _) in CASES {
        external(source);
    }
    println!("complete_original_sources={}", CASES.len());
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]

    #[test]
    fn aliases_match_independent_frozen_worlds(
        rows in proptest::collection::vec((-2_i32..=2, 0_u8..3, 0_u8..2, 0_u8..10), 1..7),
        lower in -3_i32..=3, upper in -3_i32..=3,
        body in 0_u8..4, measure in 0_u8..2,
    ) {
        Selection { rows, lower, upper, body, measure }.check_frozen();
    }

    #[test]
    fn duplicate_rows_preserve_alias_activity(
        rows in proptest::collection::vec((-2_i32..=2, 0_u8..3, 0_u8..2, 0_u8..10), 1..6),
        lower in -3_i32..=3, upper in -3_i32..=3,
        body in 0_u8..4, measure in 0_u8..2,
    ) {
        let mut selection = Selection { rows, lower, upper, body, measure };
        let before = models(&selection.source());
        selection.rows.extend(selection.rows.clone());
        selection.rows.reverse();
        proptest::prop_assert_eq!(models(&selection.source()), before);
    }
}

#[test]
fn alias_keys_own_the_element_budget() {
    for function in ["#min", "#max"] {
        let limits = FormulaLimits {
            aggregate: zetesis_ferraris::AggregateLimits {
                max_elements: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let shared = format!("0{function}{{0,k:a;0,k:b;0,k:a}}0.");
        let result = admit_formula(
            shared.clone(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .unwrap();
        assert_eq!(result.atoms(), input(&shared).atoms());
        let separate = format!("0{function}{{0,k:a;0,l:a}}0.");
        let error = admit_formula(
            separate,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Limit {
                    resource: FormulaResource::AggregateElements,
                    limit: 1,
                    observed: 2,
                    ..
                }
            ),
            "{error}"
        );
    }
}

#[test]
fn alias_expansion_observes_work_refusal() {
    let source = "0#min{0:a;0:b;0,k:a}0.";
    let limits = FormulaLimits {
        max_work: 0,
        ..Default::default()
    };
    let error = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::Work,
                limit: 0,
                observed: 1,
                ..
            }
        ),
        "{error}"
    );
}
