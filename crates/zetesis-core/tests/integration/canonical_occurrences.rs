//! Relation positions resolve only within their original canonical source.
use zetesis_core::relation::{Limits, Relation};
use zetesis_core::{Atom, AtomCatalog, Predicate, Value};

fn source() -> AtomCatalog {
    AtomCatalog::new(
        [1, 2, 1]
            .into_iter()
            .map(|n| Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(n)]).unwrap())
            .collect(),
    )
    .unwrap()
}

#[test]
fn mapped_rows_recover_original_occurrences() {
    let source = source();
    let atoms = source.atoms();
    let predicate = atoms.at(0).unwrap().predicate();
    let relation =
        Relation::from_catalog_refs(predicate, atoms, &[2, 0, 2], Limits::default()).unwrap();
    for (local, expected) in [2, 0, 2].into_iter().enumerate() {
        assert_eq!(
            relation.row(local).unwrap().occurrence_in(atoms),
            Some(expected)
        );
    }
}

#[test]
fn equal_catalogs_do_not_share_occurrence_identity() {
    let first = source();
    let second = source();
    assert_eq!(first.atoms(), second.atoms());
    let relation = Relation::from_refs(
        first.atoms().at(0).unwrap().predicate(),
        first.atoms(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(relation.row(0).unwrap().occurrence_in(second.atoms()), None);
}

#[test]
fn cloned_catalogs_share_the_original_occurrences() {
    let first = source();
    let second = first.clone();
    let relation = Relation::from_refs(
        first.atoms().at(0).unwrap().predicate(),
        first.atoms(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        relation.row(1).unwrap().occurrence_in(second.atoms()),
        Some(1)
    );
}
