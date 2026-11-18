//! Pure membership, occupied-entry scratch and independent probe-work controls.

use super::*;

fn pattern() -> AtomPattern {
    AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap()
}

// Inspect the query and its predicate representation before selecting the
// content-resolution ingress path. These fixtures contain no canonical query IDs.
const CANONICAL_APPLICABILITY_WORK: usize = 2;

#[test]
fn queries_borrow_committed_and_pending_identities() {
    let mut owner = owner(&[2, 1, 3]);
    owner
        .commit_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    insert(&mut owner, &atom(4));
    let shared = &owner;
    let committed = shared.committed();
    let pending = shared.get(3).unwrap();
    let pattern = pattern();
    for (id, value) in [2, 1, 3, 4].into_iter().enumerate() {
        let values = [Value::Number(value)];
        let key = pattern.key(values.as_slice()).unwrap();
        assert_eq!(
            shared
                .find_key_with(key, limits(), || Ok::<(), Infallible>(()))
                .unwrap(),
            Some(id)
        );
        assert_eq!(
            shared
                .find_atom_with(&atom(value), limits(), || Ok::<(), Infallible>(()))
                .unwrap(),
            Some(id)
        );
    }
    assert!(
        committed
            .get(0)
            .unwrap()
            .same_identity(shared.get(0).unwrap())
    );
    assert!(pending.same_identity(shared.get(3).unwrap()));
    assert_eq!(committed.len(), 3);
    assert_eq!(shared.len(), 4);
}

#[test]
fn occupied_entries_leave_scratch_unchanged() {
    let mut owner = owner(&[2, 1, 3]);
    let bytes = owner.storage_bytes();
    let peak = owner.storage_peak_bytes();
    for (id, value) in [2, 1, 3].into_iter().enumerate() {
        let atom = atom(value);
        let entry = owner
            .entry_atom_with(&atom, limits(), || Ok::<(), Infallible>(()))
            .unwrap();
        assert_eq!(entry.position(), Some(id));
        assert_eq!(
            entry
                .insert_with(limits(), || Err("occupied entry copies nothing"))
                .unwrap(),
            id
        );
        assert_eq!(owner.storage_bytes(), bytes);
        assert_eq!(owner.storage_peak_bytes(), peak);
    }
}

#[test]
fn occupied_entries_need_no_scratch_capacity() {
    let mut owner = owner(&[2, 1, 3]);
    let live = owner.storage_bytes();
    let bounds = Limits {
        max_bytes: live + PREPARED_BYTES,
        ..limits()
    };
    let duplicate = atom(1);
    assert_eq!(
        owner
            .entry_atom_with(&duplicate, bounds, || Ok::<(), Infallible>(()))
            .unwrap()
            .position(),
        Some(1)
    );
    let pattern = pattern();
    let values = [Value::Number(3)];
    let key = pattern.key(values.as_slice()).unwrap();
    let (_, mut append) = owner.split();
    assert_eq!(
        append
            .entry_key_with(key, bounds, || Ok::<(), Infallible>(()))
            .unwrap()
            .position(),
        Some(2)
    );
    assert_eq!(owner.storage_bytes(), live);
    validate(&owner);
}

#[test]
fn read_only_misses_need_no_mutation_storage() {
    let owner = owner(&[2, 1, 3]);
    let live = owner.storage_bytes();
    let peak = owner.storage_peak_bytes();
    let bounds = Limits {
        max_bytes: live,
        ..limits()
    };
    let pattern = pattern();
    let values = [Value::Number(4)];
    let key = pattern.key(values.as_slice()).unwrap();
    assert_eq!(
        owner
            .find_atom_with(&atom(4), bounds, || Ok::<(), Infallible>(()))
            .unwrap(),
        None
    );
    assert_eq!(
        owner
            .find_key_with(key, bounds, || Ok::<(), Infallible>(()))
            .unwrap(),
        None
    );
    assert_eq!(owner.storage_bytes(), live);
    assert_eq!(owner.storage_peak_bytes(), peak);
    validate(&owner);
}

