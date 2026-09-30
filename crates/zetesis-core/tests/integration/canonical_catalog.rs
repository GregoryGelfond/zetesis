//! Canonical publication preserves typed identity and retained interpretations.

use std::hash::{Hash, Hasher};

use zetesis_core::catalog::{AtomCatalog, AtomRef, Catalog, Error, Limits};
use zetesis_core::{Model, Value};
use zetesis_test_support::programs::atom;

fn fingerprint(value: &impl Hash) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut state);
    state.finish()
}

#[test]
fn canonical_import_preserves_original_occurrences() {
    let first = atom("p", vec![Value::Symbol("x".into()), Value::Number(3)]);
    let second = atom("p", vec![Value::String("x".into()), Value::Number(3)]);
    let source = vec![first.clone(), second, first];
    let catalog = AtomCatalog::new(source.clone()).unwrap();
    assert_eq!(catalog.atoms().len(), source.len());
    for (actual, expected) in catalog.atoms().into_iter().zip(&source) {
        assert_eq!(actual, AtomRef::from(expected));
    }
}

#[test]
fn semantic_hash_is_independent_of_catalog_owner() {
    let selected = atom("p", vec![Value::Symbol("x".into())]);
    let before = atom("a", vec![Value::Number(-7)]);
    let left = AtomCatalog::new(vec![selected.clone(), before.clone()]).unwrap();
    let right = AtomCatalog::new(vec![before, selected.clone()]).unwrap();
    let left = left.atoms().at(0).unwrap();
    let right = right.atoms().at(1).unwrap();
    assert_eq!(left, right);
    assert_eq!(fingerprint(&left), fingerprint(&right));
    assert_eq!(fingerprint(&left), fingerprint(&selected));
}

#[test]
fn appending_preserves_retained_interpretations() {
    let first = atom("p", vec![Value::Number(1)]);
    let second = atom("q", vec![Value::String("later".into())]);
    let mut owner = Catalog::new(usize::MAX);
    let old = owner
        .edit(Limits::default(), |builder| {
            let key = builder.intern_atom(&first)?;
            builder.publish(&[key, key])
        })
        .unwrap();
    let model = Model::from_positions(&old, [1, 0]).unwrap();
    let old_payload = old.retained_payload_bytes();
    let old_storage = old.storage();
    let later = owner
        .edit(Limits::default(), |builder| {
            let key = builder.intern_atom(&second)?;
            builder.publish(&[key])
        })
        .unwrap();
    assert!(old.shares_terms(&later));
    drop(owner);
    drop(later);
    assert_eq!(old.retained_payload_bytes(), old_payload);
    assert_eq!(old.storage(), old_storage);
    assert_eq!(old.atoms().len(), 2);
    assert_eq!(model.atoms().len(), 1);
    assert_eq!(model.atoms().first().unwrap(), AtomRef::from(&first));
    assert!(!model.contains(&second));
}

#[test]
fn repeated_argument_payloads_share_canonical_terms() {
    let payload = Value::String("one shared payload".repeat(100));
    let catalog = AtomCatalog::new(vec![
        atom("p", vec![payload.clone(), payload.clone()]),
        atom("q", vec![payload.clone()]),
        atom("p", vec![payload.clone(), payload]),
    ])
    .unwrap();
    assert_eq!(catalog.storage().terms, 1);
    assert_eq!(catalog.storage().atoms, 2);
    assert_eq!(catalog.atoms().len(), 3);
}

#[test]
fn catalog_limit_refusal_does_not_publish_a_model() {
    let source = atom("p", vec![Value::String("payload".into())]);
    let mut owner = Catalog::new(0);
    let result = owner.edit(Limits::default(), |builder| {
        let key = builder.intern_atom(&source)?;
        builder.publish(&[key])
    });
    assert!(matches!(result, Err(Error::Storage { limit: 0, .. })));
}

#[test]
fn publication_admits_the_occurrence_mapping() {
    let source = atom("p", vec![]);
    let mut owner = Catalog::new(65_536);
    let result = owner.edit(Limits::default(), |builder| {
        let key = builder.intern_atom(&source)?;
        // Repeated positions are legal, but their mapping still occupies space.
        builder.publish(&vec![key; 100_000])
    });
    assert!(matches!(result, Err(Error::Storage { limit: 65_536, .. })));
    let admitted = owner
        .edit(Limits::default(), |builder| {
            let key = builder.intern_atom(&source)?;
            builder.publish(&[key])
        })
        .unwrap();
    assert_eq!(admitted.atoms().at(0).unwrap(), AtomRef::from(&source));
}

#[test]
fn intern_hits_obey_current_logical_limits() {
    let source = atom("p", vec![Value::Symbol("x".into())]);
    let mut owner = Catalog::new(usize::MAX);
    let original = owner
        .edit(Limits::default(), |builder| {
            let key = builder.intern_atom(&source)?;
            builder.publish(&[key])
        })
        .unwrap();
    let limits = Limits {
        max_nodes: 0,
        ..Limits::default()
    };
    let refused = owner.edit(limits, |builder| {
        let key = builder.intern_atom(&source)?;
        builder.publish(&[key])
    });
    assert!(matches!(refused, Err(Error::Value(_))));
    assert_eq!(original.atoms().at(0).unwrap(), AtomRef::from(&source));
}
