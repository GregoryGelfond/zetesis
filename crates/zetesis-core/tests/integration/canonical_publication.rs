//! Checked publication preserves the distinction between identity and selection.

use std::convert::Infallible;

use crate::support::canonical::before;
use zetesis_core::catalog::interner::{AtomInterner, Failure, Limits};
use zetesis_core::catalog::{AtomCatalog, AtomRef};
use zetesis_core::{Atom, Model, ModelError, ModelFailure, Predicate, Value};
use zetesis_test_support::programs::unary;

fn limits() -> Limits {
    Limits::for_atoms(32, 16 * 1024 * 1024)
}

fn owner() -> AtomInterner {
    let mut owner = AtomInterner::new();
    for number in [2, 1, 3] {
        let atom = unary("p", number);
        owner
            .entry_atom_with(&atom, limits(), || Ok::<_, Infallible>(()))
            .unwrap()
            .insert_with(limits(), || Ok::<_, Infallible>(()))
            .unwrap();
    }
    owner
}

#[test]
fn selection_publication_preserves_requested_occurrences() {
    let mut owner = owner();
    let selected = owner
        .publish_selection_with(&[2, 0, 2], limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    drop(owner);
    let expected = [unary("p", 3), unary("p", 2), unary("p", 3)];
    assert_eq!(
        selected.atoms().iter().collect::<Vec<_>>(),
        expected.iter().map(AtomRef::from).collect::<Vec<_>>()
    );
}

#[test]
fn interrupted_selection_preserves_discovered_identity() {
    let mut count = 0;
    owner()
        .publish_selection_with(&[2, 0], limits(), || {
            count += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
    for limit in 0..count {
        let mut owner = owner();
        let mut remaining = limit;
        assert!(matches!(
            owner.publish_selection_with(&[2, 0], limits(), || before(&mut remaining)),
            Err(Failure::Stopped(()))
        ));
        assert_eq!(owner.len(), 3);
        for (position, number) in [2, 1, 3].into_iter().enumerate() {
            assert_eq!(
                owner.get(position),
                Some(AtomRef::from(&unary("p", number)))
            );
        }
        let retry = owner
            .publish_selection_with(&[2, 0], limits(), || Ok::<_, ()>(()))
            .unwrap();
        assert_eq!(retry.atoms().len(), 2);
    }
}

#[test]
fn sorted_publication_uses_typed_identity_order() {
    let catalog = owner()
        .into_ordered_catalog_with(limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    let expected = [unary("p", 1), unary("p", 2), unary("p", 3)];
    assert_eq!(
        catalog.atoms().iter().collect::<Vec<_>>(),
        expected.iter().map(AtomRef::from).collect::<Vec<_>>()
    );
}

#[test]
fn ordered_model_refuses_nonincreasing_occurrences() {
    for numbers in [[1, 1], [2, 1]] {
        let catalog =
            AtomCatalog::new(numbers.into_iter().map(|value| unary("p", value)).collect()).unwrap();
        let result =
            Model::from_ordered_catalog_with(catalog, usize::MAX, || Ok::<_, Infallible>(()));
        assert!(matches!(
            result,
            Err(ModelFailure::Model(ModelError::Order { position: 1 }))
        ));
    }
}

#[test]
fn ordered_model_retains_the_published_authority() {
    let catalog = AtomCatalog::new(vec![unary("p", 1), unary("p", 2)]).unwrap();
    let model =
        Model::from_ordered_catalog_with(catalog.clone(), usize::MAX, || Ok::<_, Infallible>(()))
            .unwrap();
    assert!(catalog.same_owner(model.catalog()));
}

#[test]
fn ordered_model_honors_each_caller_refusal() {
    let catalog = AtomCatalog::new(vec![unary("p", 1), unary("p", 2)]).unwrap();
    let mut count = 0;
    Model::from_ordered_catalog_with(catalog.clone(), usize::MAX, || {
        count += 1;
        Ok::<_, ()>(())
    })
    .unwrap();
    for limit in 0..count {
        let mut remaining = limit;
        assert!(matches!(
            Model::from_ordered_catalog_with(catalog.clone(), usize::MAX, || before(
                &mut remaining
            )),
            Err(ModelFailure::Stopped(()))
        ));
    }
}

#[test]
fn ordered_model_bounds_selection_storage() {
    let catalog = AtomCatalog::new(vec![unary("p", 1)]).unwrap();
    let result = Model::from_ordered_catalog_with(catalog, 0, || Ok::<_, Infallible>(()));
    assert!(matches!(
        result,
        Err(ModelFailure::Model(ModelError::Bytes { limit: 0, .. }))
    ));
}

#[test]
fn checked_storage_matches_the_complete_measure() {
    let mut owner = owner();
    let first = owner
        .publish_selection_with(&[0], limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    let extra = Atom::new(
        Predicate::new("q", 2).unwrap(),
        vec![Value::String("retained".into()), Value::Number(9)],
    )
    .unwrap();
    owner
        .entry_atom_with(&extra, limits(), || Ok::<_, Infallible>(()))
        .unwrap()
        .insert_with(limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    let second = owner
        .publish_selection_with(&[3, 0], limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    for catalog in [first, second] {
        let mut count = 0;
        let actual = catalog
            .storage_with(|| {
                count += 1;
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(actual, catalog.storage());
        assert!(count > 0);
        for limit in 0..count {
            let mut remaining = limit;
            assert_eq!(catalog.storage_with(|| before(&mut remaining)), Err(()));
        }
    }
}
