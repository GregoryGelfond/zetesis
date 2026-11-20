use zetesis_core::{Atom, Predicate, Value, ValueNodeRef};

use super::*;
use crate::formula_support::{Support, SupportCatalog};
use crate::test_support::location;

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

#[test]
fn insertion_work_is_linear_in_any_source_order() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let count = 4096_usize;
    let sources: Vec<_> = (0..i32::try_from(count).unwrap())
        .map(|number| {
            let atom =
                Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(number)]).unwrap();
            computation
                .atom_ref((&atom).into(), &limits, &mut counters, location())
                .unwrap()
        })
        .collect();
    let mut selected = SourceSelection::new(&computation, &limits, &counters, location()).unwrap();
    let before = counters.accounting.work;
    // Descending source positions: every insertion precedes all earlier ones.
    for source in sources.iter().rev() {
        let (_, new) = selected
            .insert(
                source,
                (FormulaResource::ProjectAtoms, count),
                &computation,
                &limits,
                &mut counters,
                location(),
            )
            .unwrap();
        assert!(new);
    }
    let spent = counters.accounting.work - before;
    assert!(
        spent < 64 * count as u64,
        "{spent} units for {count} insertions"
    );
    for (index, source) in sources.iter().enumerate() {
        assert_eq!(
            selected
                .position(source, &limits, &mut counters, location())
                .unwrap(),
            Some(count - 1 - index)
        );
    }
}
