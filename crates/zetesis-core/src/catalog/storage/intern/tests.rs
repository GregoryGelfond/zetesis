//! Prepared row publication consumes an exact result without probing again.

use super::*;
use crate::test_support::PERMIT;
use crate::{Predicate, Value};

#[test]
fn shared_and_local_collision_chains_remain_exact() {
    let (mut store, predicate, terms) = fixture();
    let mut before = PERMIT;
    let mut work = Work::new(&mut before);
    let prepared = store
        .prepare_atom_hash(predicate, 1, |_| terms[0], 7, &mut work)
        .unwrap();
    let first = store
        .publish_atom(prepared, |_| terms[0], &mut work)
        .unwrap();
    let base = store.close_with(0, PERMIT).unwrap();
    let mut descendant = Store::with_closed(&base, usize::MAX).unwrap();
    let mut expected = vec![first];
    for &term in &terms[1..] {
        let prepared = descendant
            .prepare_atom_hash(predicate, 1, |_| term, 7, &mut work)
            .unwrap();
        assert!(prepared.present().is_none());
        expected.push(
            descendant
                .publish_atom(prepared, |_| term, &mut work)
                .unwrap(),
        );
    }
    for (term, id) in terms.into_iter().zip(expected) {
        assert_eq!(
            descendant
                .prepare_atom_hash(predicate, 1, |_| term, 7, &mut work)
                .unwrap()
                .present(),
            Some(id)
        );
    }
}

#[test]
fn shared_lookup_hashes_arguments_once() {
    let (mut store, predicate, terms) = fixture();
    store.intern_atom(predicate, &terms[..1]).unwrap();
    let base = store.close_with(0, PERMIT).unwrap();
    let mut descendant = Store::with_closed(&base, usize::MAX).unwrap();
    descendant.intern_atom(predicate, &terms[1..2]).unwrap();
    for (term, expected_reads) in terms.into_iter().zip([2, 2, 1]) {
        let mut reads = 0;
        descendant
            .prepare_atom_by_with(
                predicate,
                1,
                |_| {
                    reads += 1;
                    term
                },
                PERMIT,
            )
            .unwrap();
        // Each occupied tuple adds one exact equality read to the single hash
        // read; an absent hash has no collision candidate in either index.
        assert_eq!(reads, expected_reads);
    }
}

fn fixture() -> (Store, PredicateId, [TermId; 3]) {
    let mut store = Store::new(usize::MAX);
    let predicate = store
        .import_predicate_with((&Predicate::new("p", 1).unwrap()).into(), PERMIT)
        .unwrap();
    let terms = [3, 1, 2].map(|number| {
        store
            .import_value(&Value::Number(number), Limits::default())
            .unwrap()
    });
    (store, predicate, terms)
}

#[test]
fn prepared_publication_survives_hash_collisions() {
    let (mut store, predicate, terms) = fixture();
    let mut ids = Vec::new();
    let mut before = PERMIT;
    let mut work = Work::new(&mut before);
    // Deliberately one hash for distinct authoritative tuples. This exercises
    // the same collision-exact preparation and publication used after hashing.
    for term in terms {
        let prepared = store
            .prepare_atom_hash(predicate, 1, |_| term, 7, &mut work)
            .unwrap();
        assert!(prepared.present().is_none());
        ids.push(store.publish_atom(prepared, |_| term, &mut work).unwrap());
    }
    for (term, id) in terms.into_iter().zip(ids) {
        let prepared = store
            .prepare_atom_hash(predicate, 1, |_| term, 7, &mut work)
            .unwrap();
        assert_eq!(prepared.present(), Some(id));
        assert_eq!(
            store
                .publish_atom(
                    prepared,
                    |_| panic!("present row reads no tuple"),
                    &mut work
                )
                .unwrap(),
            id,
        );
    }
    assert_eq!(store.counts().atoms, 3);
}

#[test]
fn vacant_publication_reads_only_its_written_columns() {
    let (mut store, predicate, terms) = fixture();
    let prepared = store
        .prepare_atom_by_with(predicate, 1, |_| terms[0], PERMIT)
        .unwrap();
    assert!(prepared.present().is_none());
    let mut reads = 0;
    store
        .publish_prepared_atom_with(
            prepared,
            |_| {
                reads += 1;
                terms[0]
            },
            PERMIT,
        )
        .unwrap();
    // A repeated hash/probe would read this source before column publication.
    assert_eq!(reads, 1);
}

#[test]
fn prepared_absence_survives_hash_head_growth() {
    let (mut store, _, terms) = fixture();
    let mut expected = Vec::new();
    let mut grew_nonempty_heads = false;
    for number in 0..128 {
        let name = format!("p{number}");
        let predicate = store
            .import_predicate_with((&Predicate::new(name, 1).unwrap()).into(), PERMIT)
            .unwrap();
        let term = terms[number % terms.len()];
        let prepared = store
            .prepare_atom_by_with(predicate, 1, |_| term, PERMIT)
            .unwrap();
        assert!(prepared.present().is_none());
        let old_capacity = store.atoms.head_capacity();
        // Only reservations and this row's publication intervene after exact
        // absence. Hash-head growth must not invalidate its preparation;
        // collision-link vector growth alone cannot satisfy this control.
        let id = store
            .publish_prepared_atom_with(prepared, |_| term, PERMIT)
            .unwrap();
        grew_nonempty_heads |= old_capacity > 0 && store.atoms.head_capacity() > old_capacity;
        expected.push((predicate, term, id));
    }
    assert!(grew_nonempty_heads);
    for (predicate, term, id) in expected {
        assert_eq!(
            store
                .prepare_atom_by_with(predicate, 1, |_| term, PERMIT)
                .unwrap()
                .present(),
            Some(id)
        );
    }
}
