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

fn partial_probe(values: &[Option<Value>], bounded: bool) -> Result<bool, FormulaFailure> {
    let limits = FormulaLimits::default();
    let mut foreign = testing::Fixture::default();
    let binding = foreign.with(location(), |_, computation, counters| {
        testing::binding(values, computation, counters, location())
    });
    let mut fixture =
        testing::Fixture::from_atoms([atom(Sign::Positive, Value::Number(3))], location());
    let pattern = fixture.pattern(
        &InputPattern::new(
            Predicate::new("partial", 2).unwrap(),
            vec![Term::Variable(0), Term::Variable(1)],
        )
        .unwrap(),
        location(),
    );
    fixture.with(location(), |support, computation, counters| {
        let empty = Binding::new(computation, &limits, counters, location()).unwrap();
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut join = Join::new(
            &[],
            &empty,
            2,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        // A foreign frame would be rejected if the complete lookup were reached.
        join.values = binding;
        let mut checked = limits;
        if bounded {
            checked.max_support_bytes = 0;
            checked.theory.max_atoms = 0;
        }
        join.already_derived(Some(pattern), computation, &checked, counters, location())
    })
}

#[test]
fn partial_head_precedes_lookup_admission() {
    // Both an absent cell and an out-of-range head slot defer lookup. A prior
    // selected cell has foreign scope, and the lookup's owner ceilings are zero.
    for values in [
        vec![Some(Value::Number(7)), None],
        vec![Some(Value::Number(7))],
    ] {
        assert!(!partial_probe(&values, true).unwrap());
    }
}

#[test]
fn complete_head_reaches_scope_authentication() {
    assert!(matches!(
        partial_probe(&[Some(Value::Number(7)), Some(Value::Number(8))], false),
        Err(FormulaFailure::TermAssignment {
            error: zetesis_core::catalog::AssignmentError::Read(
                zetesis_core::catalog::ReadError::ForeignCatalog
            ),
            ..
        })
    ));
}
