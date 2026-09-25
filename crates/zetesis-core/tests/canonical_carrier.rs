//! Sparse coordinates share vocabulary without conflating independent atom rows.

use std::convert::Infallible;

use zetesis_core::catalog::interner::{AtomInterner, Failure, Limits};
use zetesis_core::catalog::{AtomRef, Error};
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, CarrierError, CarrierFailure, Predicate, Program, Template,
    Term, Value,
};

fn atom(value: &str) -> Atom {
    Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![Value::Symbol(value.into())],
    )
    .unwrap()
}

fn program() -> Program {
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
    Program::new(templates.into(), AdmissionLimits::default()).unwrap()
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
    let program = program();
    let mut left = AtomInterner::for_program(&program, 1024 * 1024).unwrap();
    let mut right = AtomInterner::for_program(&program, 1024 * 1024).unwrap();
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
fn frozen_program_writer_refuses_unknown_terms() {
    let program = program();
    let mut owner = AtomInterner::for_program(&program, 1024 * 1024).unwrap();
    let unknown = atom("not-in-the-domain");
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
fn sparse_lookup_honors_each_caller_refusal() {
    let program = program();
    let query = atom("second");
    let mut total = 0;
    let selected = program
        .locate_atom_with(&query, false, usize::MAX, || {
            total += 1;
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(selected.atom(), AtomRef::from(&query));
    for stop in 0..total {
        let mut calls = 0;
        let result = program.locate_atom_with(&query, false, usize::MAX, || {
            let index = calls;
            calls += 1;
            if index == stop { Err(stop) } else { Ok(()) }
        });
        assert_eq!(result, Err(CarrierFailure::Stopped(stop)));
        assert_eq!(calls, stop + 1);
    }
}

#[test]
fn key_lookup_denotes_the_same_carrier_atom() {
    let program = program();
    let pattern =
        AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap();
    let binding = [Value::Symbol("second".into())];
    let key = pattern.key(binding.as_slice()).unwrap();
    let selected = program
        .locate_key_with(&key, false, usize::MAX, || Ok::<_, Infallible>(()))
        .unwrap()
        .unwrap();
    assert_eq!(selected.atom(), AtomRef::from(&atom("second")));
}

#[test]
fn sparse_lookup_distinguishes_absence_from_capacity() {
    let program = program();
    assert!(
        program
            .locate_atom(&atom("missing"), false)
            .unwrap()
            .is_none()
    );
    let refused = program.locate_atom_with(&atom("first"), false, 0, || Ok::<_, Infallible>(()));
    assert!(matches!(
        refused,
        Err(CarrierFailure::Storage(CarrierError::Bytes {
            limit: 0,
            ..
        }))
    ));
}

#[test]
fn explicit_coordinates_refuse_outside_domain_ranks() {
    let program = program();
    let result = program.carrier_atom_with(0, &[program.domain().len()], usize::MAX, || {
        Ok::<_, Infallible>(())
    });
    assert_eq!(
        result,
        Err(CarrierFailure::Storage(CarrierError::Coordinates))
    );
}
