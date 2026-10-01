//! Publication cost and prefix preservation through successive support rounds.

use super::*;

fn publication_work(count: i32) -> usize {
    let mut owner = AtomInterner::new();
    let mut work = 0;
    for value in 0..count {
        insert(&mut owner, &atom(value));
        owner
            .commit_with(limits(), || {
                work += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap();
    }
    let committed = owner.committed();
    assert_eq!(committed.len(), usize::try_from(count).unwrap());
    for value in 0..count {
        assert_eq!(
            committed.get(usize::try_from(value).unwrap()),
            Some(AtomRef::from(&atom(value)))
        );
    }
    work
}

#[test]
fn successive_publications_have_linear_work() {
    let small = publication_work(128);
    let large = publication_work(256);
    // Doubling the complete carrier may cross capacity boundaries, but must
    // not recopy its entire history at every publication (quadratic work).
    assert!(large < 3 * small, "publication work: {small} -> {large}");
}

fn committed_rounds(count: i32) -> AtomInterner {
    let mut owner = AtomInterner::new();
    for value in 0..count {
        insert(&mut owner, &atom(value));
        owner
            .commit_with(limits(), || Ok::<_, Infallible>(()))
            .unwrap();
    }
    owner
}

#[test]
fn retained_snapshot_preserves_its_readable_prefix() {
    let mut owner = committed_rounds(4);
    let prior = owner.snapshot.clone();
    let prior_bytes = prior.retained_bytes();
    let original = owner.committed.clone();
    assert!(prior.shares_snapshot(&owner.snapshot));
    for value in 4..24 {
        insert(&mut owner, &atom(value));
        let added = *owner.pending.last().unwrap();
        owner
            .commit_with(limits(), || Ok::<_, Infallible>(()))
            .unwrap();
        // The first publication must preserve the shared prefix. Subsequent
        // exclusive publications must not extend it indirectly either.
        assert!(!prior.shares_snapshot(&owner.snapshot));
        assert_eq!(prior.atom_count(), 4);
        assert_eq!(prior.retained_bytes(), prior_bytes);
        assert!(AtomRef::new(&prior, added).is_none());
        for (id, value) in original.iter().zip(0..4) {
            assert_eq!(AtomRef::new(&prior, *id), Some(AtomRef::from(&atom(value))));
        }
    }
}

fn pending_growth() -> AtomInterner {
    // Four separately sealed rounds fill the initial directory capacities;
    // the fifth publication exercises their geometric growth boundary.
    let mut owner = committed_rounds(4);
    insert(&mut owner, &atom(4));
    owner.restart_storage_peak();
    owner
}

fn measured_growth() -> (u128, usize) {
    let mut owner = pending_growth();
    let before = owner.storage_bytes();
    let mut operations = 0;
    owner
        .commit_with(limits(), || {
            operations += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert!(owner.storage_peak_bytes() > before);
    (owner.storage_peak_bytes(), operations)
}

#[test]
fn every_growth_stop_preserves_the_committed_prefix() {
    let (_, operations) = measured_growth();
    for cutoff in 0..operations {
        let mut owner = pending_growth();
        let mut spent = 0;
        let result = owner.commit_with(limits(), || {
            if spent == cutoff {
                Err(cutoff)
            } else {
                spent += 1;
                Ok(())
            }
        });
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == cutoff));
        assert_eq!(spent, cutoff);
        assert_eq!(owner.snapshot.atom_count(), 4);
        assert_sequence(&owner, &owner.committed, &[0, 1, 2, 3]);
        assert_sequence(&owner, &owner.pending, &[4]);
        assert!(AtomRef::new(&owner.snapshot, owner.pending[0]).is_none());
        owner
            .commit_with(limits(), || Ok::<_, Infallible>(()))
            .unwrap();
        assert_sequence(&owner, &owner.committed, &[0, 1, 2, 3, 4]);
        assert!(owner.pending.is_empty());
    }
}

#[test]
fn publication_accepts_its_exact_composed_peak() {
    let (peak, _) = measured_growth();
    let mut owner = pending_growth();
    owner
        .commit_with(
            Limits {
                max_bytes: peak,
                ..limits()
            },
            || Ok::<_, Infallible>(()),
        )
        .unwrap();
    assert_eq!(owner.storage_peak_bytes(), peak);
    assert_sequence(&owner, &owner.committed, &[0, 1, 2, 3, 4]);
}

#[test]
fn refused_publication_growth_retries_the_same_suffix() {
    let (peak, operations) = measured_growth();
    let mut owner = pending_growth();
    let mut admitted = 0;
    let result = owner.commit_with(
        Limits {
            max_bytes: peak - 1,
            ..limits()
        },
        || {
            admitted += 1;
            Ok::<_, Infallible>(())
        },
    );
    assert!(matches!(result, Err(Failure::Bytes { required, limit })
        if required == peak && limit == peak - 1));
    // All work permits passed: this is a publication capacity refusal, not
    // an earlier discovery-buffer or callback refusal.
    assert_eq!(admitted, operations);
    assert_eq!(owner.snapshot.atom_count(), 4);
    assert_sequence(&owner, &owner.committed, &[0, 1, 2, 3]);
    assert_sequence(&owner, &owner.pending, &[4]);
    assert!(AtomRef::new(&owner.snapshot, owner.pending[0]).is_none());
    assert!(owner.storage_peak_bytes() >= owner.storage_bytes());
    owner
        .commit_with(limits(), || Ok::<_, Infallible>(()))
        .unwrap();
    assert_sequence(&owner, &owner.committed, &[0, 1, 2, 3, 4]);
    assert!(owner.pending.is_empty());
    validate(&owner);
}

#[test]
fn vocabulary_publication_needs_no_discovered_atom() {
    let mut owner = AtomInterner::new();
    let mut terms = Vec::new();
    for value in 0..12 {
        let id = owner
            .store
            .import_value(&Value::Number(value), TermLimits::default())
            .unwrap();
        assert!(crate::catalog::TermRef::new(&owner.snapshot, id).is_none());
        terms.push(id);
        owner
            .commit_with(limits(), || Ok::<_, Infallible>(()))
            .unwrap();
        assert!(owner.committed().is_empty());
        assert_eq!(owner.snapshot.atom_count(), 0);
        assert_eq!(owner.snapshot.term_count(), terms.len());
        for (id, expected) in terms.iter().zip(0..) {
            assert_eq!(
                crate::catalog::TermRef::new(&owner.snapshot, *id)
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(expected),
            );
        }
    }
}

#[test]
fn frozen_publications_preserve_the_shared_vocabulary() {
    let mut vocabulary = Store::new(usize::MAX);
    for value in 0..12 {
        vocabulary
            .import_value(&Value::Number(value), TermLimits::default())
            .unwrap();
    }
    vocabulary
        .import_predicate_with(PredicateRef::from(&Predicate::new("p", 1).unwrap()), || {
            Ok::<_, Infallible>(())
        })
        .unwrap();
    let base = vocabulary
        .freeze_vocabulary_with(0, || Ok::<_, Infallible>(()))
        .unwrap();
    let mut owner = AtomInterner::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let bytes = owner.shared_vocabulary_bytes();
    for value in 0..12 {
        insert(&mut owner, &atom(value));
        owner
            .commit_with(limits(), || Ok::<_, Infallible>(()))
            .unwrap();
        assert_eq!(owner.shared_vocabulary_bytes(), bytes);
        assert_eq!(owner.snapshot.term_count(), 12);
        assert!(storage::Read::from(&owner.snapshot).same_vocabulary(storage::Read::from(&base)));
    }
    assert_sequence(&owner, &owner.committed, &(0..12).collect::<Vec<_>>());
}
