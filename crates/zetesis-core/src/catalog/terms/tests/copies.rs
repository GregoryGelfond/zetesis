use super::*;

#[test]
fn borrowed_term_copy_retains_canonical_identity() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::String("shared".repeat(200)));
    let read = CatalogRead(storage::Read::from(&store));
    let value = TermRef::new(read.0, id).unwrap();
    let mut target = read.assignment();
    target.resize_with(1, usize::MAX, SUCCESS).unwrap();
    let mut calls = 0;
    target
        .set_term_with(0, read, value, || {
            calls += 1;
            SUCCESS()
        })
        .unwrap();
    assert_eq!(calls, 2);
    let copied = target
        .as_slice()
        .term_with(read, 0, SUCCESS)
        .unwrap()
        .unwrap();
    let (ValueNodeRef::String(source), ValueNodeRef::String(copied)) =
        (value.descriptor(), copied.descriptor())
    else {
        panic!("string fixture");
    };
    assert_eq!(source.as_ptr(), copied.as_ptr());
}

#[test]
fn borrowed_source_authentication_precedes_preparation() {
    let mut store = storage::Store::new(usize::MAX);
    admit(&mut store, &Value::Number(7));
    let prefix = store.snapshot(0).unwrap();
    let newer = admit(&mut store, &Value::Number(9));
    let mut foreign = storage::Store::new(usize::MAX);
    let foreign_id = admit(&mut foreign, &Value::Number(7));
    let read = CatalogRead(storage::Read::from(&prefix));
    let complete = CatalogRead(storage::Read::from(&store));
    let foreign_read = CatalogRead(storage::Read::from(&foreign));
    let ingress = Value::Number(7);
    let cases = [
        ((&ingress).into(), ReadError::Uninterned),
        (
            TermRef::new(foreign_read.0, foreign_id).unwrap(),
            ReadError::ForeignCatalog,
        ),
        (
            TermRef::new(complete.0, newer).unwrap(),
            ReadError::OutsidePrefix,
        ),
    ];
    let mut target = read.assignment();
    for (value, expected) in cases {
        let mut prepared = false;
        let result = target.set_term_with(usize::MAX, read, value, || {
            prepared = true;
            Err("reservation refused")
        });
        assert!(matches!(result,
            Err(AssignmentFailure::Assignment(AssignmentError::Read(actual)))
                if actual == expected));
        assert!(!prepared);
        assert!(target.is_empty());
    }
}

#[test]
fn destination_authority_is_checked_after_first_callback() {
    let mut source = storage::Store::new(usize::MAX);
    let id = admit(&mut source, &Value::Number(7));
    let other = storage::Store::new(usize::MAX);
    let read = CatalogRead(storage::Read::from(&source));
    let value = TermRef::new(read.0, id).unwrap();
    let mut target = CatalogRead(storage::Read::from(&other)).assignment();
    assert!(matches!(
        target.set_term_with(usize::MAX, read, value, || Err("stop")),
        Err(AssignmentFailure::Stopped("stop"))
    ));
    let mut calls = 0;
    let result = target.set_term_with(usize::MAX, read, value, || {
        calls += 1;
        SUCCESS()
    });
    assert!(matches!(
        result,
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog
        )))
    ));
    assert_eq!(calls, 1);
}

#[test]
fn borrowed_copy_refusal_preserves_previous_slot() {
    let mut store = storage::Store::new(usize::MAX);
    let old = admit(&mut store, &Value::Number(3));
    let new = admit(&mut store, &Value::Number(8));
    let read = CatalogRead(storage::Read::from(&store));
    let value = TermRef::new(read.0, new).unwrap();
    for stop in 0..2 {
        let mut target = read.assignment();
        target.resize_with(1, usize::MAX, SUCCESS).unwrap();
        target.set_with(0, &key(&store, old), SUCCESS).unwrap();
        let mut calls = 0;
        let result = target.set_term_with(0, read, value, || {
            let current = calls;
            calls += 1;
            if current == stop { Err(stop) } else { Ok(()) }
        });
        assert!(matches!(result, Err(AssignmentFailure::Stopped(actual)) if actual == stop));
        assert_eq!(calls, stop + 1);
        assert_eq!(
            target
                .as_slice()
                .term_with(read, 0, SUCCESS)
                .unwrap()
                .unwrap(),
            Value::Number(3)
        );
        target.set_term_with(0, read, value, SUCCESS).unwrap();
        assert_eq!(
            target
                .as_slice()
                .term_with(read, 0, SUCCESS)
                .unwrap()
                .unwrap(),
            Value::Number(8)
        );
    }
}

