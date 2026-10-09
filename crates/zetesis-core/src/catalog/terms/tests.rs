use std::convert::Infallible;

use super::*;
use crate::test_support::PERMIT as SUCCESS;
use crate::{Value, ValueNodeRef};

fn admit(store: &mut storage::Store, value: &Value) -> storage::TermId {
    store
        .import_term_with(value.into(), super::super::Limits::default(), || {
            Ok::<(), Infallible>(())
        })
        .unwrap()
}
fn key(store: &storage::Store, id: storage::TermId) -> TermKey {
    let read = CatalogRead(storage::Read::from(store));
    read.term_key(TermRef::new(read.0, id).unwrap()).unwrap()
}

#[test]
fn equal_foreign_terms_cannot_be_assigned_by_identity() {
    let mut first = storage::Store::new(usize::MAX);
    let mut second = storage::Store::new(usize::MAX);
    admit(&mut first, &Value::Number(7));
    let id = admit(&mut second, &Value::Number(7));
    let mut frame = CatalogRead(storage::Read::from(&first)).assignment();
    frame.resize_with(1, usize::MAX, SUCCESS).unwrap();
    assert!(matches!(
        frame.set_with(0, &key(&second, id), SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog
        )))
    ));
    assert!(!frame.as_slice().is_bound(0).unwrap());
}

#[test]
fn reader_prefix_checks_only_the_selected_live_slots() {
    let mut store = storage::Store::new(usize::MAX);
    let first = admit(&mut store, &Value::Number(1));
    let prefix = store.snapshot(0).unwrap();
    let newer = admit(&mut store, &Value::Number(2));
    let mut frame = CatalogRead(storage::Read::from(&store)).assignment();
    frame.resize_with(2, usize::MAX, SUCCESS).unwrap();
    frame.set_with(0, &key(&store, first), SUCCESS).unwrap();
    frame.set_with(1, &key(&store, newer), SUCCESS).unwrap();
    let read = CatalogRead(storage::Read::from(&prefix));
    assert!(matches!(
        frame.as_slice().bind_with(read, SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            ReadError::OutsidePrefix
        )))
    ));
    assert_eq!(
        frame
            .prefix(1)
            .unwrap()
            .bind_with(read, SUCCESS)
            .unwrap()
            .get(0)
            .unwrap(),
        Value::Number(1)
    );
    frame.clear_with(1, SUCCESS).unwrap();
    assert!(
        frame
            .as_slice()
            .bind_with(read, SUCCESS)
            .unwrap()
            .get(1)
            .is_none()
    );
}

#[test]
fn frame_copy_retains_ids_and_one_vocabulary_without_payload_copy() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::String("shared".repeat(200)));
    let read = CatalogRead(storage::Read::from(&store));
    let mut frame = read.assignment();
    frame.resize_with(3, usize::MAX, SUCCESS).unwrap();
    frame.set_with(2, &key(&store, id), SUCCESS).unwrap();
    let copied = frame.as_slice().copy_with(usize::MAX, SUCCESS).unwrap();
    let left = frame
        .as_slice()
        .bind_with(read, SUCCESS)
        .unwrap()
        .get(2)
        .unwrap();
    let right = copied
        .as_slice()
        .bind_with(read, SUCCESS)
        .unwrap()
        .get(2)
        .unwrap();
    let (ValueNodeRef::String(left), ValueNodeRef::String(right)) =
        (left.descriptor(), right.descriptor())
    else {
        panic!("string fixture");
    };
    assert_eq!(left.as_ptr(), right.as_ptr());
    assert!(!copied.as_slice().is_bound(0).unwrap());
}

#[test]
fn empty_frame_copy_does_not_adopt_a_foreign_vocabulary() {
    let first = storage::Store::new(usize::MAX);
    let second = storage::Store::new(usize::MAX);
    let frame = CatalogRead(storage::Read::from(&first)).assignment();
    let copy = frame.as_slice().copy_with(usize::MAX, SUCCESS).unwrap();
    assert!(matches!(
        copy.as_slice()
            .bind_with(CatalogRead(storage::Read::from(&second)), SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog
        )))
    ));
}

#[test]
fn invalid_slot_is_distinct_from_an_unbound_slot() {
    let store = storage::Store::new(usize::MAX);
    let mut frame = CatalogRead(storage::Read::from(&store)).assignment();
    frame.resize_with(1, usize::MAX, SUCCESS).unwrap();
    assert!(frame.key(0).unwrap().is_none());
    assert!(matches!(
        frame.key(1),
        Err(AssignmentError::Slot { slot: 1, len: 1 })
    ));
}

