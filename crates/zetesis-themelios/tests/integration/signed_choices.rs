//! Signed choice operands keep contribution identity separate from support.

use crate::support::finite_bindings as reference;
use crate::support::finite_bindings::expected;

use std::collections::BTreeSet;

use reference::{Models, exhaustive, external, native};
use zetesis_reference_support::formula;

const CASES: &[(&str, &[&[&str]])] = &[
    ("{not a}.", &[&[]]),
    ("{not not a}.", &[&[]]),
    ("1{not a}1.", &[&[]]),
    ("1{not not a}1.", &[]),
    ("a.1{a;not not a}1.", &[]),
    ("a.2{a;not not a}2.", &[&["a"]]),
    ("1{a;not a;not not a}1.", &[&[]]),
    ("1{not a;not a}1.", &[&[]]),
    ("2{not a;not a}2.", &[]),
    ("1{not #false;not #false}1.", &[]),
    ("2{not #false;not #false}2.", &[&[]]),
    ("2{#true;not not #true}2.", &[&[]]),
    ("1{not #true;not not #false}1.", &[]),
    ("a:-not not a.{not a}.", &[&[], &["a"]]),
    ("a:-a.1{not not a}1.", &[]),
    ("1{not a(1..2)}1.", &[]),
    ("2{not a(1..2)}2.", &[&[]]),
    ("2{not a(1;2)}2.", &[&[]]),
    ("d(1..2).1{not a:d(X)}1.", &[&["d(1)", "d(2)"]]),
    ("d(1..2).1{not #false:d(X)}1.", &[&["d(1)", "d(2)"]]),
    ("d(1).1{not a(X+1):d(X)}1.", &[&["d(1)"]]),
    ("{a;b}.1{not a:b;not a:not b}1.", &[&[], &["b"]]),
    (
        "{a;b}.1{not not a:b;not not a:not b}1.",
        &[&["a"], &["a", "b"]],
    ),
    ("1{not -a}1.", &[&[]]),
    ("-a.1{not not -a}1.", &[&["-a"]]),
    ("2{not #false}2.2{not #false;not #false}2.", &[]),
];

#[test]
fn complete_answers_preserve_signed_choice_contracts() {
    for &(source, records) in CASES {
        assert_eq!(native(&formula(source)), expected(records), "{source}");
    }
}

#[test]
fn signed_choices_match_exhaustive_reduct_checking() {
    for &(source, _) in CASES {
        let admitted = formula(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
#[ignore = "requires clingo: original signed sources match clingo"]
fn original_signed_sources_match_clingo() {
    for &(source, records) in CASES {
        let result = external(source, true);
        assert_eq!(result["Models"]["More"], "no");
        let mut actual = Models::new();
        for call in result["Call"].as_array().unwrap() {
            if let Some(witnesses) = call["Witnesses"].as_array() {
                for witness in witnesses {
                    let names: BTreeSet<String> = witness["Value"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|atom| atom.as_str().unwrap().to_owned())
                        .collect();
                    assert!(actual.insert(names), "duplicate full answer: {source}");
                }
            }
        }
        assert_eq!(actual, expected(records), "{source}");
        assert_eq!(native(&formula(source)), actual, "{source}");
    }
}
