//! Canonical scope, sparse discovery and indivisible inverse publication.

use super::*;
use crate::test_support::PERMIT;
use crate::{PatternRef, TemplateTerm};

fn text_owner(width: usize) -> AtomInterner {
    let mut owner = AtomInterner::new();
    for suffix in 0..9 {
        let atom = Atom::new(
            Predicate::new("p", 1).unwrap(),
            vec![Value::String(format!("{}{suffix}", "a".repeat(width)))],
        )
        .unwrap();
        insert(&mut owner, &atom);
    }
    owner
}

fn atom_lookup_work(width: usize) -> usize {
    let owner = text_owner(width);
    let mut work = 0;
    assert_eq!(
        owner
            .find_atom_with(owner.get(0).unwrap(), limits(), || {
                work += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap(),
        Some(0)
    );
    work
}

#[test]
fn canonical_atom_lookup_does_not_compare_text() {
    // The queried first discovery is not the semantic AVL root. An ordered
    // content search would inspect unequal strings with their common prefix.
    assert_eq!(atom_lookup_work(1), atom_lookup_work(4096));
}

fn key_lookup_work(width: usize) -> usize {
    let owner = text_owner(width);
    let atom = owner.get(0).unwrap();
    let terms = [TemplateTerm::Constant(atom.values().at(0).unwrap())];
    let pattern = PatternRef::from_parts(atom.predicate(), &terms).unwrap();
    let key = pattern.key(&[] as &[Value]).unwrap();
    let mut work = 0;
    assert_eq!(
        owner
            .find_key_with(key, limits(), || {
                work += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap(),
        Some(0)
    );
    work
}

#[test]
fn canonical_key_lookup_does_not_compare_text() {
    assert_eq!(key_lookup_work(1), key_lookup_work(4096));
}

#[test]
fn canonical_occupied_entries_need_no_path_storage() {
    let mut owner = owner(&[2, 1, 3]);
    owner.commit_with(limits(), PERMIT).unwrap();
    owner.index.path = Vec::new();
    owner.discovery.path = Vec::new();
    owner.restart_storage_peak();
    let bytes = owner.storage_bytes();
    let exact = Limits {
        max_bytes: bytes + PREPARED_BYTES,
        ..limits()
    };
    {
        let (prefix, mut append) = owner.split();
        let atom = prefix.get(1).unwrap();
        let terms = [TemplateTerm::Constant(atom.values().at(0).unwrap())];
        let pattern = PatternRef::from_parts(atom.predicate(), &terms).unwrap();
        let key = pattern.key(&[] as &[Value]).unwrap();
        assert_eq!(
            append
                .entry_atom_with(atom, exact, PERMIT)
                .unwrap()
                .position(),
            Some(1)
        );
        assert_eq!(
            append
                .entry_key_with(key, exact, PERMIT)
                .unwrap()
                .position(),
            Some(1)
        );
    }
    assert_eq!(owner.index.path.capacity(), 0);
    assert_eq!(owner.discovery.path.capacity(), 0);
    assert_eq!(owner.storage_bytes(), bytes);
    assert_eq!(owner.storage_peak_bytes(), bytes + PREPARED_BYTES);
}

#[test]
fn shared_vocabulary_does_not_share_atom_positions() {
    let mut source = Store::new(usize::MAX);
    for value in [1, 2] {
        source
            .import_value(&Value::Number(value), TermLimits::default())
            .unwrap();
    }
    source
        .import_predicate_with((&Predicate::new("p", 1).unwrap()).into(), PERMIT)
        .unwrap();
    let vocabulary = source.freeze_vocabulary_with(0, PERMIT).unwrap();
    let mut left = AtomInterner::with_vocabulary(vocabulary.clone(), usize::MAX).unwrap();
    let mut right = AtomInterner::with_vocabulary(vocabulary, usize::MAX).unwrap();
    insert(&mut left, &atom(1));
    insert(&mut right, &atom(2));
    assert_eq!(left.pending[0], right.pending[0]);
    assert!(
        left.read()
            .storage()
            .same_vocabulary(right.read().storage())
    );
    assert!(!left.read().storage().same_atoms(right.read().storage()));
    assert_eq!(
        left.find_atom_with(right.get(0).unwrap(), limits(), PERMIT)
            .unwrap(),
        None
    );
    insert(&mut left, &atom(2));
    assert_eq!(
        left.find_atom_with(right.get(0).unwrap(), limits(), PERMIT)
            .unwrap(),
        Some(1)
    );
}

#[test]
fn foreign_canonical_queries_use_semantic_identity() {
    let local = owner(&[1, 2, 3]);
    let foreign = owner(&[3, 1, 8]);
    assert!(
        !local
            .read()
            .storage()
            .same_vocabulary(foreign.read().storage())
    );
    for (position, expected) in [Some(2), Some(0), None].into_iter().enumerate() {
        assert_eq!(
            local
                .find_atom_with(foreign.get(position).unwrap(), limits(), PERMIT)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn canonical_only_rows_are_not_discoveries() {
    let mut owner = AtomInterner::new();
    // Store admission is exactly the state allowed to survive an interrupted
    // interner publication. It does not publish either discovery index.
    let id = owner
        .store
        .import_atom(&atom(7), TermLimits::default())
        .unwrap();
    let row = AtomRef::new(&owner.store, id).unwrap();
    assert_eq!(owner.find_atom_with(row, limits(), PERMIT).unwrap(), None);
    assert!(owner.discovery.nodes.is_empty());
    assert!(owner.is_empty());
    assert_eq!(insert(&mut owner, &atom(2)), 0);
    assert_eq!(insert(&mut owner, &atom(7)), 1);
    assert_eq!(owner.pending[1], id);
    assert_eq!(
        owner
            .find_atom_with(owner.get(1).unwrap(), limits(), PERMIT)
            .unwrap(),
        Some(1)
    );
}

#[test]
fn inverse_rotations_preserve_discovery_positions() {
    fn height(owner: &AtomInterner, link: Link) -> i32 {
        let Some(link) = link else {
            return 0;
        };
        let node = owner.discovery.nodes[position(link)];
        let left = height(owner, node.children[0]);
        let right = height(owner, node.children[1]);
        assert!((right - left).abs() <= 1);
        assert_eq!(i32::from(node.balance), right - left);
        1 + left.max(right)
    }
    for order in [[0, 1, 2], [2, 1, 0], [2, 0, 1], [0, 2, 1]] {
        let mut owner = AtomInterner::new();
        for value in 0..3 {
            owner
                .store
                .import_atom(&atom(value), TermLimits::default())
                .unwrap();
        }
        // Identity allocation precedes this independently ordered discovery.
        for (position, value) in order.into_iter().enumerate() {
            assert_eq!(insert(&mut owner, &atom(value)), position);
            height(&owner, owner.discovery.root);
            validate(&owner);
        }
    }
}

#[test]
fn inverse_growth_obeys_the_combined_storage_peak() {
    let values: Vec<_> = (0..8).collect();
    let mut admitted = owner(&values);
    admitted.restart_storage_peak();
    let addition = atom(8);
    insert(&mut admitted, &addition);
    let exact = admitted.storage_peak_bytes();
    let mut exact_owner = owner(&values);
    exact_owner.restart_storage_peak();
    let exact_limits = Limits {
        max_bytes: exact,
        ..limits()
    };
    assert_eq!(
        exact_owner
            .entry_atom_with(&addition, exact_limits, PERMIT)
            .unwrap()
            .insert_with(exact_limits, PERMIT)
            .unwrap(),
        8
    );
    let mut refused = owner(&values);
    refused.restart_storage_peak();
    let short = Limits {
        max_bytes: exact - 1,
        ..limits()
    };
    let result = refused
        .entry_atom_with(&addition, short, PERMIT)
        .and_then(|entry| entry.insert_with(short, PERMIT));
    assert!(matches!(result, Err(Failure::Bytes { required, limit }) if required > limit));
    assert_eq!(refused.len(), 8);
    assert_eq!(refused.discovery.nodes.len(), 8);
    assert!(refused.storage_peak_bytes() >= refused.storage_bytes());
    validate(&refused);
}

#[test]
fn derived_allowance_covers_distinct_signed_predicates() {
    const ATOMS: usize = 64;
    // Explicit fixture allowance for text, signatures, rows and snapshots;
    // atom count by itself cannot bound arbitrary canonical payload.
    const CANONICAL_BYTES: usize = 64 * 1024;
    let limits = Limits::for_atoms(ATOMS, CANONICAL_BYTES);
    let mut owner = AtomInterner::new();
    let mut expected = BTreeSet::new();
    for index in 0..ATOMS {
        let sign = if index % 2 == 0 {
            Sign::Positive
        } else {
            Sign::Negative
        };
        let atom = Atom::new(
            Predicate::with_sign(format!("predicate_{}", index / 2), 0, sign).unwrap(),
            vec![],
        )
        .unwrap();
        let position = owner
            .entry_atom_with(&atom, limits, PERMIT)
            .unwrap()
            .insert_with(limits, PERMIT)
            .unwrap();
        assert_eq!(position, index);
        expected.insert(atom);
        if (index + 1) % 8 == 0 {
            owner.commit_with(limits, PERMIT).unwrap();
        }
    }
    assert_eq!(owner.subtrees.len(), ATOMS);
    assert!(
        owner.store.current_bytes() + owner.snapshot.metadata_bytes() <= CANONICAL_BYTES as u128
    );
    let order = owner.ordered_ids_with(limits, PERMIT).unwrap();
    assert!(owner.storage_peak_bytes() <= limits.max_bytes);
    assert_eq!(
        order
            .iter()
            .map(|&id| owner.get(id).unwrap())
            .collect::<Vec<_>>(),
        expected.iter().map(AtomRef::from).collect::<Vec<_>>()
    );
}

#[test]
fn every_inverse_publication_cut_keeps_prior_discovery() {
    let mut complete = owner(&[2, 1, 3]);
    let addition = atom(4);
    let mut operations = 0;
    let mut before = || {
        operations += 1;
        Ok::<_, Infallible>(())
    };
    complete
        .entry_atom_with(&addition, limits(), &mut before)
        .unwrap()
        .insert_with(limits(), &mut before)
        .unwrap();
    for cutoff in 0..operations {
        let mut owner = owner(&[2, 1, 3]);
        let mut accepted = 0;
        let mut before = || {
            if accepted == cutoff {
                Err(cutoff)
            } else {
                accepted += 1;
                Ok(())
            }
        };
        let result = owner
            .entry_atom_with(&addition, limits(), &mut before)
            .and_then(|entry| entry.insert_with(limits(), &mut before));
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == cutoff));
        assert_eq!(owner.len(), 3);
        assert_eq!(owner.discovery.nodes.len(), 3);
        validate(&owner);
        assert_eq!(insert(&mut owner, &addition), 3);
        assert_eq!(insert(&mut owner, &addition), 3);
        validate(&owner);
    }
}

#[test]
fn a_stopped_canonical_lookup_returns_no_membership() {
    let owner = owner(&[1, 2, 3]);
    let atom = owner.get(2).unwrap();
    let terms = [TemplateTerm::Constant(atom.values().at(0).unwrap())];
    let pattern = PatternRef::from_parts(atom.predicate(), &terms).unwrap();
    let key = pattern.key(&[] as &[Value]).unwrap();
    let before = (owner.storage_bytes(), owner.storage_peak_bytes());
    let mut operations = 0;
    owner
        .find_key_with(key, limits(), || {
            operations += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    for cutoff in 0..operations {
        let mut accepted = 0;
        let result = owner.find_key_with(key, limits(), || {
            if accepted == cutoff {
                Err(cutoff)
            } else {
                accepted += 1;
                Ok(())
            }
        });
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == cutoff));
        assert_eq!((owner.storage_bytes(), owner.storage_peak_bytes()), before);
    }
}
