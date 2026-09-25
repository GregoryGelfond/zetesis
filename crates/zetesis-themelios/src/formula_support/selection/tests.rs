use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_core::{Atom, Predicate, Value, ValueNodeRef};

use super::*;
use crate::formula_support::{Support, SupportCatalog};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

#[test]
fn semantic_order_rebuilds_local_coordinates() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let mut selected = SourceSelection::new(&computation, &limits, &counters, location()).unwrap();
    let mut sources = Vec::new();
    for number in [3, 1, 2] {
        let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(number)]).unwrap();
        let source = computation
            .atom_ref((&atom).into(), &limits, &mut counters, location())
            .unwrap();
        selected
            .insert(
                &source,
                (FormulaResource::ProjectAtoms, 3),
                &computation,
                &limits,
                &mut counters,
                location(),
            )
            .unwrap();
        sources.push(source);
    }
    let selected = selected
        .order(&computation, &limits, &mut counters, location())
        .unwrap();
    for (source, expected) in sources.iter().zip([2, 0, 1]) {
        assert_eq!(
            selected
                .position(source, &limits, &mut counters, location())
                .unwrap(),
            Some(expected)
        );
    }
    let numbers: Vec<_> = (0..selected.len())
        .map(|local| {
            selected
                .atom(local, &computation, &limits, &mut counters, location())
                .unwrap()
                .values()
                .get(0)
                .unwrap()
                .descriptor()
        })
        .collect();
    assert_eq!(
        numbers,
        vec![
            ValueNodeRef::Number(1),
            ValueNodeRef::Number(2),
            ValueNodeRef::Number(3)
        ]
    );
}

#[test]
fn empty_sort_still_checks_the_source_scope() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut first = SupportCatalog::default();
    let mut second = SupportCatalog::default();
    let (first_rows, mut first_append) = first.split(&limits, &mut counters, location()).unwrap();
    let first_support = Support::indexed(&first_rows, &limits, &counters, location()).unwrap();
    let first_computation = Computation::new(&mut first_append, &first_support);
    let selected =
        SourceSelection::new(&first_computation, &limits, &counters, location()).unwrap();
    let (second_rows, mut second_append) =
        second.split(&limits, &mut counters, location()).unwrap();
    let second_support = Support::indexed(&second_rows, &limits, &counters, location()).unwrap();
    let second_computation = Computation::new(&mut second_append, &second_support);
    assert!(matches!(
        selected.order(&second_computation, &limits, &mut counters, location()),
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            ..
        })
    ));
}
