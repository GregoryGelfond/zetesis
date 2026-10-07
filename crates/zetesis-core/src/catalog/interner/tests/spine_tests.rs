//! Published-path authority, ordered fallback, and atomic retry boundaries.

use super::*;
use crate::test_support::PERMIT;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn assert_path(index: &Index, root: Link) {
    let mut cursor = root;
    for step in &index.path {
        assert_eq!(cursor.map(position), Some(step.id));
        let node = index.nodes[step.id];
        assert_eq!(step.node.children, node.children);
        assert_eq!(step.node.balance, node.balance);
        assert!(step.right);
        assert!(!step.changed);
        cursor = node.children[1];
    }
    assert!(
        cursor.is_none(),
        "a certificate covers the complete right spine"
    );
}

fn assert_spines(owner: &AtomInterner) {
    if let Some(spine) = owner.spines.semantic {
        let relation = owner
            .subtrees
            .iter()
            .find(|relation| spine.matches(&owner.index, relation.root, relation.last))
            .expect("semantic certificate belongs to one current subtree");
        assert_path(&owner.index, relation.root);
    }
    if let Some(spine) = owner.spines.discovery {
        assert!(spine.matches(&owner.discovery, owner.discovery.root, spine.last()));
        assert_path(&owner.discovery, owner.discovery.root);
        let last = discovery::identity(&owner.committed, &owner.pending, spine.last()).unwrap();
        assert!(
            owner
                .committed
                .iter()
                .chain(&owner.pending)
                .all(|id| *id <= last)
        );
    }
}

fn counted(owner: &mut AtomInterner, input: &Atom) -> usize {
    let mut work = 0;
    let mut before = || {
        work += 1;
        Ok::<_, Infallible>(())
    };
    owner
        .entry_atom_with(input, limits(), &mut before)
        .unwrap()
        .insert_with(limits(), &mut before)
        .unwrap();
    work
}

