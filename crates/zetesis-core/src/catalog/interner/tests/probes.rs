//! Pure membership, occupied-entry scratch and independent probe-work controls.

use super::*;

fn pattern() -> AtomPattern {
    AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap()
}

// These fixtures query canonical rows with borrowed ingress signatures/values,
// so they do not take the same-canonical-ID shortcut. Finding p/1 reads the
// query signature, probes its relation, resolves both signatures, compares the
// signature descriptor, then compares the 'p' byte and the string terminator.
const SIGNATURE_WORK: usize = 7;
// Inspect the query and its predicate representation before selecting the
// semantic ingress fallback. These fixtures contain no canonical query IDs.
const CANONICAL_APPLICABILITY_WORK: usize = 2;
// Each visited numeric row resolves the row, arity and argument, then reads
// each root descriptor once, compares storage ranks and compares the numbers.
// Scalars have no descendants; equality no longer visits two cursor ends or
// compares their lengths. Equal and unequal numeric rows cost the same here.
const NUMERIC_NODE_WORK: usize = 3 + 2 + 1 + 1;
// A prepared vacant route replays one node/direction and writes one Step.
const REPLAY_NODE_WORK: usize = 2;
// The subtree holds its last atom, p(3): one numeric comparison with it
// precedes any search, and only an atom beyond it walks the right spine.
const LAST_WORK: usize = NUMERIC_NODE_WORK;

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
    // p(2) is the root; the other queries visit two numeric nodes.
    for (value, expected, nodes) in [
        (2, Some(0), 1),
        (1, Some(1), 2),
        (3, Some(2), 2),
        (0, None, 2),
        (4, None, 2),
    ] {
        let work = CANONICAL_APPLICABILITY_WORK + SIGNATURE_WORK + nodes * NUMERIC_NODE_WORK;
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
        let total = CANONICAL_APPLICABILITY_WORK + SIGNATURE_WORK + 2 * NUMERIC_NODE_WORK;
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
    assert_eq!(atom_work, CANONICAL_APPLICABILITY_WORK + 1);
    assert_eq!(key_work, CANONICAL_APPLICABILITY_WORK + 1);
}

