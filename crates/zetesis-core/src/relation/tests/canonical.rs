//! Canonical source occurrences use the same dictionary and query operation.

use super::{atoms, predicate};
use crate::catalog::{PredicateRef, TermRef};
use crate::relation::{Failure, Limits, Relation};
use crate::{AtomCatalog, Value};

#[test]
fn canonical_rows_preserve_catalog_occurrences() {
    let signature = predicate(1);
    let catalog = AtomCatalog::new(atoms(
        &signature,
        vec![
            vec![Value::String("1".into())],
            vec![Value::Symbol("1".into())],
        ],
    ))
    .unwrap();
    let indices = [1, 0, 1];
    let relation = Relation::from_catalog_refs(
        PredicateRef::from(&signature),
        catalog.atoms(),
        &indices,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(relation.row_count(), indices.len());
    for (position, &source) in indices.iter().enumerate() {
        let row = relation.row(position).unwrap();
        assert_eq!(row.position(), position);
        assert_eq!(row.source_index(), source);
        assert_eq!(
            row.value(0),
            catalog.atoms().at(source).unwrap().values().at(0)
        );
    }
}

#[test]
fn foreign_catalog_terms_resolve_by_typed_value() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![
            vec![Value::String("1".into())],
            vec![Value::Symbol("1".into())],
            vec![Value::Symbol("1".into())],
        ],
    );
    let catalog = AtomCatalog::new(source.clone()).unwrap();
    let foreign = AtomCatalog::new(vec![source[1].clone()]).unwrap();
    let relation = Relation::from_refs(
        catalog.atoms().at(0).unwrap().predicate(),
        catalog.atoms(),
        Limits::default(),
    )
    .unwrap();
    let term = foreign.atoms().at(0).unwrap().values().at(0).unwrap();
    let query = relation.query(&[(0, term)], Limits::default()).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    assert_eq!(
        relation
            .select(&query, &input, Limits::default())
            .unwrap()
            .positions(),
        &[1, 2]
    );
}

#[test]
fn canonical_mapping_rejects_missing_occurrences() {
    let signature = predicate(0);
    let catalog = AtomCatalog::new(atoms(&signature, vec![vec![]])).unwrap();
    assert!(matches!(
        Relation::from_catalog_refs(
            PredicateRef::from(&signature),
            catalog.atoms(),
            &[1],
            Limits::default(),
        ),
        Err(Failure::CatalogIndex)
    ));
}

#[test]
fn canonical_empty_rows_retain_the_predicate() {
    let signature = predicate(2);
    let catalog = AtomCatalog::default();
    let relation = Relation::from_refs(
        PredicateRef::from(&signature),
        catalog.atoms(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(relation.predicate(), PredicateRef::from(&signature));
    assert_eq!(relation.row_count(), 0);
    assert_eq!(relation.columns().len(), 2);
}

#[test]
fn encoding_receipts_count_argument_occurrences() {
    let signature = predicate(1);
    let value = Value::Symbol("term".into());
    let source = atoms(&signature, vec![vec![value.clone()], vec![value.clone()]]);
    let catalog = AtomCatalog::new(source.clone()).unwrap();
    let ingress = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let canonical = Relation::from_refs(
        PredicateRef::from(&signature),
        catalog.atoms(),
        Limits::default(),
    )
    .unwrap();
    let expected = 2 * TermRef::from(&value).canonical_bytes() as u128;
    assert_eq!(ingress.storage().referenced_encoding_bytes, expected);
    assert_eq!(canonical.storage().referenced_encoding_bytes, expected);
}
