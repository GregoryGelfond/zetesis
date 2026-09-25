//! Head membership preserves full identity without establishing body validity.
use std::collections::BTreeSet;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::{Atom, AtomPattern as InputPattern, Predicate, Sign, Term, Value};

use super::*;
use crate::ExpansionLimits;
use crate::formula_support::Context;

fn location() -> Location {
    Location {
        source: SourceId::new(3),
        span: Span::empty(ByteOffset::new(7)),
    }
}
fn pattern(sign: Sign) -> InputPattern {
    InputPattern::new(
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
            .insert(&atom, &limits, &mut counters, location())
            .unwrap();
    }
    let pattern = testing::admit_pattern(
        &mut catalog,
        &pattern(Sign::Negative),
        &mut counters,
        location(),
    );
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    for atom in delta {
        let discovered = computation
            .atom_ref(atom.into(), &limits, &mut counters, location())
            .unwrap();
        computation
            .support(&discovered, &limits, &mut counters, location())
            .unwrap();
    }
    let mut budget = Budget::new(
        ExpansionLimits {
            max_scalar_bytes: 0,
            ..ExpansionLimits::default()
        },
        0,
    );
    let empty = Binding::new(&computation, &limits, &mut counters, location()).unwrap();
    let mut join = Join::new(
        &[],
        &empty,
        1,
        &support,
        &mut budget,
        Context::new(&computation, &limits, &mut counters, location()),
    )
    .unwrap();
    if let Some(value) = value {
        let key = computation
            .import((&value).into(), &limits, &mut counters, location())
            .unwrap();
        join.values
            .set(0, &key, &limits, &mut counters, location())
            .unwrap();
    }
    join.already_derived(
        Some(pattern),
        &mut computation,
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
