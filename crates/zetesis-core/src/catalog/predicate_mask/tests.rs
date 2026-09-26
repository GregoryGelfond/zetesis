use std::convert::Infallible;

use super::*;
use crate::catalog::{AtomCatalog, Limits};
use crate::{Atom, Predicate, Sign, Value};

fn atom(name: &str, arity: usize, sign: Sign) -> Atom {
    Atom::new(
        Predicate::with_sign(name, arity, sign).unwrap(),
        vec![Value::Number(0); arity],
    )
    .unwrap()
}

fn catalog(atoms: Vec<Atom>) -> AtomCatalog {
    AtomCatalog::new(atoms).unwrap()
}

#[test]
fn a_mask_decides_each_prefix_predicate_once() {
    // Equal signatures share one identity, so a repeated atom adds no decision.
    let catalog = catalog(vec![
        atom("p", 0, Sign::Positive),
        atom("p", 1, Sign::Negative),
        atom("q", 1, Sign::Positive),
        atom("p", 0, Sign::Positive),
    ]);
    let mut decided = 0;
    let mask = catalog
        .read()
        .predicate_mask_with(|predicate| {
            decided += 1;
            Ok::<_, Infallible>(predicate.name() == "p")
        })
        .unwrap();
    assert_eq!(decided, 3);
    assert_eq!(mask.len(), 3);
    for atom in catalog.atoms() {
        let predicate = atom.predicate();
        assert_eq!(mask.decision(predicate), Some(predicate.name() == "p"));
    }
}

#[test]
fn a_mask_decides_predicates_beyond_its_first_word() {
    // The 65th identity lives in the second word; its neighbours stay unset.
    let catalog = catalog(
        (0..65)
            .map(|index| atom(&format!("p{index:02}"), 0, Sign::Positive))
            .collect(),
    );
    let mask = catalog
        .read()
        .predicate_mask_with(|predicate| Ok::<_, Infallible>(predicate.name() == "p64"))
        .unwrap();
    assert_eq!(mask.len(), 65);
    for atom in catalog.atoms() {
        let predicate = atom.predicate();
        assert_eq!(mask.decision(predicate), Some(predicate.name() == "p64"));
    }
}

#[test]
fn a_mask_has_no_decision_for_a_later_predicate() {
    // A newer snapshot of the same vocabulary resolves the prepared identities
    // but not one admitted after preparation.
    let mut store = storage::Store::new(usize::MAX);
    let limits = Limits::default();
    store
        .import_atom(&atom("p", 1, Sign::Positive), limits)
        .unwrap();
    let old = store.snapshot(0).unwrap();
    store
        .import_atom(&atom("q", 1, Sign::Positive), limits)
        .unwrap();
    let new = store.snapshot(0).unwrap();
    let mask = CatalogRead(storage::Read::from(&old))
        .predicate_mask_with(|_| Ok::<_, Infallible>(true))
        .unwrap();
    let decisions: Vec<_> = storage::Read::from(&new)
        .predicate_ids()
        .map(|id| mask.decision(PredicateRef::new(&new, id).unwrap()))
        .collect();
    assert_eq!(decisions, [Some(true), None]);
}

#[test]
fn a_mask_has_no_decision_for_another_vocabulary() {
    // Equal signatures in an independent vocabulary have unrelated identities.
    let prepared = catalog(vec![atom("p", 1, Sign::Positive)]);
    let other = catalog(vec![atom("p", 1, Sign::Positive)]);
    let mask = prepared
        .read()
        .predicate_mask_with(|_| Ok::<_, Infallible>(true))
        .unwrap();
    let predicate = other.atoms().at(0).unwrap().predicate();
    assert_eq!(mask.decision(predicate), None);
}

#[test]
fn a_mask_has_no_decision_for_ingress() {
    let catalog = catalog(vec![atom("p", 1, Sign::Positive)]);
    let mask = catalog
        .read()
        .predicate_mask_with(|_| Ok::<_, Infallible>(true))
        .unwrap();
    let ingress = Predicate::new("p", 1).unwrap();
    assert_eq!(mask.decision(PredicateRef::from(&ingress)), None);
}

#[test]
fn a_refused_decision_returns_no_mask() {
    let catalog = catalog(vec![
        atom("p", 0, Sign::Positive),
        atom("q", 0, Sign::Positive),
        atom("r", 0, Sign::Positive),
    ]);
    for stop in 0..3 {
        let mut calls = 0;
        let result = catalog.read().predicate_mask_with(|_| {
            calls += 1;
            if calls > stop { Err(stop) } else { Ok(true) }
        });
        assert!(matches!(result, Err(PredicateMaskFailure::Stopped(value)) if value == stop));
        assert_eq!(calls, stop + 1);
    }
}

#[test]
fn a_mask_retains_one_word_per_64_predicates() {
    let catalog = catalog(
        (0..65)
            .map(|index| atom(&format!("p{index:02}"), 0, Sign::Positive))
            .collect(),
    );
    let mask = catalog
        .read()
        .predicate_mask_with(|_| Ok::<_, Infallible>(false))
        .unwrap();
    assert_eq!(
        mask.retained_bytes(),
        size_of::<PredicateMask>() + 2 * size_of::<u64>()
    );
}