#[test]
fn increasing_entries_reuse_the_published_path() {
    let values: Vec<_> = (0..31).collect();
    let mut warm = owner(&values);
    let mut cold = owner(&values);
    assert_spines(&warm);
    let depth = warm.index.path.len();
    assert!(depth > 2);
    cold.spines.semantic = None;
    let input = atom(31);
    let mut warm_work = 0;
    let mut cold_work = 0;
    {
        let _entry = warm
            .entry_atom_with(&input, limits(), || {
                warm_work += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap();
    }
    {
        let _entry = cold
            .entry_atom_with(&input, limits(), || {
                cold_work += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap();
    }
    // Cold preparation reads and copies every spine node. Warm preparation
    // checks one certificate and consumes it before returning a vacant entry.
    assert_eq!(cold_work, warm_work + 2 * depth - 1);
    assert!(warm.spines.semantic.is_none());
    assert_eq!(warm.len(), values.len());
    assert_path(&warm.index, warm.subtrees[0].root);
}

#[test]
fn monotone_publication_reduces_work_without_a_second_path() {
    let mut warm = AtomInterner::new();
    let mut cold = AtomInterner::new();
    let (mut warm_work, mut cold_work) = (0, 0);
    for value in 0..127 {
        cold.spines = Spines::default();
        warm_work += counted(&mut warm, &atom(value));
        cold_work += counted(&mut cold, &atom(value));
        assert_spines(&warm);
        assert!(warm.spines.semantic.is_some());
        assert!(warm.spines.discovery.is_some());
        validate(&warm);
    }
    assert!(warm_work < cold_work, "{warm_work} versus {cold_work}");
    assert_eq!(warm.index.path.capacity(), cold.index.path.capacity());
    assert_eq!(
        warm.discovery.path.capacity(),
        cold.discovery.path.capacity()
    );
    assert_eq!(warm.storage_bytes(), cold.storage_bytes());
}

#[test]
fn nonmonotone_and_predicate_switches_preserve_both_indexes() {
    let sequences = [
        (0..64).collect::<Vec<_>>(),
        (0..64).rev().collect(),
        (0..64).map(|value| value * 37 % 64).collect(),
    ];
    for sequence in sequences {
        let mut warm = AtomInterner::new();
        let mut cold = AtomInterner::new();
        for (offset, value) in sequence.into_iter().enumerate() {
            for (name, sign) in [("p", Sign::Positive), ("q", Sign::Negative)] {
                let input = Atom::new(
                    Predicate::with_sign(name, 1, sign).unwrap(),
                    vec![Value::Number(value)],
                )
                .unwrap();
                cold.spines = Spines::default();
                assert_eq!(insert(&mut warm, &input), insert(&mut cold, &input));
                assert_spines(&warm);
                // Occupied entries must not disturb either retained path.
                let retained = (warm.index.path.len(), warm.discovery.path.len());
                insert(&mut warm, &input);
                assert_spines(&warm);
                assert_eq!(retained, (warm.index.path.len(), warm.discovery.path.len()));
            }
            if offset % 11 == 0 {
                warm.commit_with(limits(), PERMIT).unwrap();
                cold.commit_with(limits(), PERMIT).unwrap();
                assert_eq!(
                    warm.ordered_ids_with(limits(), PERMIT).unwrap(),
                    cold.ordered_ids_with(limits(), PERMIT).unwrap()
                );
                assert!(warm.spines.semantic.is_none());
            }
            validate(&warm);
            validate(&cold);
            assert_spines(&warm);
            for id in 0..warm.len() {
                assert_eq!(warm.get(id), cold.get(id));
            }
        }
    }
}

#[test]
fn sparse_closed_ids_do_not_imply_increasing_discovery_keys() {
    // Canonical IDs follow this deliberately nonsemantic insertion order.
    let source = owner(&[40, 10, 60, 20, 50, 30, 70, 80]);
    let base = source.into_closed_with(limits(), PERMIT).unwrap();
    let mut warm = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let mut cold = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let canonical = warm.store.snapshot(0).unwrap().atom_count();
    let mut saw_nonincreasing = false;
    for value in [10, 20, 30, 40, 50, 60, 70] {
        let input = atom(value);
        cold.spines = Spines::default();
        assert_eq!(insert(&mut warm, &input), insert(&mut cold, &input));
        assert!(warm.spines.semantic.is_some());
        saw_nonincreasing |= warm.spines.discovery.is_none();
        assert_spines(&warm);
        validate(&warm);
        assert_eq!(warm.store.snapshot(0).unwrap().atom_count(), canonical);
    }
    assert!(
        saw_nonincreasing,
        "semantic order differs from the sparse ID order"
    );
    assert_eq!(warm.len(), 7);
    assert_eq!(warm.discovery.nodes.len(), 7);
}

fn prefix() -> AtomInterner {
    owner(&(0..30).collect::<Vec<_>>())
}

fn attempt(
    owner: &mut AtomInterner,
    before: &mut impl FnMut() -> Result<(), usize>,
) -> Result<usize, Failure<usize>> {
    let input = atom(30);
    owner
        .entry_atom_with(&input, limits(), &mut *before)
        .and_then(|entry| entry.insert_with(limits(), before))
}

#[test]
fn every_warm_publication_stop_retries_the_same_identity() {
    let mut complete = prefix();
    let previous_depth = complete.index.path.len();
    let mut required = 0;
    attempt(&mut complete, &mut || {
        required += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        complete.index.path.len(),
        previous_depth,
        "a rotation removes one old spine node after adding the leaf"
    );
    assert_spines(&complete);
    for cutoff in 0..required {
        let mut owner = prefix();
        let mut work = 0;
        let result = attempt(&mut owner, &mut || {
            if work == cutoff {
                Err(cutoff)
            } else {
                work += 1;
                Ok(())
            }
        });
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == cutoff));
        assert_eq!(owner.len(), 30);
        // A still-present certificate can only describe untouched published
        // scratch; any path that reached preparation has already revoked it.
        assert_spines(&owner);
        validate(&owner);
        assert_eq!(insert(&mut owner, &atom(30)), 30);
        assert_spines(&owner);
        validate(&owner);
        for id in 0..owner.len() {
            assert_eq!(owner.get(id), complete.get(id));
        }
        assert!(owner.storage_peak_bytes() >= owner.storage_bytes());
    }
}

#[test]
fn every_warm_callback_unwind_leaves_retriable_published_paths() {
    let mut complete = prefix();
    let mut required = 0;
    attempt(&mut complete, &mut || {
        required += 1;
        Ok(())
    })
    .unwrap();
    for cutoff in 0..required {
        let mut owner = prefix();
        let mut work = 0;
        let failure = catch_unwind(AssertUnwindSafe(|| {
            let _ = attempt(&mut owner, &mut || {
                assert_ne!(work, cutoff, "injected work-callback unwind");
                work += 1;
                Ok(())
            });
        }));
        assert!(failure.is_err());
        assert_eq!(owner.len(), 30);
        assert_spines(&owner);
        validate(&owner);
        assert_eq!(insert(&mut owner, &atom(30)), 30);
        assert_spines(&owner);
        validate(&owner);
    }
}

#[test]
fn retained_spines_still_require_actual_owner_storage() {
    let mut owner = prefix();
    let occupied = atom(29);
    let exact = owner.storage_bytes() + PREPARED_BYTES;
    assert!(
        owner
            .entry_atom_with(
                &occupied,
                Limits {
                    max_bytes: exact,
                    ..limits()
                },
                PERMIT
            )
            .is_ok()
    );
    assert!(matches!(
        owner.entry_atom_with(
            &occupied,
            Limits {
                max_bytes: exact - 1,
                ..limits()
            },
            PERMIT
        ),
        Err(Failure::Bytes { .. })
    ));
    assert_spines(&owner);
    let input = atom(30);
    assert!(matches!(
        owner
            .entry_atom_with(&input, limits(), PERMIT)
            .unwrap()
            .insert_with(
                Limits {
                    max_atoms: 30,
                    ..limits()
                },
                PERMIT
            ),
        Err(Failure::Atoms {
            required: 31,
            limit: 30
        })
    ));
    assert_eq!(owner.len(), 30);
    assert_eq!(insert(&mut owner, &input), 30);
    validate(&owner);
}
