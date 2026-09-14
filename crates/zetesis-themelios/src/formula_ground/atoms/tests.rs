//! Shared atom identity, first-occurrence order and located refusal contracts.

#[path = "count_capture.rs"]
mod count_capture;

use std::collections::BTreeMap;
use std::error::Error;

use proptest::prelude::*;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::atom_interner::Limits;
use zetesis_core::{Atom, AtomPattern, Predicate, Sign, Term, Value, ValueLimits, ValueNode};

use super::Catalog;
use crate::formula_support::Counters;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

fn atom(name: &str, sign: Sign, values: Vec<Value>) -> Atom {
    Atom::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values,
    )
    .unwrap()
}
fn number(value: i32) -> Atom {
    atom("p", Sign::Positive, vec![Value::Number(value)])
}
fn location() -> Location {
    Location {
        source: SourceId::new(7),
        span: Span::empty(ByteOffset::new(3)),
    }
}
fn bound() -> (FormulaResource, usize) {
    (FormulaResource::Atoms, 1024)
}

fn intern(catalog: &mut Catalog, atom: &Atom) -> usize {
    let pattern = AtomPattern::new(
        atom.predicate().clone(),
        atom.values().iter().cloned().map(Term::Constant).collect(),
    )
    .unwrap();
    let mut counters = Counters::default();
    let limits = FormulaLimits::default();
    catalog
        .entry(
            pattern.key(&[] as &[Value]).unwrap(),
            bound(),
            &mut counters,
            &limits,
            location(),
        )
        .unwrap()
        .insert_with(Limits::for_atoms(bound().1), || {
            counters.work(&limits, location())
        })
        .map_err(|error| super::failure(error, bound(), location()))
        .unwrap()
}
fn find(catalog: &Catalog, atom: &Atom) -> Option<usize> {
    catalog
        .find(
            atom,
            bound(),
            &mut Counters::default(),
            &FormulaLimits::default(),
            location(),
        )
        .unwrap()
}
fn commit(catalog: &mut Catalog) {
    catalog
        .commit(
            bound(),
            &mut Counters::default(),
            &FormulaLimits::default(),
            location(),
        )
        .unwrap();
}
fn finish(catalog: Catalog) -> Vec<Atom> {
    catalog
        .into_atoms(
            bound(),
            &mut Counters::default(),
            &FormulaLimits::default(),
            location(),
        )
        .unwrap()
}

#[test]
fn shared_index_preserves_complete_atom_identity() {
    let tuple = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
        ValueLimits::default(),
    )
    .unwrap();
    let atoms = vec![
        number(1),
        atom("p", Sign::Negative, vec![Value::Number(1)]),
        atom("p", Sign::Positive, vec![Value::String("1".into())]),
        atom("p", Sign::Positive, vec![Value::Symbol("1".into())]),
        atom("q", Sign::Positive, vec![Value::Number(1)]),
        atom("p", Sign::Positive, vec![]),
        atom("p", Sign::Positive, vec![tuple]),
    ];
    let mut catalog = Catalog::default();
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(intern(&mut catalog, atom), id);
    }
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(find(&catalog, atom), Some(id));
    }
    assert_eq!(find(&catalog, &number(2)), None);
    assert_eq!(finish(catalog), atoms);
}

#[test]
fn growth_preserves_first_occurrence_order() {
    let atoms: Vec<_> = (0..257).rev().map(number).collect();
    let mut catalog = Catalog::default();
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(intern(&mut catalog, atom), id);
        if id % 31 == 0 {
            commit(&mut catalog);
        }
    }
    for (id, atom) in atoms.iter().enumerate().rev() {
        assert_eq!(find(&catalog, atom), Some(id));
    }
    assert_eq!(finish(catalog), atoms);
}

#[test]
fn exhausted_lookup_retains_its_source_cause() {
    let mut catalog = Catalog::default();
    intern(&mut catalog, &number(4));
    let limits = FormulaLimits {
        max_work: 0,
        ..FormulaLimits::default()
    };
    let mut counters = Counters::default();
    let result = catalog.find(&number(5), bound(), &mut counters, &limits, location());
    assert!(matches!(result, Err(FormulaFailure::Limit {
        resource: FormulaResource::Work, observed: 1, limit: 0, location: actual,
    }) if actual == location()));
    assert_eq!(counters.work, 0);
    assert_eq!(catalog.len(), 1);
    assert_eq!(find(&catalog, &number(4)), Some(0));
    assert_eq!(find(&catalog, &number(5)), None);
    assert_eq!(intern(&mut catalog, &number(5)), 1);
    assert_eq!(finish(catalog), [number(4), number(5)]);
}

#[test]
fn allocation_diagnostics_retain_the_original_error() {
    let error = Vec::<Atom>::new().try_reserve(usize::MAX).unwrap_err();
    let failure = FormulaFailure::AtomAllocation {
        error,
        location: location(),
    };
    assert!(failure.to_string().starts_with("formula atom storage:"));
    assert!(
        failure
            .source()
            .unwrap()
            .is::<std::collections::TryReserveError>()
    );
    let diagnostics = failure.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].primary().location, location());
}

#[test]
fn named_storage_refusal_preserves_exact_requested_bytes() {
    let failure = super::failure(
        zetesis_core::atom_interner::Failure::Bytes {
            required: 101,
            limit: 100,
        },
        bound(),
        location(),
    );
    assert!(matches!(failure, FormulaFailure::Limit {
        resource: FormulaResource::AtomStorageBytes, observed: 101, limit: 100, location: actual,
    } if actual == location()));
}

proptest! {
    #[test]
    fn indexed_lookup_agrees_with_typed_equality(values in prop::collection::vec(-32_i32..32, 0..256)) {
        let mut catalog = Catalog::default();
        let mut reference = BTreeMap::new();
        let mut ordered = Vec::new();
        for value in values {
            let atom = number(value);
            let next = reference.len();
            let id = *reference.entry(atom.clone()).or_insert_with(|| { ordered.push(atom.clone()); next });
            prop_assert_eq!(intern(&mut catalog, &atom), id);
            prop_assert_eq!(find(&catalog, &atom), Some(id));
        }
        prop_assert_eq!(finish(catalog), ordered);
    }
}
