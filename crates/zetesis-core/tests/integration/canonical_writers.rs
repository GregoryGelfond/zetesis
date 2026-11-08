//! Independent tuple writers share vocabulary while keeping distinct atom scopes.

use std::convert::Infallible;
use std::hash::{DefaultHasher, Hash, Hasher};

use super::support::canonical::template_catalog;
use zetesis_core::catalog::interner::{AtomInterner, Failure, Limits};
use zetesis_core::catalog::{AtomRef, Error};
use zetesis_core::{
    Atom, AtomCatalog, AtomPattern, Model, Predicate, Template, TemplateCatalog, Term, Value,
};

fn atom(value: &str) -> Atom {
    Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![Value::Symbol(value.into())],
    )
    .unwrap()
}

fn vocabulary() -> TemplateCatalog {
    let predicate = Predicate::new("p", 1).unwrap();
    let templates = ["first", "second"].map(|value| {
        Template::new(
            Some(
                AtomPattern::new(
                    predicate.clone(),
                    vec![Term::Constant(Value::Symbol(value.into()))],
                )
                .unwrap(),
            ),
            vec![],
            vec![],
            vec![],
            vec![],
        )
    });
    template_catalog(&templates)
}

fn limits() -> Limits {
    Limits::for_atoms(16, 1024 * 1024)
}

fn insert(owner: &mut AtomInterner, value: &str) {
    let value = atom(value);
    owner
        .entry_atom_with(&value, limits(), || Ok::<_, Infallible>(()))
        .unwrap()
        .insert_with(limits(), || Ok::<_, Infallible>(()))
        .unwrap();
}

#[test]
fn independent_writers_do_not_alias_atom_positions() {
    let catalog = vocabulary();
    let mut left = AtomInterner::for_template_catalog(&catalog, 1024 * 1024).unwrap();
    let mut right = AtomInterner::for_template_catalog(&catalog, 1024 * 1024).unwrap();
    insert(&mut left, "first");
    insert(&mut right, "second");
    let left = left
        .into_catalog_with(limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    let right = right
        .into_catalog_with(limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    assert!(left.shares_terms(&right));
    assert_ne!(left.atoms().at(0), right.atoms().at(0));
    assert_eq!(left.atoms().at(0), Some(AtomRef::from(&atom("first"))));
    assert_eq!(right.atoms().at(0), Some(AtomRef::from(&atom("second"))));
}

#[test]
fn frozen_catalog_writer_refuses_unknown_terms() {
    let catalog = vocabulary();
    let mut owner = AtomInterner::for_template_catalog(&catalog, 1024 * 1024).unwrap();
    let unknown = atom("not-in-the-vocabulary");
    let result = owner
        .entry_atom_with(&unknown, limits(), || Ok::<_, Infallible>(()))
        .and_then(|entry| entry.insert_with(limits(), || Ok::<_, Infallible>(())));
    assert!(matches!(
        result,
        Err(Failure::Catalog(Error::FrozenVocabulary))
    ));
    assert!(owner.is_empty());
}

#[test]
fn canonical_atom_views_agree_with_their_logical_atoms() {
    // Arities zero, one and two exercise complete argument traversal and identity.
    fn hash(value: &impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }
    let atoms = [
        Atom::new(Predicate::new("q", 0).unwrap(), vec![]).unwrap(),
        atom("first"),
        atom("second"),
        Atom::new(
            Predicate::new("r", 2).unwrap(),
            ["first", "second"]
                .map(|value| Value::Symbol(value.into()))
                .into(),
        )
        .unwrap(),
    ];
    let catalog = AtomCatalog::new(atoms.to_vec()).unwrap();
    for (view, atom) in catalog.atoms().iter().zip(&atoms) {
        assert_eq!(view.values().len(), atom.values().len());
        assert!(view.values().iter().eq(atom.values().iter().cloned()));
        assert_eq!(hash(&view), hash(&AtomRef::from(atom)));
        for (other, other_atom) in catalog.atoms().iter().zip(&atoms) {
            let expected = AtomRef::from(atom).cmp(&AtomRef::from(other_atom));
            assert_eq!(view.cmp(&other), expected);
            assert_eq!(
                view.compare_ref_with(other, || Ok::<_, Infallible>(())),
                Ok(expected)
            );
        }
    }
}

#[test]
fn canonical_lookup_honors_each_caller_refusal() {
    let model = Model::new([atom("first"), atom("second")]).unwrap();
    let query = atom("second");
    let mut total = 0;
    let selected = model
        .lookup()
        .get_with(&query, || {
            total += 1;
            Ok::<_, usize>(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(selected.atom(), AtomRef::from(&query));
    assert!(total > 0);
    for stop in 0..total {
        let mut calls = 0;
        let result = model.lookup().get_with(&query, || {
            let index = calls;
            calls += 1;
            if index == stop { Err(stop) } else { Ok(()) }
        });
        assert_eq!(result.unwrap_err(), stop);
        assert_eq!(calls, stop + 1);
    }
}

#[test]
fn key_lookup_denotes_the_same_canonical_atom() {
    let model = Model::new([atom("first"), atom("second")]).unwrap();
    let pattern =
        AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap();
    let binding = [Value::Symbol("second".into())];
    let key = pattern.key(binding.as_slice()).unwrap();
    let selected = model
        .lookup()
        .get_key_with(&key, || Ok::<_, Infallible>(()))
        .unwrap()
        .unwrap();
    assert_eq!(selected.atom(), AtomRef::from(&atom("second")));
    assert!(
        model
            .lookup()
            .get_with(&atom("missing"), || Ok::<_, Infallible>(()))
            .unwrap()
            .is_none()
    );
}
