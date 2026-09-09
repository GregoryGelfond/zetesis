//! Signed head activity is evaluated in the candidate without creating support.
//!
//! Expected source answers and finite M/J checks are independent of the source
//! compiler. Original-source clingo agreement is additional corroboration.

#[path = "support/head_element_reference.rs"]
mod reference;

use reference::{Selection, cost_records, expected, external, input, models};
use zetesis_themelios::{
    AdmissionOptions, CountPlanLimits, CountPlanStatus, ExpansionLimits, FormulaLimits,
    prepare_formula,
};

const CASES: &[(&str, &[&[&str]])] = &[
    ("{not a}.", &[&[]]),
    ("{not not a}.", &[&[]]),
    ("1{not a}1.", &[&[]]),
    ("1{not not a}1.", &[]),
    ("{a;not a;not not a}.", &[&[], &["a"]]),
    ("1{a;not a;not not a}1.", &[&[]]),
    ("2{a;not a;not not a}2.", &[&["a"]]),
    ("1{not a;not a}1.", &[&[]]),
    ("2{not a;not a}2.", &[]),
    ("{a}.1{a;not not a}1.", &[]),
    ("{a}.2{a;not not a}2.", &[&["a"]]),
    ("{a}.1{not a;not not a}1.", &[&[], &["a"]]),
    ("1{not #false}1.", &[&[]]),
    ("1{not #true}1.", &[]),
    ("1{not not #true}1.", &[&[]]),
    ("1{not not #false}1.", &[]),
    ("2{not #false;not not #true}2.", &[&[]]),
    ("1{not #false;not not #true}1.", &[]),
    ("2{not #false;not #false}2.", &[&[]]),
    ("a:-a.1{not not a}1.", &[]),
    ("a:-not not a.1{not not a}1.", &[&["a"]]),
    ("d(1..2).1{not a:d(X)}1.", &[&["d(1)", "d(2)"]]),
    ("d(1..2).2{not a:d(X)}2.", &[]),
    ("d(1..2).1{not #false:d(X)}1.", &[&["d(1)", "d(2)"]]),
    ("d(1..2).2{not #false:d(X)}2.", &[]),
    ("2{not #false;not #false}2.2{not #false}2.", &[]),
    ("1{not #false}1.1{not #false}1.", &[&[]]),
    ("1{not a}1:-#false.", &[&[]]),
    ("{a}.1#count{0:not a;0:not not a}1.", &[&[], &["a"]]),
    ("1#count{0:not #false;0:not not #true}1.", &[&[]]),
    ("2#count{0:not #false;0:not not #true}2.", &[]),
    ("1#count{0:not a;0:a}1.", &[&[], &["a"]]),
    ("1#count{0:not not a;0:a}1.", &[&["a"]]),
    ("a:-a.1#count{0:not not a}1.", &[]),
];

#[test]
fn signed_heads_preserve_complete_answers() {
    for &(source, records) in CASES {
        assert_eq!(models(source), expected(records), "{source}");
    }
}

#[test]
fn signed_constants_introduce_no_atoms() {
    for source in [
        "2{not #false;not not #true}2.",
        "1#count{0:not #false;0:not not #true}1.",
        "0#sum{0:not #false;0:not not #true}0.",
    ] {
        assert!(input(source).atoms().is_empty(), "{source}");
    }
}

#[test]
fn independent_unsigned_groups_keep_count_plans() {
    for source in [
        "2{a;b;c;d}2.{a;b}1.{c;d}1.1{not e}1.",
        "2{a;b;c;d}2.{a;b}1.{c;d}1.1#count{0:not e}1.",
    ] {
        let ordinary = input(source);
        let planned = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
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
            panic!("the independent unsigned atom partition remains applicable");
        };
        assert_eq!(plan.consequence_count(), 2);
        assert!(planned.theory().same_instance(plan.original_theory()));
        assert_eq!(planned.atoms(), ordinary.atoms());
        assert_eq!(planned.theory().nodes(), ordinary.theory().nodes());
        assert_eq!(planned.theory().roots(), ordinary.theory().roots());
    }
}

fn measured_sources() -> Vec<String> {
    ["#count", "#sum", "#sum+", "#min", "#max"]
        .into_iter()
        .flat_map(|function| {
            [
                format!("1{function}{{1:not a;1:a}}1."),
                format!("1{function}{{1:not not a;1:a}}1."),
                format!("1{function}{{1:not #false;1:not not #true}}1."),
                format!("{{a}}.1{function}{{1:not a;1:not not a}}1."),
            ]
        })
        .collect()
}

#[test]
fn shared_measures_keep_atomic_permission() {
    for sources in measured_sources().chunks_exact(4) {
        for (source, records) in sources.iter().zip([
            expected(&[&[], &["a"]]),
            expected(&[&["a"]]),
            expected(&[&[]]),
            expected(&[&[], &["a"]]),
        ]) {
            assert_eq!(models(source), records, "{source}");
        }
    }
}

fn objective_sources() -> Vec<String> {
    ["1{not a}1.", "1#count{0:not a;0:a}1."]
        .map(|head| format!("{{x;y}}.{head}#minimize{{1@3,k:x;0@1,l:y}}."))
        .into()
}

#[test]
fn independent_objectives_keep_scored_answers() {
    for (source, heads) in objective_sources()
        .into_iter()
        .zip([expected(&[&[]]), expected(&[&[], &["a"]])])
    {
        let mut records = std::collections::BTreeSet::new();
        for head in heads {
            for x in [false, true] {
                for y in [false, true] {
                    let mut atoms = head.clone();
                    if x {
                        atoms.insert("x".into());
                    }
                    if y {
                        atoms.insert("y".into());
                    }
                    records.insert((atoms, Some(vec![i64::from(x), 0])));
                }
            }
        }
        assert_eq!(cost_records(&source), records, "{source}");
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_signed_sources_match_clingo() {
    for &(source, _) in CASES {
        external(source);
    }
    for source in measured_sources().into_iter().chain(objective_sources()) {
        external(&source);
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn signed_measure_contexts_match_clingo() {
    // Change both head sign and measure while retaining recursive eligibility,
    // a shared complete tuple and independent possible producer support.
    for measure in 0..6 {
        for head in 0..12 {
            let selection = Selection {
                rows: vec![(1, 0, head, 0), (1, 0, head, 4), (2, 1, 0, 2)],
                lower: 1,
                upper: 2,
                body: head % 4,
                measure,
            };
            external(&selection.source());
        }
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]

    #[test]
    fn signed_elements_match_frozen_worlds(
        rows in proptest::collection::vec((-2_i32..=2, 0_u8..3, 0_u8..12, 0_u8..10), 1..7),
        lower in -3_i32..=3, upper in -3_i32..=3,
        body in 0_u8..4, measure in 0_u8..6,
    ) {
        let rows = rows.into_iter().map(|(value, key, head, condition)| {
            (if measure == 4 { value.abs() } else { value }, key, head, condition)
        }).collect();
        Selection { rows, lower, upper, body, measure }.check_frozen();
    }
}
