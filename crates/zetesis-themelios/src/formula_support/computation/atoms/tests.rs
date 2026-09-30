use zetesis_core::catalog::TermRef;
use zetesis_core::{AtomPattern, Predicate, Term, Value, ValueNodeRef};

use super::*;
use crate::formula_support::{Support, SupportCatalog};
use crate::test_support::location;

#[test]
fn assigned_atoms_preserve_argument_occurrences() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let pattern = crate::formula_support::testing::admit_pattern(
        &mut catalog,
        &AtomPattern::new(
            Predicate::new("assigned", 3).unwrap(),
            vec![
                Term::Variable(0),
                Term::Constant(Value::Number(4)),
                Term::Variable(0),
            ],
        )
        .unwrap(),
        &mut counters,
        location(),
    );
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let mut binding = Binding::new(&computation, &limits, &mut counters, location()).unwrap();
    binding
        .extend_scope(1, &computation, &limits, &mut counters, location())
        .unwrap();
    let key = computation
        .number(7, &limits, &mut counters, location())
        .unwrap();
    binding
        .set(0, &key, &limits, &mut counters, location())
        .unwrap();
    let pattern = computation
        .static_pattern(pattern, &limits, &mut counters, location())
        .unwrap();
    let source = computation
        .atom(pattern, &binding, &limits, &mut counters, location())
        .unwrap();
    let atom = computation
        .source_atom(&source, &limits, &mut counters, location())
        .unwrap();
    let values: Vec<_> = atom.values().iter().map(TermRef::descriptor).collect();
    assert_eq!(
        values,
        vec![
            ValueNodeRef::Number(7),
            ValueNodeRef::Number(4),
            ValueNodeRef::Number(7)
        ]
    );
}

#[test]
fn discovered_heads_require_support_membership() {
    let limits = FormulaLimits::default();
    let mut fixture = crate::formula_support::testing::Fixture::default();
    let pattern = fixture.pattern(
        &AtomPattern::new(
            Predicate::new("selected", 1).unwrap(),
            vec![Term::Constant(Value::Number(4))],
        )
        .unwrap(),
        location(),
    );
    fixture.with(location(), |_, computation, counters| {
        let binding = Binding::new(computation, &limits, counters, location()).unwrap();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        assert!(
            !computation
                .contains_pattern(pattern, &binding, &limits, counters, location())
                .unwrap()
        );
        let source = computation
            .atom(pattern, &binding, &limits, counters, location())
            .unwrap();
        assert!(
            !computation
                .contains_pattern(pattern, &binding, &limits, counters, location())
                .unwrap()
        );
        computation
            .support(&source, &limits, counters, location())
            .unwrap();
        assert!(
            computation
                .contains_pattern(pattern, &binding, &limits, counters, location())
                .unwrap()
        );
    });
}
