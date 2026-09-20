//! Exact denotation, AVL shape, transaction refusal and scoped prefix controls.

mod probes;

use super::*;
use crate::{AtomCatalog, AtomPattern, Predicate, Sign, Term, Value, ValueLimits, ValueNode};
use proptest::prelude::*;
use std::{collections::BTreeSet, convert::Infallible};

fn limits() -> Limits {
    Limits {
        max_atoms: 1024,
        max_bytes: 16 * 1024 * 1024,
    }
}
fn atom(number: i32) -> Atom {
    Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(number)]).unwrap()
}
fn insert(owner: &mut AtomInterner, atom: &Atom) -> usize {
    owner
        .entry_atom_with(atom, limits(), || Ok::<(), Infallible>(()))
        .unwrap()
        .insert_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap()
}
fn owner(values: &[i32]) -> AtomInterner {
    let mut owner = AtomInterner::new();
    for &value in values {
        insert(&mut owner, &atom(value));
    }
    owner
}

// Literal recursive height/order validation is restricted to these small test
// populations. Production insertion and traversal are iterative.
fn validate(owner: &AtomInterner) {
    fn subtree(
        owner: &AtomInterner,
        root: index::Link,
        seen: &mut BTreeSet<usize>,
    ) -> (i32, Vec<Atom>) {
        let Some(root) = root else {
            return (0, Vec::new());
        };
        let id = position(root);
        assert!(seen.insert(id), "duplicate or cyclic link {id}");
        let node = owner.index.nodes[id];
        let (left_height, mut left) = subtree(owner, node.children[0], seen);
        let (right_height, right) = subtree(owner, node.children[1], seen);
        let atom = owner.get(id).unwrap();
        assert!(left.iter().all(|previous| previous < atom));
        assert!(right.iter().all(|next| atom < next));
        assert!((right_height - left_height).abs() <= 1);
        assert_eq!(i32::from(node.balance), right_height - left_height);
        left.push(atom.clone());
        left.extend(right);
        (1 + left_height.max(right_height), left)
    }
    let mut seen = BTreeSet::new();
    let mut actual = Vec::new();
    for (index, relation) in owner.subtrees.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1) {
            assert!(owner.subtrees[previous].predicate < relation.predicate);
        }
        let (_, atoms) = subtree(owner, relation.root, &mut seen);
        assert!(
            atoms
                .iter()
                .all(|atom| *atom.predicate() == relation.predicate)
        );
        actual.extend(atoms);
    }
    assert_eq!(seen, (0..owner.len()).collect());
    let expected: BTreeSet<_> = (0..owner.len())
        .map(|id| owner.get(id).unwrap().clone())
        .collect();
    assert_eq!(actual, expected.into_iter().collect::<Vec<_>>());
}

proptest! {
    #[test]
    fn append_ids_preserve_exact_typed_membership(values in prop::collection::vec(-100i32..100, 0..90)) {
        let mut owner = AtomInterner::new();
        let mut expected = Vec::new();
        for value in values {
            let atom = atom(value);
            let id = insert(&mut owner, &atom);
            let expected_id = expected.iter().position(|existing| existing == &atom).unwrap_or_else(|| {
                let id = expected.len(); expected.push(atom); id
            });
            prop_assert_eq!(id, expected_id);
            validate(&owner);
            for (id, expected) in expected.iter().enumerate() { prop_assert_eq!(owner.get(id), Some(expected)); }
        }
        owner.commit_with(limits(), || Ok::<(), Infallible>(())).unwrap();
        let order = owner.ordered_ids_with(limits(), || Ok::<(), Infallible>(())).unwrap();
        let actual: Vec<_> = order.iter().map(|&id| owner.get(id).unwrap().clone()).collect();
        expected.sort();
        prop_assert_eq!(actual, expected);
    }
}

#[test]
fn every_rotation_preserves_original_positions() {
    for values in [[3, 2, 1], [1, 2, 3], [3, 1, 2], [1, 3, 2]] {
        let mut owner = owner(&values);
        validate(&owner);
        owner
            .commit_with(limits(), || Ok::<(), Infallible>(()))
            .unwrap();
        let order = owner
            .ordered_ids_with(limits(), || Ok::<(), Infallible>(()))
            .unwrap();
        let mut expected: Vec<_> = (0..values.len()).collect();
        expected.sort_by_key(|&id| values[id]);
        assert_eq!(order, expected);
        for (id, value) in values.into_iter().enumerate() {
            assert_eq!(owner.get(id), Some(&atom(value)));
        }
    }
}

