//! Head membership preserves full identity without establishing body validity.
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::{Predicate, Sign, Term};

use super::*;
use crate::ExpansionLimits;

fn location() -> Location {
    Location {
        source: SourceId::new(3),
        span: Span::empty(ByteOffset::new(7)),
    }
}
fn pattern(sign: Sign) -> AtomPattern {
    AtomPattern::new(
        Predicate::with_sign("p", 2, sign).unwrap(),
        vec![Term::Variable(0), Term::Variable(0)],
    )
    .unwrap()
}
fn atom(sign: Sign, value: Value) -> Atom {
    Atom::new(
        pattern(sign).predicate().clone(),
        vec![value.clone(), value],
    )
    .unwrap()
}
fn probe(existing: Vec<Atom>, delta: &BTreeSet<Atom>, value: Option<Value>) -> bool {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    for atom in existing {
        catalog = catalog
            .insert(atom, &limits, &mut counters, location())
            .unwrap();
    }
    let support = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let mut budget = Budget::new(
        ExpansionLimits {
            max_scalar_bytes: 0,
            ..ExpansionLimits::default()
        },
        0,
    );
    let mut join = Join::new(
        &[],
        &Binding::default(),
        1,
        &support,
        &mut budget,
        location(),
    )
    .unwrap();
    join.values[0] = value;
    join.already_derived(
        Some((&pattern(Sign::Negative), delta)),
        &limits,
        &mut counters,
        location(),
    )
    .unwrap()
}
#[test]
fn head_membership_finds_prior_support() {
    let value = Value::String("large typed payload".repeat(100));
    assert!(probe(
        vec![atom(Sign::Negative, value.clone())],
        &BTreeSet::new(),
        Some(value)
    ));
}
#[test]
fn head_membership_finds_current_delta() {
    let value = Value::String("large typed payload".repeat(100));
    assert!(probe(
        vec![],
        &BTreeSet::from([atom(Sign::Negative, value.clone())]),
        Some(value)
    ));
}
#[test]
fn head_membership_keeps_predicate_sign() {
    assert!(!probe(
        vec![atom(Sign::Positive, Value::Number(2))],
        &BTreeSet::new(),
        Some(Value::Number(2))
    ));
}
#[test]
fn head_membership_keeps_value_type() {
    assert!(!probe(
        vec![],
        &BTreeSet::from([atom(Sign::Negative, Value::String("2".into()))]),
        Some(Value::Number(2))
    ));
}
#[test]
fn absent_head_input_defers_membership() {
    assert!(!probe(
        vec![atom(Sign::Negative, Value::Number(0))],
        &BTreeSet::new(),
        None
    ));
}