#[test]
fn selected_roots_count_once_without_selecting_children_or_neighbors() {
    let mut store = storage::Store::new(usize::MAX);
    let mut ids = Vec::new();
    for value in 0..130 {
        ids.push(admit(&mut store, &Value::Number(value)));
    }
    let mut set = CatalogRead(storage::Read::from(&store)).term_set();
    for index in [0, 64, 129] {
        assert!(
            set.insert_with(&key(&store, ids[index]), usize::MAX, SUCCESS)
                .unwrap()
        );
        let bytes = set.retained_bytes();
        assert!(
            !set.insert_with(&key(&store, ids[index]), bytes, SUCCESS)
                .unwrap()
        );
        assert_eq!(set.retained_bytes(), bytes);
    }
    assert_eq!(set.len(), 3);
    for (index, id) in ids.into_iter().enumerate() {
        assert_eq!(
            set.contains_with(&key(&store, id), SUCCESS).unwrap(),
            [0, 64, 129].contains(&index)
        );
    }
}

#[test]
fn frame_resize_refusals_do_not_publish_partial_initialization() {
    let store = storage::Store::new(usize::MAX);
    let read = CatalogRead(storage::Read::from(&store));
    let mut measured = read.assignment();
    let mut calls = 0;
    measured
        .resize_with(4, usize::MAX, || {
            calls += 1;
            SUCCESS()
        })
        .unwrap();
    for cut in 0..calls {
        let mut frame = read.assignment();
        let mut visited = 0;
        assert!(matches!(frame.resize_with(4, usize::MAX, || {
            if visited == cut { return Err(cut); }
            visited += 1; Ok(())
        }), Err(AssignmentFailure::Stopped(actual)) if actual == cut));
        assert!(frame.is_empty());
        assert_eq!(visited, cut);
    }
}

#[test]
fn frame_growth_admits_replacement_overlap() {
    let store = storage::Store::new(usize::MAX);
    let read = CatalogRead(storage::Read::from(&store));
    let mut frame = read.assignment();
    frame.resize_with(2, usize::MAX, SUCCESS).unwrap();
    let exact = frame.retained_bytes() + 4 * size_of::<Option<storage::TermId>>();
    assert!(matches!(frame.resize_with(4, exact - 1, SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage { required, limit })))
        if required == exact as u128 && limit == exact - 1));
    assert_eq!(frame.len(), 2);
    frame.resize_with(4, exact, SUCCESS).unwrap();
    assert_eq!(frame.len(), 4);
}

#[test]
fn selected_root_refusals_withhold_membership_and_count() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::Number(1));
    let value = key(&store, id);
    let read = CatalogRead(storage::Read::from(&store));
    let mut measured = read.term_set();
    let mut calls = 0;
    measured
        .insert_with(&value, usize::MAX, || {
            calls += 1;
            SUCCESS()
        })
        .unwrap();
    for cut in 0..calls {
        let mut set = read.term_set();
        let mut visited = 0;
        assert!(matches!(set.insert_with(&value, usize::MAX, || {
            if visited == cut { return Err(cut); }
            visited += 1; Ok(())
        }), Err(AssignmentFailure::Stopped(actual)) if actual == cut));
        assert!(set.is_empty());
        assert!(!set.contains_with(&value, SUCCESS).unwrap());
    }
}

#[test]
fn refused_copy_keeps_old_slots_and_exposes_reserved_capacity() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::Number(8));
    let read = CatalogRead(storage::Read::from(&store));
    let mut source = read.assignment();
    source.resize_with(4, usize::MAX, SUCCESS).unwrap();
    source.set_with(3, &key(&store, id), SUCCESS).unwrap();
    let mut target = read.assignment();
    target.resize_with(1, usize::MAX, SUCCESS).unwrap();
    target.set_with(0, &key(&store, id), SUCCESS).unwrap();
    let original = target.retained_bytes();
    let mut calls = 0;
    let mut reference = read.assignment();
    reference
        .copy_from_with(source.as_slice(), usize::MAX, || {
            calls += 1;
            SUCCESS()
        })
        .unwrap();
    let mut visited = 0;
    // This cut reaches the slot-copy phase after reservation, without publishing it.
    let result = target.copy_from_with(source.as_slice(), usize::MAX, || {
        visited += 1;
        if visited == calls {
            Err("copy")
        } else {
            Ok(())
        }
    });
    assert!(matches!(result, Err(AssignmentFailure::Stopped("copy"))));
    assert_eq!(target.len(), 1);
    assert!(target.as_slice().is_bound(0).unwrap());
    assert!(target.retained_bytes() > original);
}

