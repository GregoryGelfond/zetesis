//! Pure membership, occupied-entry scratch and independent probe-work controls.

use super::*;

fn pattern() -> AtomPattern {
    AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap()
}

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
    assert!(std::ptr::eq(
        committed.get(0).unwrap(),
        shared.get(0).unwrap()
    ));
    assert!(std::ptr::eq(pending, shared.get(3).unwrap()));
    assert_eq!(committed.len(), 3);
    assert_eq!(shared.len(), 4);
}

#[test]
fn occupied_entries_leave_scratch_unchanged() {
    let mut owner = owner(&[2, 1, 3]);
    let snapshot = |owner: &AtomInterner| {
        owner
            .index
            .path
            .iter()
            .map(|step| {
                (
                    step.id,
                    step.node.children,
                    step.node.balance,
                    step.right,
                    step.changed,
                )
            })
            .collect::<Vec<_>>()
    };
    let original = snapshot(&owner);
    assert!(!original.is_empty());
    let capacity = owner.index.path.capacity();
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
        assert_eq!(snapshot(&owner), original);
        assert_eq!(owner.index.path.capacity(), capacity);
        assert_eq!(owner.storage_peak_bytes(), peak);
    }
}

#[test]
fn occupied_entries_need_no_scratch_capacity() {
    let mut owner = owner(&[2, 1, 3]);
    // Mutation scratch is disposable; dropping it changes no published links.
    owner.index.path = Vec::new();
    let live = owner.storage_bytes();
    let bounds = Limits {
        max_bytes: live,
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
    assert_eq!(owner.index.path.capacity(), 0);
    assert_eq!(owner.storage_bytes(), live);
    validate(&owner);
}

#[test]
fn read_only_misses_need_no_mutation_storage() {
    let mut owner = owner(&[2, 1, 3]);
    owner.index.path = Vec::new();
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
    assert_eq!(owner.index.path.capacity(), 0);
    assert_eq!(owner.storage_bytes(), live);
    assert_eq!(owner.storage_peak_bytes(), peak);
    validate(&owner);
}

#[test]
fn probe_work_matches_visited_typed_prefixes() {
    let owner = owner(&[2, 1, 3]);
    let pattern = pattern();
    // Each p(number) visit costs a node, predicate descriptor, 'p' byte,
    // terminating name length and numeric Value descriptor: five operations.
    // p(2) is the root; every other query below visits exactly two nodes.
    for (value, expected, work) in [
        (2, Some(0), 5),
        (1, Some(1), 10),
        (3, Some(2), 10),
        (0, None, 10),
        (4, None, 10),
    ] {
        let mut spent = 0;
        let actual = owner
            .find_atom_with(&atom(value), limits(), || {
                spent += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(spent, work);
        let values = [Value::Number(value)];
        let mut spent = 0;
        let actual = owner
            .find_key_with(pattern.key(values.as_slice()).unwrap(), limits(), || {
                spent += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(spent, work);
    }
}

#[test]
fn query_work_limits_are_inclusive() {
    let owner = owner(&[2, 1, 3]);
    let pattern = pattern();
    for value in [3, 4] {
        let values = [Value::Number(value)];
        let key = pattern.key(values.as_slice()).unwrap();
        for limit in 0..=10 {
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
            if limit < 10 {
                assert!(
                    matches!(result, Err(Failure::Stopped(actual)) if std::ptr::eq(actual, &raw const cause))
                );
                assert_eq!(spent, limit);
            } else {
                assert_eq!(result.unwrap(), (value == 3).then_some(2));
                assert_eq!(spent, 10);
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
fn empty_queries_perform_no_operations() {
    let owner = AtomInterner::new();
    let atom = Atom::new(Predicate::new("empty", 0).unwrap(), vec![]).unwrap();
    let pattern = AtomPattern::new(atom.predicate().clone(), vec![]).unwrap();
    assert_eq!(
        owner
            .find_atom_with(&atom, limits(), || Err("no node"))
            .unwrap(),
        None
    );
    assert_eq!(
        owner
            .find_key_with(pattern.key(&[] as &[Value]).unwrap(), limits(), || Err(
                "no node"
            ))
            .unwrap(),
        None
    );
}

#[test]
fn vacant_entries_charge_both_searches() {
    let mut owner = owner(&[2, 1, 3]);
    assert!(owner.index.path.capacity() >= 2);
    let mut spent = 0;
    let absent = atom(4);
    let entry = owner
        .entry_atom_with(&absent, limits(), || {
            spent += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
    assert_eq!(entry.position(), None);
    // Each search visits two five-operation nodes; only the second search
    // writes the two path steps. Existing capacity needs no growth allowance.
    assert_eq!(spent, 2 * 10 + 2);
    assert_eq!(
        entry
            .insert_with(limits(), || Ok::<(), Infallible>(()))
            .unwrap(),
        3
    );
    validate(&owner);
}
