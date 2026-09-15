//! A fixed projection domain uses complete scoped source activity.

use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn domain(source: &str) -> usize {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
    .projection()
    .atoms()
    .len()
}

#[test]
fn domain_order_applies_the_complete_permutation() {
    let input = admit_formula(
        r"d(1..3).p(0..2).#project p((X+1)\3):d(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let values: Vec<_> = input
        .projection()
        .atoms()
        .iter()
        .map(|atom| atom.values()[0].clone())
        .collect();
    assert_eq!(
        values,
        (0..3).map(zetesis_core::Value::Number).collect::<Vec<_>>()
    );
}

#[test]
fn rich_guards_read_required_nested_dependencies() {
    for source in [
        "q.{p}.#project p:0=#count{1:q}.",
        "q.{p}.#project p:0=#sum{1:q}.",
        "d(1).q.{p}.#project p:not q:d(X).",
        "q(1).{p}.#project p:not q(_).",
    ] {
        assert_eq!(domain(source), 0, "{source}");
    }
}

#[test]
fn rich_guards_preserve_optional_nested_dependencies() {
    for source in [
        "{q;p}.#project p:0=#count{1:q}.",
        "{q;p}.#project p:0=#sum{1:q}.",
        "d(1).{q;p}.#project p:not q:d(X).",
        "{q(1);p}.#project p:not q(_).",
    ] {
        assert_eq!(domain(source), 1, "{source}");
    }
}