#[test]
fn slot_swap_refusals_preserve_bound_and_absent_slots() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::Number(8));
    let read = CatalogRead(storage::Read::from(&store));
    let mut frame = read.assignment();
    frame.resize_with(2, usize::MAX, SUCCESS).unwrap();
    frame.set_with(0, &key(&store, id), SUCCESS).unwrap();
    for cut in 0..2 {
        let mut visited = 0;
        assert!(matches!(frame.swap_with(0, 1, || {
            if visited == cut { return Err(cut); }
            visited += 1; Ok(())
        }), Err(AssignmentFailure::Stopped(actual)) if actual == cut));
        assert!(frame.as_slice().is_bound(0).unwrap());
        assert!(!frame.as_slice().is_bound(1).unwrap());
    }
    assert!(matches!(
        frame.swap_with(0, 2, SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Slot {
            slot: 2,
            len: 2
        }))
    ));
    frame.swap_with(0, 1, SUCCESS).unwrap();
    let bound = frame.as_slice().bind_with(read, SUCCESS).unwrap();
    assert!(bound.get(0).is_none());
    assert_eq!(bound.get(1).unwrap(), Value::Number(8));
}

#[test]
fn identity_index_order_is_distinct_from_asp_order() {
    let mut store = storage::Store::new(usize::MAX);
    let first = admit(&mut store, &Value::Number(9));
    let second = admit(&mut store, &Value::Number(-1));
    let read = CatalogRead(storage::Read::from(&store));
    let mut frame = read.assignment();
    frame.resize_with(1, usize::MAX, SUCCESS).unwrap();
    frame.set_with(0, &key(&store, first), SUCCESS).unwrap();
    assert!(
        frame
            .as_slice()
            .compare_key(0, &key(&store, second))
            .unwrap()
            .is_lt()
    );
    let first = read.term(&key(&store, first)).unwrap();
    let second = read.term(&key(&store, second)).unwrap();
    assert!(first.compare_terms_with(second, SUCCESS).unwrap().is_gt());
}

#[test]
fn identity_comparison_checks_scope_before_slot_absence() {
    let first = storage::Store::new(usize::MAX);
    let mut second = storage::Store::new(usize::MAX);
    let foreign = admit(&mut second, &Value::Number(1));
    let frame = CatalogRead(storage::Read::from(&first)).assignment();
    assert!(matches!(
        frame.as_slice().compare_key(0, &key(&second, foreign)),
        Err(AssignmentError::Read(ReadError::ForeignCatalog))
    ));
}

#[test]
fn identity_comparison_distinguishes_invalid_and_absent_slots() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::Number(1));
    let mut frame = CatalogRead(storage::Read::from(&store)).assignment();
    frame.resize_with(1, usize::MAX, SUCCESS).unwrap();
    let key = key(&store, id);
    assert!(matches!(
        frame.as_slice().compare_key(0, &key),
        Err(AssignmentError::Unbound { slot: 0 })
    ));
    assert!(matches!(
        frame.as_slice().compare_key(1, &key),
        Err(AssignmentError::Slot { slot: 1, len: 1 })
    ));
}

#[test]
fn single_slot_read_does_not_require_unselected_newer_terms() {
    let mut store = storage::Store::new(usize::MAX);
    let first = admit(&mut store, &Value::Number(1));
    let prefix = store.snapshot(0).unwrap();
    let newer = admit(&mut store, &Value::Number(2));
    let mut frame = CatalogRead(storage::Read::from(&store)).assignment();
    frame.resize_with(2, usize::MAX, SUCCESS).unwrap();
    frame.set_with(0, &key(&store, first), SUCCESS).unwrap();
    frame.set_with(1, &key(&store, newer), SUCCESS).unwrap();
    let read = CatalogRead(storage::Read::from(&prefix));
    assert_eq!(
        frame
            .as_slice()
            .term_with(read, 0, SUCCESS)
            .unwrap()
            .unwrap(),
        Value::Number(1)
    );
    assert!(matches!(
        frame.as_slice().term_with(read, 1, SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            ReadError::OutsidePrefix
        )))
    ));
}

#[test]
fn single_slot_read_distinguishes_absence_from_invalid_position() {
    let store = storage::Store::new(usize::MAX);
    let read = CatalogRead(storage::Read::from(&store));
    let mut frame = read.assignment();
    frame.resize_with(1, usize::MAX, SUCCESS).unwrap();
    assert!(
        frame
            .as_slice()
            .term_with(read, 0, SUCCESS)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        frame.as_slice().term_with(read, 1, SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Slot {
            slot: 1,
            len: 1
        }))
    ));
}

#[test]
fn single_slot_read_checks_scope_before_absence() {
    let owner = storage::Store::new(usize::MAX);
    let foreign = storage::Store::new(usize::MAX);
    let mut frame = CatalogRead(storage::Read::from(&owner)).assignment();
    frame.resize_with(1, usize::MAX, SUCCESS).unwrap();
    assert!(matches!(
        frame
            .as_slice()
            .term_with(CatalogRead(storage::Read::from(&foreign)), 0, SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog
        )))
    ));
}