#[test]
fn vacant_entries_charge_link_replay() {
    let mut owner = owner(&[2, 1, 3]);
    // Exercise cold path preparation independently of published-spine reuse.
    owner.spines.semantic = None;
    assert!(owner.index.path.capacity() >= 2);
    let mut spent = 0;
    // p(0) precedes the last atom, p(3): after the one comparison with it,
    // and no spine walk, the full search runs.
    let absent = atom(0);
    let entry = owner
        .entry_atom_with(&absent, limits(), || {
            spent += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
    assert_eq!(entry.position(), None);
    // The two typed probes record the route. Replay does not repeat those
    // comparisons or grow the already available path storage.
    assert_eq!(
        spent,
        CANONICAL_APPLICABILITY_WORK
            + SIGNATURE_WORK
            + LAST_WORK
            + 2 * (NUMERIC_NODE_WORK + REPLAY_NODE_WORK)
    );
    assert_eq!(
        entry
            .insert_with(limits(), || Ok::<(), Infallible>(()))
            .unwrap(),
        3
    );
    validate(&owner);
}

#[test]
fn beyond_last_preparation_visits_the_spine_once() {
    let mut owner = owner(&[2, 1, 3]);
    // Exercise cold path preparation independently of published-spine reuse.
    owner.spines.semantic = None;
    let mut spent = 0;
    let beyond = atom(4);
    let entry = owner
        .entry_atom_with(&beyond, limits(), || {
            spent += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
    assert_eq!(entry.position(), None);
    // One comparison with the last atom; the route is its right spine, root
    // p(2) and p(3). Each node is read and retained once, with no preliminary
    // direction-recording walk and no further typed comparison.
    assert_eq!(
        spent,
        CANONICAL_APPLICABILITY_WORK + SIGNATURE_WORK + LAST_WORK + 2 * REPLAY_NODE_WORK
    );
    assert_eq!(
        entry
            .insert_with(limits(), || Ok::<(), Infallible>(()))
            .unwrap(),
        3
    );
    validate(&owner);
}

#[test]
fn stopped_monotone_preparation_preserves_discovery() {
    let initial: Vec<_> = (0..31).collect();
    let reference = owner(&initial);
    let mut spine = 0;
    let mut cursor = reference.subtrees[0].root;
    while let Some(node) = cursor {
        spine += 1;
        cursor = reference.index.nodes[position(node)].children[1];
    }
    assert!(spine > 2, "exercise more than the small rotation fixtures");
    assert!(reference.index.path.capacity() >= spine);
    let required =
        CANONICAL_APPLICABILITY_WORK + SIGNATURE_WORK + LAST_WORK + spine * REPLAY_NODE_WORK;
    let added = atom(31);
    for cutoff in 0..=required {
        let mut owner = owner(&initial);
        // This boundary sweep exercises the cold right-spine builder.
        owner.spines.semantic = None;
        let mut spent = 0;
        let result = owner.entry_atom_with(&added, limits(), || {
            if spent == cutoff {
                Err(cutoff)
            } else {
                spent += 1;
                Ok(())
            }
        });
        if cutoff == required {
            let entry = result.unwrap();
            assert_eq!(entry.position(), None);
            assert_eq!(spent, required);
            // Preparing the admitted path has not yet published discovery.
            assert_eq!(entry.appender.len(), initial.len());
        } else {
            assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == cutoff));
            assert_eq!(spent, cutoff);
        }
        assert_eq!(owner.len(), initial.len());
        validate(&owner);
        assert_eq!(insert(&mut owner, &added), initial.len());
        validate(&owner);
    }
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
    // intentionally exercised both indexes while every identity was pending.
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
fn direction_record_preserves_word_boundaries() {
    let mut directions = Directions::default();
    let capacity = 2 * usize::BITS as usize;
    // Repeat runs of both directions across the word boundary and final bit.
    // Read every recorded prefix so an offset error cannot hide in a full word.
    for position in 0..capacity {
        directions.push(position % 3 == 0).unwrap();
        assert_eq!(directions.len(), position + 1);
        for previous in 0..=position {
            assert_eq!(directions.get(previous), Some(previous % 3 == 0));
        }
        assert_eq!(directions.get(position + 1), None);
    }
}

#[test]
fn full_direction_record_refuses_without_change() {
    let mut directions = Directions::default();
    let capacity = 2 * usize::BITS as usize;
    for position in 0..capacity {
        directions.push(position % 2 == 0).unwrap();
    }
    assert_eq!(directions.push(true), None);
    assert_eq!(directions.push(false), None);
    assert_eq!(directions.len(), capacity);
    for position in 0..capacity {
        assert_eq!(directions.get(position), Some(position % 2 == 0));
    }
    assert_eq!(directions.get(capacity), None);
    assert_eq!(directions.get(usize::MAX), None);
}

#[test]
fn replay_refusal_preserves_published_membership() {
    // p(0) takes the relation lookup, the comparison with the last atom and two
    // unequal typed probes, followed by two replay node/Step pairs. Refuse each
    // pair's node read and Step write.
    let lookup = SIGNATURE_WORK + LAST_WORK + 2 * NUMERIC_NODE_WORK;
    for limit in lookup..lookup + 2 * REPLAY_NODE_WORK {
        let mut owner = owner(&[2, 1, 3]);
        assert!(owner.index.path.capacity() >= 2);
        owner
            .commit_with(limits(), || Ok::<(), Infallible>(()))
            .unwrap();
        let original = owner.committed.as_ptr();
        let cause = ("replay stopped", limit);
        let mut spent = 0;
        let added = atom(0);
        let result = owner.entry_atom_with(&added, limits(), || {
            if spent == limit {
                Err(&cause)
            } else {
                spent += 1;
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(Failure::Stopped(actual)) if std::ptr::eq(actual, &raw const cause))
        );
        assert_eq!(spent, limit);
        assert_eq!(owner.len(), 3);
        assert_eq!(owner.committed.as_ptr(), original);
        for (id, value) in [2, 1, 3].into_iter().enumerate() {
            assert_eq!(owner.get(id), Some(AtomRef::from(&atom(value))));
        }
        validate(&owner);
        assert_eq!(insert(&mut owner, &added), 3);
        assert_eq!(owner.get(3), Some(AtomRef::from(&added)));
        validate(&owner);
    }
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
