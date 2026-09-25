//! Source identity, emitted local order and located refusal contracts.

#[path = "count_capture.rs"]
mod count_capture;

use std::collections::BTreeMap;
use std::error::Error;

use proptest::prelude::*;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::catalog::AtomRef;
use zetesis_core::{Atom, AtomCatalog, Predicate, Sign, Value, ValueLimits, ValueNode};

use super::Catalog;
use crate::formula_support::{Publication, SourceSelection, testing::Fixture};
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

struct Selection {
    source: Fixture,
    catalog: Catalog,
}
impl Selection {
    fn new() -> Self {
        let mut source = Fixture::default();
        let catalog = source.with(location(), |_, computation, counters| {
            Catalog::new(computation, counters, &FormulaLimits::default(), location()).unwrap()
        });
        Self { source, catalog }
    }
    fn intern(&mut self, input: &Atom) -> usize {
        let Self { source, catalog } = self;
        source.with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let atom = computation
                .atom_ref(input.into(), &limits, counters, location())
                .unwrap();
            catalog
                .insert(&atom, bound(), computation, counters, &limits, location())
                .unwrap()
                .0
        })
    }
    fn find(&mut self, input: &Atom) -> Option<usize> {
        let Self { source, catalog } = self;
        source.with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let atom = computation
                .atom_ref(input.into(), &limits, counters, location())
                .unwrap();
            catalog
                .position(&atom, counters, &limits, location())
                .unwrap()
        })
    }
    fn finish(self, limits: &FormulaLimits) -> Result<AtomCatalog, FormulaFailure> {
        let (mut completed, mut counters) = self.source.finish(location());
        let mut publication = Publication::new(&mut completed, &counters, location())?;
        publication.atoms(
            self.catalog.into_selection(),
            limits,
            &mut counters,
            location(),
        )
    }
}

fn assert_atoms(actual: &AtomCatalog, expected: &[Atom]) {
    assert!(actual.atoms().iter().eq(expected.iter().map(AtomRef::from)));
}

#[test]
fn local_selection_preserves_complete_atom_identity() {
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
    let mut selected = Selection::new();
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(selected.intern(atom), id);
    }
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(selected.find(atom), Some(id));
    }
    assert_eq!(selected.find(&number(2)), None);
    assert_atoms(&selected.finish(&FormulaLimits::default()).unwrap(), &atoms);
}

#[test]
fn growth_preserves_first_emitted_occurrence_order() {
    let atoms: Vec<_> = (0..257).rev().map(number).collect();
    let mut selected = Selection::new();
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(selected.intern(atom), id);
    }
    for (id, atom) in atoms.iter().enumerate().rev() {
        assert_eq!(selected.find(atom), Some(id));
    }
    assert_atoms(&selected.finish(&FormulaLimits::default()).unwrap(), &atoms);
}

#[test]
fn exhausted_lookup_retains_its_source_location() {
    let mut selected = Selection::new();
    selected.intern(&number(4));
    selected
        .source
        .with(location(), |_, computation, counters| {
            let limits = FormulaLimits {
                max_work: counters.accounting.work,
                ..FormulaLimits::default()
            };
            let result = selected
                .catalog
                .get(0, computation, counters, &limits, location());
            assert!(matches!(result, Err(FormulaFailure::Limit {
            resource: FormulaResource::Work, observed, limit, location: actual,
        }) if observed == limit + 1 && actual == location()));
        });
    assert_eq!(selected.catalog.len(), 1);
    assert_eq!(selected.find(&number(4)), Some(0));
    assert_eq!(selected.intern(&number(5)), 1);
    assert_atoms(
        &selected.finish(&FormulaLimits::default()).unwrap(),
        &[number(4), number(5)],
    );
}

#[test]
fn signed_lookup_keeps_local_selection_membership() {
    let mut selected = Selection::new();
    selected.intern(&number(4));
    let opposite = atom("p", Sign::Negative, vec![Value::Number(4)]);
    // Canonical discovery alone is not an emitted member.
    assert_eq!(selected.find(&opposite), None);
    selected
        .source
        .with(location(), |_, computation, counters| {
            assert_eq!(
                selected
                    .catalog
                    .find(
                        0,
                        Sign::Negative,
                        computation,
                        counters,
                        &FormulaLimits::default(),
                        location()
                    )
                    .unwrap(),
                None
            );
        });
    assert_eq!(selected.intern(&opposite), 1);
    selected
        .source
        .with(location(), |_, computation, counters| {
            assert_eq!(
                selected
                    .catalog
                    .find(
                        0,
                        Sign::Negative,
                        computation,
                        counters,
                        &FormulaLimits::default(),
                        location()
                    )
                    .unwrap(),
                Some(1)
            );
        });
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
fn final_catalog_keeps_its_canonical_authority() {
    let mut selected = Selection::new();
    selected.intern(&number(4));
    let first = selected
        .source
        .with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let source = selected
                .catalog
                .source(0, counters, &limits, location())
                .unwrap();
            let mut first =
                SourceSelection::new(computation, &limits, counters, location()).unwrap();
            first
                .insert(&source, bound(), computation, &limits, counters, location())
                .unwrap();
            first
        });
    selected.intern(&number(5));
    let (mut completed, mut counters) = selected.source.finish(location());
    let limits = FormulaLimits::default();
    let mut publication = Publication::new(&mut completed, &counters, location()).unwrap();
    let prefix = publication
        .atoms(first, &limits, &mut counters, location())
        .unwrap();
    let finished = publication
        .atoms(
            selected.catalog.into_selection(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    assert!(prefix.shares_snapshot(&finished));
    assert_atoms(&prefix, &[number(4)]);
    assert_atoms(&finished, &[number(4), number(5)]);
}

#[test]
fn repeated_argument_payload_has_one_canonical_term() {
    let value = Value::String("shared argument".repeat(256));
    let mut selected = Selection::new();
    selected.intern(&atom("p", Sign::Positive, vec![value.clone()]));
    selected.intern(&atom("q", Sign::Positive, vec![value]));
    assert_eq!(
        selected
            .finish(&FormulaLimits::default())
            .unwrap()
            .storage()
            .terms,
        1
    );
}

#[test]
fn zero_atom_storage_refuses_final_publication() {
    let mut selected = Selection::new();
    selected.intern(&number(1));
    let limits = FormulaLimits {
        max_atom_storage_bytes: 0,
        ..FormulaLimits::default()
    };
    assert!(
        matches!(selected.finish(&limits), Err(FormulaFailure::Limit {
        resource: FormulaResource::AtomStorageBytes, observed, limit: 0, location: actual,
    }) if observed > 0 && actual == location())
    );
}

proptest! {
    #[test]
    fn local_lookup_agrees_with_typed_equality(values in prop::collection::vec(-32_i32..32, 0..256)) {
        let mut selected = Selection::new();
        let mut reference = BTreeMap::new();
        let mut ordered = Vec::new();
        for value in values {
            let atom = number(value);
            let next = reference.len();
            let id = *reference.entry(atom.clone()).or_insert_with(|| { ordered.push(atom.clone()); next });
            prop_assert_eq!(selected.intern(&atom), id);
            prop_assert_eq!(selected.find(&atom), Some(id));
        }
        assert_atoms(&selected.finish(&FormulaLimits::default()).unwrap(), &ordered);
    }
}
