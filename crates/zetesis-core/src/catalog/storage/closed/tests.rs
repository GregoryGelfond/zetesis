use super::super::{AtomId, PredicateId, Read, TermId};
use super::*;
use crate::catalog::{AtomRef, Limits, PredicateRef, TermRef};
use crate::{Atom, Predicate, Value};
use std::convert::Infallible;

const PERMIT: fn() -> Result<(), Infallible> = || Ok(());

fn fixture() -> (Store, PredicateId, [TermId; 3]) {
    let mut store = Store::new(usize::MAX);
    let predicate = store
        .import_predicate_with(PredicateRef::from(&Predicate::new("p", 1).unwrap()), PERMIT)
        .unwrap();
    let terms = [3, 1, 2].map(|number| {
        store
            .import_value(&Value::Number(number), Limits::default())
            .unwrap()
    });
    store.intern_atom(predicate, &terms[..1]).unwrap();
    (store, predicate, terms)
}

#[test]
fn initial_snapshot_exposes_base_rows() {
    let (store, _, _) = fixture();
    let base = store.close_with(0, PERMIT).unwrap();
    let descendant = Store::with_closed(&base, usize::MAX).unwrap();
    let snapshot = descendant.empty_snapshot();
    assert_eq!(snapshot.atom_count(), 1);
    assert_eq!(
        AtomRef::new(&snapshot, AtomId(0))
            .unwrap()
            .values()
            .at(0)
            .unwrap(),
        TermRef::from(&Value::Number(3))
    );
}

#[test]
fn local_rows_keep_consecutive_index_coordinates() {
    let (store, predicate, terms) = fixture();
    let base = store.close_with(0, PERMIT).unwrap();
    let mut descendant = Store::with_closed(&base, usize::MAX).unwrap();
    let first = descendant.intern_atom(predicate, &terms[1..2]).unwrap();
    let old = descendant.snapshot(0).unwrap();
    let second = descendant.intern_atom(predicate, &terms[2..]).unwrap();
    assert_eq!((first, second), (AtomId(1), AtomId(2)));
    for (term, expected) in terms.into_iter().zip([AtomId(0), first, second]) {
        assert_eq!(
            descendant.intern_atom(predicate, &[term]).unwrap(),
            expected
        );
    }
    assert!(Read::from(&old).contains_atom(first));
    assert!(!Read::from(&old).contains_atom(second));
}

#[test]
fn sibling_tail_coordinates_have_distinct_authorities() {
    let (store, predicate, terms) = fixture();
    let base = store.close_with(0, PERMIT).unwrap();
    let mut left = Store::with_closed(&base, usize::MAX).unwrap();
    let mut right = Store::with_closed(&base, usize::MAX).unwrap();
    let a = left.intern_atom(predicate, &terms[1..2]).unwrap();
    let b = right.intern_atom(predicate, &terms[2..]).unwrap();
    assert_eq!(a, b);
    assert!(Read::from(&left).same_vocabulary(Read::from(&right)));
    assert!(!Read::from(&left).same_atoms(Read::from(&right)));
    assert_ne!(AtomRef::new(&left, a), AtomRef::new(&right, b));
}

#[test]
fn snapshots_do_not_retain_closed_indexes() {
    let (store, predicate, terms) = fixture();
    let base = store.close_with(0, PERMIT).unwrap();
    let indexes = Arc::downgrade(&base.rows);
    let payload = Arc::downgrade(&base.rows.payload);
    let vocabulary_indexes = Arc::downgrade(&base.vocabulary.indexes);
    let mut descendant = Store::with_closed(&base, usize::MAX).unwrap();
    let id = descendant.intern_atom(predicate, &terms[1..2]).unwrap();
    let snapshot = descendant.snapshot(0).unwrap();
    drop(descendant);
    drop(base);
    assert!(indexes.upgrade().is_none());
    assert!(vocabulary_indexes.upgrade().is_none());
    assert!(payload.upgrade().is_some());
    assert!(AtomRef::new(&snapshot, id).is_some());
}

#[test]
fn closure_moves_index_allocations() {
    let (store, _, _) = fixture();
    let vocabulary_index = store.vocabulary.indexes().terms.allocation_identity();
    let rows_index = store.atoms.allocation_identity();
    let base = store.close_with(0, PERMIT).unwrap();
    assert_eq!(base.rows.index.allocation_identity(), rows_index);
    assert_eq!(
        base.vocabulary.indexes.terms.allocation_identity(),
        vocabulary_index
    );
}

#[test]
fn closure_shares_sealed_row_allocations() {
    let (mut store, _, _) = fixture();
    let old = store.snapshot(0).unwrap();
    let base = store.close_with(0, PERMIT).unwrap();
    assert!(Arc::ptr_eq(
        &old.data.segments[0],
        &base.rows.payload.segments[0]
    ));
}

#[test]
fn descendant_cannot_be_closed_again() {
    let (store, _, _) = fixture();
    let base = store.close_with(0, PERMIT).unwrap();
    let descendant = Store::with_closed(&base, usize::MAX).unwrap();
    assert!(matches!(
        descendant.close_with(0, PERMIT).unwrap_err().failure,
        Failure::Storage(Fault::CatalogHasBase)
    ));
}