#[test]
fn slot_copy_does_not_require_an_older_reader_prefix() {
    let mut store = storage::Store::new(usize::MAX);
    admit(&mut store, &Value::Number(1));
    let prefix = store.snapshot(0).unwrap();
    let mut target = CatalogRead(storage::Read::from(&prefix)).assignment();
    target.resize_with(1, usize::MAX, SUCCESS).unwrap();
    let newer = admit(&mut store, &Value::Number(2));
    let read = CatalogRead(storage::Read::from(&store));
    let mut source = read.assignment();
    source.resize_with(2, usize::MAX, SUCCESS).unwrap();
    source.set_with(1, &key(&store, newer), SUCCESS).unwrap();
    let mut calls = 0;
    target
        .copy_slot_with(0, source.as_slice(), 1, || {
            calls += 1;
            SUCCESS()
        })
        .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(
        target
            .as_slice()
            .term_with(read, 0, SUCCESS)
            .unwrap()
            .unwrap(),
        Value::Number(2)
    );
    assert!(matches!(
        target
            .as_slice()
            .term_with(CatalogRead(storage::Read::from(&prefix)), 0, SUCCESS),
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            ReadError::OutsidePrefix
        )))
    ));
}

#[test]
fn slot_copy_checks_source_presence_before_destination() {
    let source_store = storage::Store::new(usize::MAX);
    let target_store = storage::Store::new(usize::MAX);
    let mut source = CatalogRead(storage::Read::from(&source_store)).assignment();
    source.resize_with(1, usize::MAX, SUCCESS).unwrap();
    let mut target = CatalogRead(storage::Read::from(&target_store)).assignment();
    for slot in [0, 1, usize::MAX] {
        let mut calls = 0;
        let result = target.copy_slot_with(usize::MAX, source.as_slice(), slot, || {
            calls += 1;
            Err("stop")
        });
        if slot == 0 {
            assert!(matches!(
                result,
                Err(AssignmentFailure::Assignment(AssignmentError::Unbound {
                    slot: 0
                }))
            ));
        } else {
            assert!(
                matches!(result, Err(AssignmentFailure::Assignment(AssignmentError::Slot { slot: actual, len: 1 })) if actual == slot)
            );
        }
        assert_eq!(calls, 0);
    }
}

#[test]
fn slot_copy_refusal_preserves_previous_slot() {
    let mut store = storage::Store::new(usize::MAX);
    let old = admit(&mut store, &Value::Number(3));
    let new = admit(&mut store, &Value::Number(8));
    let read = CatalogRead(storage::Read::from(&store));
    let mut source = read.assignment();
    source.resize_with(1, usize::MAX, SUCCESS).unwrap();
    source.set_with(0, &key(&store, new), SUCCESS).unwrap();
    for stop in 0..2 {
        let mut target = read.assignment();
        target.resize_with(1, usize::MAX, SUCCESS).unwrap();
        target.set_with(0, &key(&store, old), SUCCESS).unwrap();
        let mut calls = 0;
        let result = target.copy_slot_with(0, source.as_slice(), 0, || {
            let current = calls;
            calls += 1;
            if current == stop { Err(stop) } else { Ok(()) }
        });
        assert!(matches!(result, Err(AssignmentFailure::Stopped(actual)) if actual == stop));
        assert_eq!(
            target
                .as_slice()
                .term_with(read, 0, SUCCESS)
                .unwrap()
                .unwrap(),
            Value::Number(3)
        );
    }
}

#[test]
fn term_reborrowing_checks_selected_prefix() {
    let mut store = storage::Store::new(usize::MAX);
    let first = admit(&mut store, &Value::Number(1));
    let prefix = store.snapshot(0).unwrap();
    let newer = admit(&mut store, &Value::Number(2));
    let complete = CatalogRead(storage::Read::from(&store));
    let read = TermRead::from(CatalogRead(storage::Read::from(&prefix)));
    assert_eq!(
        read.borrow_term(TermRef::new(complete.0, first).unwrap())
            .unwrap(),
        Value::Number(1)
    );
    assert!(matches!(
        read.borrow_term(TermRef::new(complete.0, newer).unwrap()),
        Err(ReadError::OutsidePrefix)
    ));
}

#[test]
fn borrowed_destination_slot_follows_first_callback() {
    let mut store = storage::Store::new(usize::MAX);
    let id = admit(&mut store, &Value::Number(7));
    let read = CatalogRead(storage::Read::from(&store));
    let value = TermRef::new(read.0, id).unwrap();
    let mut target = read.assignment();
    let mut calls = 0;
    let result = target.set_term_with(1, read, value, || {
        calls += 1;
        SUCCESS()
    });
    assert!(matches!(
        result,
        Err(AssignmentFailure::Assignment(AssignmentError::Slot {
            slot: 1,
            len: 0
        }))
    ));
    assert_eq!(calls, 1);
    assert!(target.is_empty());
}
