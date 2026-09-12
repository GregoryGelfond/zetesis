//! Atom identity, first-occurrence order and reservation failures.

use std::collections::BTreeMap;
use std::error::Error;
use std::hash::{BuildHasher, BuildHasherDefault, Hasher};

use proptest::prelude::*;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::{Atom, AtomPattern, Predicate, Sign, Term, Value, ValueLimits, ValueNode};

use super::{Catalog, Entry};
use crate::{AtomAllocation, FormulaFailure};

#[derive(Default)]
struct ConstantHash;

impl Hasher for ConstantHash {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, _bytes: &[u8]) {}
}

type CollidingCatalog = Catalog<BuildHasherDefault<ConstantHash>>;

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

fn intern<S: BuildHasher>(catalog: &mut Catalog<S>, atom: &Atom) -> usize {
    let pattern = AtomPattern::new(
        atom.predicate().clone(),
        atom.values().iter().cloned().map(Term::Constant).collect(),
    )
    .unwrap();
    match catalog.entry(pattern.key(&[] as &[Value]).unwrap()) {
        Entry::Occupied(id) => id,
        Entry::Vacant(entry) => entry.insert().unwrap(),
    }
}

#[test]
fn hash_collisions_preserve_complete_atom_identity() {
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
    let mut catalog = CollidingCatalog::default();
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(intern(&mut catalog, atom), id);
    }
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(catalog.find(atom), Some(id));
        // Verify the forced-collision route actually stored every identity
        // under the constant hash, rather than silently using another hasher.
        assert_eq!(catalog.index.find(0, |&stored| stored == id), Some(&id));
    }
    assert_eq!(catalog.find(&number(2)), None);
    assert_eq!(catalog.into_atoms(), atoms);
}

#[test]
fn duplicate_atoms_reuse_the_first_id() {
    let mut catalog = CollidingCatalog::default();
    assert_eq!(intern(&mut catalog, &number(2)), 0);
    assert_eq!(intern(&mut catalog, &number(1)), 1);
    let pattern = AtomPattern::new(number(2).predicate().clone(), vec![Term::Variable(0)]).unwrap();
    assert!(matches!(
        catalog.entry(pattern.key(&[Value::Number(2)][..]).unwrap()),
        Entry::Occupied(0)
    ));
    assert_eq!(catalog.into_atoms(), [number(2), number(1)]);
}

#[test]
fn table_growth_preserves_first_occurrence_order() {
    let atoms: Vec<_> = (0..257).rev().map(number).collect();
    let mut catalog = Catalog::<std::collections::hash_map::RandomState>::default();
    for (id, atom) in atoms.iter().enumerate() {
        assert_eq!(intern(&mut catalog, atom), id);
    }
    for (id, atom) in atoms.iter().enumerate().rev() {
        assert_eq!(catalog.find(atom), Some(id));
    }
    assert_eq!(catalog.into_atoms(), atoms);
}

#[test]
fn refused_reservation_preserves_existing_membership() {
    let mut catalog = CollidingCatalog::default();
    assert_eq!(intern(&mut catalog, &number(4)), 0);
    assert!(matches!(
        catalog.reserve(usize::MAX),
        Err(AtomAllocation::Atoms(_))
    ));
    assert_eq!(catalog.find(&number(4)), Some(0));
    assert_eq!(catalog.find(&number(5)), None);
    assert_eq!(intern(&mut catalog, &number(5)), 1);
    assert_eq!(catalog.into_atoms(), [number(4), number(5)]);
}

fn located(error: AtomAllocation) -> FormulaFailure {
    FormulaFailure::AtomAllocation {
        error,
        location: Location {
            source: SourceId::new(7),
            span: Span::empty(ByteOffset::new(3)),
        },
    }
}

#[test]
fn atom_allocation_preserves_the_reservation_error() {
    let error = Vec::<Atom>::new().try_reserve(usize::MAX).unwrap_err();
    let failure = located(AtomAllocation::Atoms(error));
    assert!(failure.to_string().starts_with("formula atom storage:"));
    assert!(
        failure
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<std::collections::TryReserveError>()
    );
    let diagnostics = failure.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].primary().location.source, SourceId::new(7));
    assert_eq!(
        diagnostics[0].primary().location.span,
        Span::empty(ByteOffset::new(3))
    );
}

#[test]
fn index_allocation_preserves_the_reservation_error() {
    let error = hashbrown::HashTable::<usize>::new()
        .try_reserve(usize::MAX, |_| 0)
        .unwrap_err();
    let failure = located(AtomAllocation::Index(error));
    assert!(failure.to_string().starts_with("formula atom index:"));
    assert!(
        failure
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<hashbrown::TryReserveError>()
    );
    assert_eq!(
        failure.diagnostics()[0].primary().location.source,
        SourceId::new(7)
    );
}

proptest! {
    #[test]
    fn collision_lookup_agrees_with_typed_equality(values in prop::collection::vec(-32_i32..32, 0..256)) {
        let mut catalog = CollidingCatalog::default();
        let mut reference = BTreeMap::new();
        let mut ordered = Vec::new();
        for value in values {
            let atom = number(value);
            let next = reference.len();
            let id = *reference.entry(atom.clone()).or_insert_with(|| {
                ordered.push(atom.clone());
                next
            });
            prop_assert_eq!(intern(&mut catalog, &atom), id);
            prop_assert_eq!(catalog.find(&atom), Some(id));
        }
        prop_assert_eq!(catalog.into_atoms(), ordered);
    }
}