#[test]
fn refused_insert_preserves_the_published_tree() {
    for initial in [[3, 2], [1, 2], [3, 1], [1, 3]] {
        let added = atom(if initial == [3, 2] {
            1
        } else if initial == [1, 2] {
            3
        } else {
            2
        });
        let mut complete = owner(&initial);
        let mut operations = 0;
        complete
            .entry_atom_with(&added, limits(), || {
                operations += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap()
            .insert_with(limits(), || {
                operations += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap();
        for limit in 0..operations {
            let mut owner = owner(&initial);
            let mut spent = 0;
            let mut before = || {
                if spent == limit {
                    Err(limit)
                } else {
                    spent += 1;
                    Ok(())
                }
            };
            let result = owner
                .entry_atom_with(&added, limits(), &mut before)
                .and_then(|entry| entry.insert_with(limits(), &mut before));
            assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == limit));
            assert_eq!(spent, limit);
            assert_eq!(owner.len(), initial.len());
            for (id, value) in initial.into_iter().enumerate() {
                assert_eq!(owner.get(id), Some(&atom(value)));
            }
            validate(&owner);
            assert_eq!(insert(&mut owner, &added), initial.len());
            validate(&owner);
        }
    }
}

#[test]
fn committed_rows_survive_pending_growth() {
    let mut owner = owner(&[17]);
    owner
        .commit_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    {
        let (committed, mut append) = owner.split();
        let original = committed.get(0).unwrap();
        for value in 0..96 {
            let atom = atom(value);
            let entry = append
                .entry_atom_with(&atom, limits(), || Ok::<(), Infallible>(()))
                .unwrap();
            let id = entry
                .insert_with(limits(), || Ok::<(), Infallible>(()))
                .unwrap();
            assert_eq!(append.get(id), Some(&atom));
            assert_eq!(committed.len(), 1);
            assert!(std::ptr::eq(original, committed.get(0).unwrap()));
            assert_eq!(committed.get(1), None);
        }
    }
    owner
        .commit_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    assert_eq!(owner.len(), 96);
    assert_eq!(owner.get(0), Some(&atom(17)));
    validate(&owner);
}

#[test]
fn final_transfer_keeps_identity_only_tail() {
    let mut owner = owner(&[7]);
    owner
        .commit_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    insert(&mut owner, &atom(-1));
    assert_eq!(owner.committed.len(), 1);
    let atoms = owner
        .into_atoms_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    assert_eq!(atoms, [atom(7), atom(-1)]);
    let pointer = atoms.as_ptr();
    let catalog = AtomCatalog::new(atoms);
    assert_eq!(catalog.atoms().as_ptr(), pointer);
    assert_eq!(catalog.atoms(), [atom(7), atom(-1)]);
}

#[test]
fn duplicate_at_population_limit_copies_no_payload() {
    let mut owner = owner(&[1, 2]);
    let bounds = Limits {
        max_atoms: 2,
        ..limits()
    };
    let duplicate = atom(1);
    let entry = owner
        .entry_atom_with(&duplicate, bounds, || Ok::<(), Infallible>(()))
        .unwrap();
    assert_eq!(entry.position(), Some(0));
    assert_eq!(
        entry
            .insert_with(bounds, || Err("copy must not run"))
            .unwrap(),
        0
    );
    let absent = atom(3);
    let result = owner
        .entry_atom_with(&absent, bounds, || Ok::<(), Infallible>(()))
        .unwrap()
        .insert_with(bounds, || Ok::<(), Infallible>(()));
    assert!(matches!(
        result,
        Err(Failure::Atoms {
            required: 3,
            limit: 2
        })
    ));
    validate(&owner);
}

#[test]
fn refused_work_retains_actual_reserved_capacity() {
    let mut owner = AtomInterner::new();
    let old = owner.storage_bytes();
    let atom = atom(1);
    let entry = owner
        .entry_atom_with(&atom, limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    let mut work = 0;
    let result = entry.insert_with(limits(), || {
        if work == 1 {
            Err("stop")
        } else {
            work += 1;
            Ok(())
        }
    });
    assert!(matches!(result, Err(Failure::Stopped("stop"))));
    assert!(owner.storage_bytes() > old);
    assert!(owner.storage_peak_bytes() >= owner.storage_bytes());
    assert!(owner.is_empty());
    validate(&owner);
}

#[test]
fn tighter_storage_refuses_before_lookup() {
    let mut owner = owner(&[1, 2, 3]);
    let current = owner.storage_bytes();
    let peak = owner.storage_peak_bytes();
    let atom = atom(1);
    let result = owner.entry_atom_with(
        &atom,
        Limits {
            max_bytes: current - 1,
            ..limits()
        },
        || Err("work"),
    );
    assert!(
        matches!(result, Err(Failure::Bytes { required, limit }) if required == current && limit == current - 1)
    );
    assert_eq!(owner.storage_bytes(), current);
    assert_eq!(owner.storage_peak_bytes(), peak);
    validate(&owner);
}

#[test]
fn borrowed_keys_preserve_structural_signed_identity() {
    let nested = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 2 },
            ValueNode::String("é".into()),
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 1,
            },
            ValueNode::Number(3),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let mut owner = AtomInterner::new();
    let values = [
        nested,
        Value::String("same".into()),
        Value::Symbol("same".into()),
    ];
    let mut expected = Vec::new();
    for sign in [Sign::Positive, Sign::Negative] {
        let pattern = AtomPattern::new(
            Predicate::with_sign("p", 2, sign).unwrap(),
            vec![Term::Variable(1), Term::Variable(0)],
        )
        .unwrap();
        for value in &values {
            let assignment = [value.clone(), Value::Number(4)];
            let key = pattern.key(assignment.as_slice()).unwrap();
            let materialized = key.to_atom();
            let id = owner
                .entry_key_with(key, limits(), || Ok::<(), Infallible>(()))
                .unwrap()
                .insert_with(limits(), || Ok::<(), Infallible>(()))
                .unwrap();
            assert_eq!(id, expected.len());
            expected.push(materialized.clone());
            assert_eq!(
                owner
                    .find_key_with(key, limits(), || Ok::<(), Infallible>(()))
                    .unwrap(),
                Some(id)
            );
            assert_eq!(
                owner
                    .find_atom_with(&materialized, limits(), || Ok::<(), Infallible>(()))
                    .unwrap(),
                Some(id)
            );
            assert_eq!(insert(&mut owner, &materialized), id);
            for actual in &expected {
                assert_eq!(
                    key.compare_identity_with(actual, || Ok::<(), Infallible>(()))
                        .unwrap(),
                    materialized.cmp(actual)
                );
            }
        }
    }
    owner
        .commit_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    let ids = owner
        .ordered_ids_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    let actual: Vec<_> = ids
        .iter()
        .map(|&id| owner.get(id).unwrap().clone())
        .collect();
    expected.sort();
    assert_eq!(actual, expected);
    validate(&owner);
}

#[test]
fn occupied_entry_rechecks_a_changed_population_limit() {
    let mut owner = owner(&[1, 2]);
    let duplicate = atom(1);
    let entry = owner
        .entry_atom_with(&duplicate, limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    assert_eq!(entry.position(), Some(0));
    let result = entry.insert_with(
        Limits {
            max_atoms: 1,
            ..limits()
        },
        || Err("no operation admitted"),
    );
    assert!(matches!(
        result,
        Err(Failure::Atoms {
            required: 2,
            limit: 1
        })
    ));
    assert_eq!(owner.len(), 2);
    assert_eq!(owner.get(0), Some(&atom(1)));
    assert_eq!(owner.get(1), Some(&atom(2)));
    validate(&owner);
}

fn staged() -> AtomInterner {
    let mut owner = owner(&[5, 1, 9]);
    owner
        .commit_with(limits(), || Ok::<(), Infallible>(()))
        .unwrap();
    for value in [7, 3, 11, 0] {
        insert(&mut owner, &atom(value));
    }
    owner
}

#[test]
fn refused_commit_keeps_the_original_prefix() {
    let mut complete = staged();
    let mut operations = 0;
    complete
        .commit_with(limits(), || {
            operations += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
    for limit in 0..operations {
        let mut owner = staged();
        let mut spent = 0;
        let result = owner.commit_with(limits(), || {
            if spent == limit {
                Err(limit)
            } else {
                spent += 1;
                Ok(())
            }
        });
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == limit));
        assert_eq!(spent, limit);
        assert_eq!(owner.committed, [atom(5), atom(1), atom(9)]);
        assert_eq!(owner.pending, [atom(7), atom(3), atom(11), atom(0)]);
        validate(&owner);
        owner
            .commit_with(limits(), || Ok::<(), Infallible>(()))
            .unwrap();
        assert_eq!(owner.committed, [5, 1, 9, 7, 3, 11, 0].map(atom));
        assert!(owner.pending.is_empty());
    }
}

#[test]
fn refused_order_exposes_no_partial_selection() {
    let mut complete = staged();
    let mut operations = 0;
    assert_eq!(
        complete
            .ordered_ids_with(limits(), || {
                operations += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap(),
        [1, 0, 2]
    );
    for limit in 0..operations {
        let mut owner = staged();
        let mut spent = 0;
        let result = owner.ordered_ids_with(limits(), || {
            if spent == limit {
                Err(limit)
            } else {
                spent += 1;
                Ok(())
            }
        });
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == limit));
        assert_eq!(spent, limit);
        validate(&owner);
        assert_eq!(
            owner
                .ordered_ids_with(limits(), || Ok::<(), Infallible>(()))
                .unwrap(),
            [1, 0, 2]
        );
        assert_eq!(owner.committed, [atom(5), atom(1), atom(9)]);
        assert_eq!(owner.pending, [atom(7), atom(3), atom(11), atom(0)]);
    }
}

#[test]
fn first_commit_transfers_the_pending_buffer() {
    let mut owner = owner(&[5, 1, 9]);
    let pointer = owner.pending.as_ptr();
    let capacity = owner.pending.capacity();
    let live = owner.storage_bytes();
    let bounds = Limits {
        max_bytes: live,
        ..limits()
    };
    owner
        .commit_with(bounds, || Ok::<(), Infallible>(()))
        .unwrap();
    assert_eq!(owner.committed.as_ptr(), pointer);
    assert_eq!(owner.committed.capacity(), capacity);
    assert_eq!(owner.committed, [atom(5), atom(1), atom(9)]);
    assert!(owner.pending.is_empty());
    assert_eq!(owner.storage_bytes(), live);
}