#[test]
fn foreign_lookup_work_is_independent_of_population() {
    // Ingress queries resolve the predicate, each argument and the row by
    // content through exact hash indexes: no typed search over the relation.
    let small = owner(&[2, 1, 3]);
    let large = owner(&(0..256).rev().collect::<Vec<_>>());
    let pattern = pattern();
    let work = |owner: &AtomInterner, value: i32| {
        let mut atom_work = 0;
        let found = owner
            .find_atom_with(&atom(value), limits(), || {
                atom_work += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap();
        let values = [Value::Number(value)];
        let mut key_work = 0;
        let keyed = owner
            .find_key_with(pattern.key(values.as_slice()).unwrap(), limits(), || {
                key_work += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap();
        assert_eq!(found, keyed);
        (found.is_some(), atom_work, key_work)
    };
    for value in [1, 2, 3] {
        let (present, atom_work, key_work) = work(&small, value);
        assert!(present);
        assert_eq!(work(&large, value), (true, atom_work, key_work));
    }
    for value in [0, 4] {
        assert!(!work(&small, value).0);
    }
}

#[test]
fn query_work_limits_are_inclusive() {
    let owner = owner(&[2, 1, 3]);
    let pattern = pattern();
    for value in [3, 4] {
        let values = [Value::Number(value)];
        let key = pattern.key(values.as_slice()).unwrap();
        let mut total = 0;
        owner
            .find_key_with(key, limits(), || {
                total += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap();
        for limit in 0..=total {
            let cause = ("query stopped", limit);
            let mut spent = 0;
            let mut before = || {
                if spent == limit {
                    Err(&cause)
                } else {
                    spent += 1;
                    Ok(())
                }
            };
            let result = owner.find_key_with(key, limits(), &mut before);
            if limit < total {
                assert!(
                    matches!(result, Err(Failure::Stopped(actual)) if std::ptr::eq(actual, &raw const cause))
                );
                assert_eq!(spent, limit);
            } else {
                assert_eq!(result.unwrap(), (value == 3).then_some(2));
                assert_eq!(spent, total);
            }
        }
    }
    validate(&owner);
}

#[test]
fn query_population_admission_precedes_probing() {
    let owner = owner(&[2, 1, 3]);
    let result = owner.find_atom_with(
        &atom(2),
        Limits {
            max_atoms: 2,
            ..limits()
        },
        || Err("no work"),
    );
    assert!(matches!(
        result,
        Err(Failure::Atoms {
            required: 3,
            limit: 2
        })
    ));
}

#[test]
fn query_storage_admission_precedes_probing() {
    let owner = owner(&[2, 1, 3]);
    let live = owner.storage_bytes();
    let result = owner.find_atom_with(
        &atom(2),
        Limits {
            max_bytes: live - 1,
            ..limits()
        },
        || Err("no work"),
    );
    assert!(
        matches!(result, Err(Failure::Bytes { required, limit }) if required == live && limit == live - 1)
    );
}

#[test]
fn empty_queries_charge_signature_resolution() {
    let owner = AtomInterner::new();
    let atom = Atom::new(Predicate::new("empty", 0).unwrap(), vec![]).unwrap();
    let pattern = AtomPattern::new(atom.predicate().clone(), vec![]).unwrap();
    let mut atom_work = 0;
    assert_eq!(
        owner
            .find_atom_with(&atom, limits(), || {
                atom_work += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap(),
        None
    );
    let mut key_work = 0;
    assert_eq!(
        owner
            .find_key_with(pattern.key(&[] as &[Value]).unwrap(), limits(), || {
                key_work += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap(),
        None
    );
    // An absent signature ends the content resolution before any argument.
    assert!(atom_work > CANONICAL_APPLICABILITY_WORK);
    assert_eq!(key_work, atom_work);
}

#[test]
fn monotone_and_inner_insertions_keep_typed_order() {
    let mut owner = AtomInterner::new();
    let values = [5, 1, 9, 7, 11, 11, 0, 6, 12, 5];
    let mut discovered = Vec::new();
    for value in values {
        let expected = discovered
            .iter()
            .position(|known| *known == value)
            .unwrap_or_else(|| {
                let position = discovered.len();
                discovered.push(value);
                position
            });
        assert_eq!(insert(&mut owner, &atom(value)), expected);
        validate(&owner);
    }
    // Ordered publication exposes the committed prefix; insertion above has
    // intentionally exercised discovery while every identity was pending.
    owner
        .commit_with(limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    let order = owner
        .ordered_ids_with(limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    let mut expected = discovered.clone();
    expected.sort_unstable();
    assert_eq!(
        order.iter().map(|&id| discovered[id]).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn empty_lookup_permits_canonical_signature_resolution() {
    let atom = Atom::new(Predicate::new("empty", 0).unwrap(), vec![]).unwrap();
    let catalog = AtomCatalog::new(vec![atom]).unwrap();
    let query = catalog.atoms().at(0).unwrap();
    let owner = AtomInterner::new();
    let cause = "signature stopped";
    let result = owner.find_atom_with(query, limits(), || Err(&cause));
    assert!(
        matches!(result, Err(Failure::Stopped(actual)) if std::ptr::eq(actual, &raw const cause))
    );
    assert!(owner.is_empty());
}

#[test]
fn empty_entry_permits_canonical_signature_resolution() {
    let atom = Atom::new(Predicate::new("empty", 0).unwrap(), vec![]).unwrap();
    let catalog = AtomCatalog::new(vec![atom]).unwrap();
    let query = catalog.atoms().at(0).unwrap();
    let mut owner = AtomInterner::new();
    let cause = "signature stopped";
    let result = owner.entry_atom_with(query, limits(), || Err(&cause));
    assert!(
        matches!(result, Err(Failure::Stopped(actual)) if std::ptr::eq(actual, &raw const cause))
    );
    assert!(owner.is_empty());
}

#[test]
fn appender_lookup_reads_both_prefixes_without_capacity_mutation() {
    let mut owner = owner(&[2, 1]);
    owner
        .commit_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    let (_, mut append) = owner.split();
    append
        .entry_atom_with(&atom(3), limits(), || Ok::<(), Infallible>(()))
        .unwrap()
        .insert_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    let before = (append.storage_bytes(), append.storage_peak_bytes());
    let pattern = pattern();
    for (value, expected) in [(2, Some(0)), (1, Some(1)), (3, Some(2)), (4, None)] {
        let values = [Value::Number(value)];
        let key = pattern.key(values.as_slice()).unwrap();
        assert_eq!(
            append
                .find_key_with(key, limits(), || Ok::<(), Infallible>(()))
                .unwrap(),
            expected
        );
    }
    assert_eq!(
        (append.storage_bytes(), append.storage_peak_bytes()),
        before
    );
}

#[test]
fn appender_lookup_preserves_each_callback_refusal() {
    let mut owner = owner(&[2, 1, 3]);
    let (_, append) = owner.split();
    let pattern = pattern();
    let values = [Value::Number(3)];
    let key = pattern.key(values.as_slice()).unwrap();
    let mut total = 0;
    append
        .find_key_with(key, limits(), || {
            total += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
    let before = (append.storage_bytes(), append.storage_peak_bytes());
    for cut in 0..total {
        let mut calls = 0;
        assert!(matches!(append.find_key_with(key, limits(), || {
            if calls == cut { return Err(cut); }
            calls += 1; Ok(())
        }), Err(Failure::Stopped(actual)) if actual == cut));
        assert_eq!(calls, cut);
        assert_eq!(
            (append.storage_bytes(), append.storage_peak_bytes()),
            before
        );
    }
}

#[test]
fn predicate_import_byte_refusal_reports_the_complete_owner() {
    let predicate = Predicate::new("long_name".repeat(512), 0).unwrap();
    let mut measured = owner(&[1]);
    measured
        .declare_predicate_with(&predicate, limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    let exact = measured.storage_peak_bytes();
    let mut admitted = owner(&[1]);
    admitted
        .declare_predicate_with(
            &predicate,
            Limits {
                max_bytes: exact,
                ..limits()
            },
            || Ok::<(), Infallible>(()),
        )
        .unwrap();
    let mut refused = owner(&[1]);
    assert!(matches!(refused.declare_predicate_with(&predicate,
        Limits { max_bytes: exact - 1, ..limits() }, || Ok::<(), Infallible>(())),
        Err(Failure::Bytes { required, limit }) if required == exact && limit == exact - 1));
    assert_eq!(refused.len(), 1);
}

#[test]
fn publication_byte_refusal_includes_metadata_once() {
    let mut measured = owner(&[1]);
    measured.restart_storage_peak();
    measured
        .commit_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    let exact = measured.storage_peak_bytes();
    let mut admitted = owner(&[1]);
    admitted
        .commit_with(
            Limits {
                max_bytes: exact,
                ..limits()
            },
            || Ok::<(), Infallible>(()),
        )
        .unwrap();
    let mut refused = owner(&[1]);
    assert!(
        matches!(refused.commit_with(Limits { max_bytes: exact - 1, ..limits() },
        || Ok::<(), Infallible>(())), Err(Failure::Bytes { required, limit })
        if required == exact && limit == exact - 1)
    );
    assert!(refused.committed().is_empty());
    assert_eq!(refused.len(), 1);
}

#[test]
fn store_representation_limit_is_not_mislabeled_as_owner_exhaustion() {
    let required = usize::MAX as u128 + 1;
    let error: Failure<Infallible> = store_failure(
        storage::Fault::Storage {
            required,
            limit: usize::MAX,
        },
        7,
        Limits {
            max_bytes: u128::MAX,
            ..limits()
        },
    );
    assert!(matches!(error, Failure::Catalog(storage::Fault::Storage {
        required: actual, limit: usize::MAX,
    }) if actual == required));
}