#[test]
fn closed_descendant_refuses_new_vocabulary() {
    let (store, _, _) = fixture();
    let base = store.close_with(0, PERMIT).unwrap();
    let mut descendant = Store::with_closed(&base, usize::MAX).unwrap();
    assert_eq!(
        descendant.import_value(&Value::Number(99), Limits::default()),
        Err(Fault::FrozenVocabulary)
    );
    let atom = Atom::new(Predicate::new("new", 0).unwrap(), vec![]).unwrap();
    assert_eq!(
        descendant.import_atom(&atom, Limits::default()),
        Err(Fault::FrozenVocabulary)
    );
}

#[test]
fn prior_snapshot_metadata_excludes_shared_payload() {
    let (mut store, predicate, terms) = fixture();
    let prior = store.snapshot(0).unwrap();
    store.intern_atom(predicate, &terms[1..2]).unwrap();
    store
        .import_value(&Value::String("later".into()), Limits::default())
        .unwrap();
    let expected = size_of::<SnapshotData>() as u128
        + budget::capacity(&prior.data.segments)
        + prior.data.vocabulary.metadata_bytes();
    let base = store.close_with(0, PERMIT).unwrap();
    assert_eq!(base.prior_metadata(&prior).unwrap(), expected);
    let descendant = Store::with_closed(&base, usize::MAX)
        .unwrap()
        .empty_snapshot();
    assert_eq!(
        base.prior_metadata(&descendant),
        Err(ReadError::ForeignCatalog)
    );
}

#[test]
fn prior_receipt_rejects_an_unavailable_extent() {
    let (mut store, _, _) = fixture();
    let mut prior = store.snapshot(0).unwrap();
    let base = store.close_with(0, PERMIT).unwrap();
    // Public constructors cannot forge this prefix; exercise the explicit
    // extent defense independently of the matching identity witnesses.
    Arc::get_mut(&mut prior.data).unwrap().atoms += 1;
    assert_eq!(base.prior_metadata(&prior), Err(ReadError::OutsidePrefix));
}

#[test]
fn frozen_vocabulary_directory_is_counted_once() {
    // The separate vocabulary-only input exercises an already frozen directory.
    let mut vocabulary = Store::new(usize::MAX);
    vocabulary
        .import_value(&Value::Number(4), Limits::default())
        .unwrap();
    let vocabulary = vocabulary.freeze_vocabulary_with(0, PERMIT).unwrap();
    let mut store = Store::with_vocabulary(vocabulary, usize::MAX).unwrap();
    let prior = store.snapshot(0).unwrap();
    let base = store.close_with(0, PERMIT).unwrap();
    assert_eq!(
        base.prior_metadata(&prior).unwrap(),
        size_of::<SnapshotData>() as u128 + budget::capacity(&prior.data.segments)
    );
}

#[test]
fn rejected_close_proposals_do_not_inflate_peak() {
    let (mut store, _, _) = fixture();
    let initial = store.current_bytes();
    store.ceiling(usize::try_from(initial).unwrap()).unwrap();
    let error = store.close_with(0, PERMIT).unwrap_err();
    assert!(
        matches!(error.failure, Failure::Storage(Fault::Storage { required, .. }) if required > initial)
    );
    assert_eq!(error.peak, initial);
}

#[test]
fn close_cutoffs_preserve_prior_snapshots() {
    let (mut complete, _, _) = fixture();
    complete.snapshot(0).unwrap();
    let mut count = 0;
    complete
        .close_with(0, || {
            count += 1;
            Ok::<_, usize>(())
        })
        .unwrap();
    for cut in 0..count {
        let (mut store, _, _) = fixture();
        let old = store.snapshot(0).unwrap();
        let mut calls = 0;
        let error = store
            .close_with(0, || {
                let current = calls;
                calls += 1;
                if current == cut { Err(cut) } else { Ok(()) }
            })
            .unwrap_err();
        assert!(matches!(error.failure, Failure::Stopped(value) if value == cut));
        assert_eq!(old.atom_count(), 1);
        assert!(AtomRef::new(&old, AtomId(0)).is_some());
    }
}

#[test]
fn returned_stop_records_successful_directory_growth() {
    let (complete, _, _) = fixture();
    let mut count = 0;
    complete
        .close_with(0, || {
            count += 1;
            Ok::<_, usize>(())
        })
        .unwrap();
    let mut observed_growth = false;
    for cut in 0..count {
        let (mut store, _, _) = fixture();
        assert_eq!(store.sealed.capacity(), 0);
        let Vocabulary::Growing(vocabulary) = &store.vocabulary else {
            panic!("the fixture has not frozen its vocabulary");
        };
        assert_eq!(vocabulary.sealed.capacity(), 0);
        let initial = store.current_bytes();
        let mut calls = 0;
        let mut before = || {
            let current = calls;
            calls += 1;
            if current == cut { Err(cut) } else { Ok(()) }
        };
        let mut peak = initial;
        assert!(matches!(
            store.prepare_close(0, &mut peak, &mut Work::new(&mut before)),
            Err(Failure::Stopped(value)) if value == cut
        ));
        // Preparation has only two directory allocations. Their actual
        // capacities remain inspectable here, before consuming publication.
        // Fixed Arc envelopes are still proposals at every stopped callback.
        let Vocabulary::Growing(vocabulary) = &store.vocabulary else {
            panic!("preparation cannot publish a frozen vocabulary");
        };
        let growth = store.sealed.capacity() * size_of::<Arc<RowSegment>>()
            + vocabulary.sealed.capacity() * size_of::<Arc<super::super::VocabularySegment>>();
        assert_eq!(peak, initial + growth as u128, "stopped permit {cut}");
        observed_growth |= growth > 0;
    }
    assert!(observed_growth);
}