#[test]
fn single_slot_read_preserves_every_caller_refusal() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::Number(1));
    let read = CatalogRead(storage::Read::from(&store));
    let mut frame = read.assignment();
    frame.resize_with(1, usize::MAX, SUCCESS).unwrap();
    frame.set_with(0, &key(&store, id), SUCCESS).unwrap();
    let mut complete = 0;
    frame
        .as_slice()
        .term_with(read, 0, || {
            complete += 1;
            Ok::<_, usize>(())
        })
        .unwrap();
    for cutoff in 1..=complete {
        let mut visited = 0;
        let result = frame.as_slice().term_with(read, 0, || {
            visited += 1;
            if visited == cutoff {
                Err(cutoff)
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(AssignmentFailure::Stopped(value)) if value == cutoff));
        assert_eq!(visited, cutoff);
    }
}

#[test]
fn slot_comparison_matches_key_comparison() {
    let mut store = storage::Store::new(usize::MAX);
    let ids = [
        admit(&mut store, &Value::Number(9)),
        admit(&mut store, &Value::Number(-1)),
    ];
    let read = CatalogRead(storage::Read::from(&store));
    let mut left = read.assignment();
    let mut right = read.assignment();
    left.resize_with(2, usize::MAX, SUCCESS).unwrap();
    right.resize_with(2, usize::MAX, SUCCESS).unwrap();
    for (slot, id) in ids.iter().copied().enumerate() {
        left.set_with(slot, &key(&store, id), SUCCESS).unwrap();
        right.set_with(slot, &key(&store, id), SUCCESS).unwrap();
    }
    for a in 0..2 {
        for b in 0..2 {
            assert_eq!(
                left.as_slice().compare_slot(a, right.as_slice(), b),
                left.as_slice()
                    .compare_key(a, &right.key(b).unwrap().unwrap())
            );
        }
    }
    assert!(
        left.as_slice()
            .compare_slot(0, right.as_slice(), 1)
            .unwrap()
            .is_lt()
    );
}

#[test]
fn slot_comparison_preserves_authentication_order() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::Number(1));
    let mut left = CatalogRead(storage::Read::from(&store)).assignment();
    left.resize_with(2, usize::MAX, SUCCESS).unwrap();
    left.set_with(0, &key(&store, id), SUCCESS).unwrap();
    let right = CatalogRead(storage::Read::from(&store)).assignment();
    let mut foreign_store = storage::Store::new(usize::MAX);
    let foreign_id = admit(&mut foreign_store, &Value::Number(1));
    let mut foreign = CatalogRead(storage::Read::from(&foreign_store)).assignment();
    foreign.resize_with(1, usize::MAX, SUCCESS).unwrap();
    foreign
        .set_with(0, &key(&foreign_store, foreign_id), SUCCESS)
        .unwrap();
    assert!(matches!(
        left.as_slice().compare_slot(0, foreign.as_slice(), 0),
        Err(AssignmentError::Read(ReadError::ForeignCatalog))
    ));
    assert!(matches!(
        left.as_slice().compare_slot(3, foreign.as_slice(), 3),
        Err(AssignmentError::Read(ReadError::ForeignCatalog))
    ));
    assert_eq!(
        left.as_slice().compare_slot(3, right.as_slice(), 3),
        Err(AssignmentError::Slot { slot: 3, len: 2 })
    );
    assert_eq!(
        left.as_slice().compare_slot(1, right.as_slice(), 3),
        Err(AssignmentError::Unbound { slot: 1 })
    );
    assert_eq!(
        left.as_slice().compare_slot(0, right.as_slice(), 3),
        Err(AssignmentError::Slot { slot: 3, len: 0 })
    );
    assert_eq!(
        left.as_slice().compare_slot(0, left.as_slice(), 1),
        Err(AssignmentError::Unbound { slot: 1 })
    );
}

#[test]
fn slot_equality_does_not_certify_a_reader_prefix() {
    let mut store = storage::Store::new(usize::MAX);
    let old = store.snapshot(0).unwrap();
    let id = admit(&mut store, &Value::Number(1));
    let mut frame = CatalogRead(storage::Read::from(&store)).assignment();
    frame.resize_with(1, usize::MAX, SUCCESS).unwrap();
    frame.set_with(0, &key(&store, id), SUCCESS).unwrap();
    assert_eq!(
        frame.as_slice().compare_slot(0, frame.as_slice(), 0),
        Ok(std::cmp::Ordering::Equal)
    );
    assert!(matches!(
        frame
            .as_slice()
            .bind_with(CatalogRead(storage::Read::from(&old)), SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            ReadError::OutsidePrefix
        )))
    ));
}

mod copies;
