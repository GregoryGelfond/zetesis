use std::sync::Arc;

use super::{Failure, Fault, FrozenVocabulary, PredicateId, Read, Store, TermId};
use crate::catalog::{AtomRef, PredicateRef, TermRef};
use crate::test_support::{PERMIT, unlimited as limits};
use crate::{Atom, Predicate, Value, ValueLimits, ValueNode};

fn base() -> (FrozenVocabulary, PredicateId, TermId, TermId) {
    let mut writer = Store::new(usize::MAX);
    let first = writer
        .import_value(&Value::String("shared".into()), limits())
        .unwrap();
    let second = writer
        .import_value(&Value::Symbol("shared".into()), limits())
        .unwrap();
    let predicate = writer
        .import_predicate_with(PredicateRef::from(&Predicate::new("p", 1).unwrap()), PERMIT)
        .unwrap();
    (
        writer.freeze_vocabulary_with(0, PERMIT).unwrap(),
        predicate,
        first,
        second,
    )
}
fn row(
    writer: &mut Store,
    base: &FrozenVocabulary,
    predicate: PredicateId,
    term: TermId,
) -> super::AtomId {
    writer
        .import_row_with(
            PredicateRef::new(base, predicate).unwrap(),
            [TermRef::new(base, term).unwrap()],
            limits(),
            PERMIT,
        )
        .unwrap()
}

#[test]
fn independent_writers_share_vocabulary_allocations() {
    let (base, predicate, first, _) = base();
    let mut left = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let mut right = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let left_id = row(&mut left, &base, predicate, first);
    let right_id = row(&mut right, &base, predicate, first);
    let left = left.snapshot(0).unwrap();
    let right = right.snapshot(0).unwrap();
    assert!(Arc::ptr_eq(&left.data.vocabulary, &base.data));
    assert!(Arc::ptr_eq(&right.data.vocabulary, &base.data));
    assert_eq!(
        AtomRef::new(&left, left_id).unwrap(),
        AtomRef::new(&right, right_id).unwrap()
    );
}

#[test]
fn independent_atom_scopes_do_not_equate_equal_ids() {
    let (base, predicate, first, second) = base();
    let mut left = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let mut right = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let left_id = row(&mut left, &base, predicate, first);
    let right_id = row(&mut right, &base, predicate, second);
    assert_eq!(left_id, right_id);
    assert!(Read::from(&left).same_vocabulary(Read::from(&right)));
    assert!(!Read::from(&left).same_atoms(Read::from(&right)));
    assert_ne!(
        AtomRef::new(&left, left_id).unwrap(),
        AtomRef::new(&right, right_id).unwrap()
    );
}

#[test]
fn foreign_atom_import_does_not_reuse_another_writer_id() {
    let (base, predicate, first, second) = base();
    let mut left = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let left_id = row(&mut left, &base, predicate, first);
    let mut right = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let other = row(&mut right, &base, predicate, second);
    let imported = right
        .import_atom_with(AtomRef::new(&left, left_id).unwrap(), limits(), PERMIT)
        .unwrap();
    assert_ne!(imported, other);
    assert_eq!(
        AtomRef::new(&right, imported).unwrap(),
        AtomRef::new(&left, left_id).unwrap()
    );
}

#[test]
fn frozen_existing_foreign_terms_use_exact_lookup() {
    let (base, _, first, _) = base();
    let mut writer = Store::with_vocabulary(base, usize::MAX).unwrap();
    assert_eq!(
        writer
            .import_value(&Value::String("shared".into()), limits())
            .unwrap(),
        first
    );
}

#[test]
fn closed_vocabulary_refuses_new_terms() {
    let (base, _, _, _) = base();
    let mut writer = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let before = writer.current_bytes();
    assert_eq!(
        writer.import_value(&Value::Number(7), limits()),
        Err(Fault::FrozenVocabulary)
    );
    assert_eq!(writer.current_bytes(), before);
    assert_eq!(writer.counts().terms, base.data.counts.terms);
}

#[test]
fn closed_vocabulary_refuses_new_predicates() {
    let (base, _, _, _) = base();
    let mut writer = Store::with_vocabulary(base, usize::MAX).unwrap();
    let predicate = Predicate::new("p", 2).unwrap();
    assert_eq!(
        writer.import_predicate_with(PredicateRef::from(&predicate), PERMIT),
        Err(Failure::Storage(Fault::FrozenVocabulary))
    );
}

#[test]
fn empty_prefix_carries_declared_scopes() {
    let mut writer = Store::new(usize::MAX);
    let empty = writer.empty_snapshot();
    assert!(Read::from(&empty).same_atoms(Read::from(&writer)));
    assert!(Read::from(&empty).same_vocabulary(Read::from(&writer)));
    let predicate = writer
        .import_predicate_with(
            PredicateRef::from(&Predicate::new("empty", 0).unwrap()),
            PERMIT,
        )
        .unwrap();
    assert!(writer.has_unpublished());
    assert!(!Read::from(&empty).contains_predicate(predicate));
    let current = writer.snapshot(0).unwrap();
    assert_eq!(current.atom_count(), 0);
    assert!(Read::from(&current).contains_predicate(predicate));
    assert!(!writer.has_unpublished());
}

#[test]
fn frozen_empty_prefix_exposes_base_without_rows() {
    let (base, predicate, term, _) = base();
    let writer = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let empty = writer.empty_snapshot();
    assert!(Read::from(&empty).contains_predicate(predicate));
    assert!(Read::from(&empty).contains_term(term));
    assert_eq!(empty.atom_count(), 0);
    assert!(Arc::ptr_eq(&empty.data.vocabulary, &base.data));
}

#[test]
fn vocabulary_freeze_refuses_materialized_atoms() {
    let mut writer = Store::new(usize::MAX);
    writer
        .import_atom(
            &Atom::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap(),
            limits(),
        )
        .unwrap();
    assert!(matches!(
        writer.freeze_vocabulary_with(0, PERMIT),
        Err(Failure::Storage(Fault::VocabularyHasAtoms))
    ));
}

#[test]
fn frozen_snapshot_outlives_program_base() {
    let (base, predicate, term, _) = base();
    let mut writer = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    let id = row(&mut writer, &base, predicate, term);
    let snapshot = writer.snapshot(0).unwrap();
    drop(writer);
    drop(base);
    let atom = AtomRef::new(&snapshot, id).unwrap();
    assert_eq!(atom.predicate().name(), "p");
    assert_eq!(
        atom.arguments().get(0).unwrap().descriptor(),
        crate::ValueNodeRef::String("shared")
    );
}

#[test]
fn tuple_snapshot_does_not_retain_frozen_indexes() {
    let (base, predicate, term, _) = base();
    let indexes = Arc::downgrade(&base.indexes);
    let mut writer = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    row(&mut writer, &base, predicate, term);
    let snapshot = writer.snapshot(0).unwrap();
    drop(base);
    drop(writer);
    assert!(indexes.upgrade().is_none());
    assert_eq!(snapshot.atom_count(), 1);
}

#[test]
fn frozen_writer_admission_includes_shared_base() {
    let (base, _, _, _) = base();
    let required = std::mem::size_of::<Store>() as u128 + base.retained_bytes();
    assert!(matches!(
        Store::with_vocabulary(base, usize::try_from(required - 1).unwrap()),
        Err(Fault::Storage { .. })
    ));
}

#[test]
fn frozen_vocabulary_handles_deep_terms_iteratively() {
    let mut nodes = vec![ValueNode::Tuple { arity: 1 }; 10_000];
    nodes.push(ValueNode::Number(0));
    let value = Value::from_nodes(
        nodes,
        ValueLimits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        },
    )
    .unwrap();
    let mut writer = Store::new(usize::MAX);
    let id = writer.import_value(&value, limits()).unwrap();
    let base = writer.freeze_vocabulary_with(0, PERMIT).unwrap();
    drop(value);
    let mut writer = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    assert_eq!(
        writer
            .import_term_with(TermRef::new(&base, id).unwrap(), limits(), PERMIT)
            .unwrap(),
        id
    );
    assert_eq!(TermRef::new(&base, id).unwrap().depth(), 10_001);
}

#[test]
fn freeze_preserves_existing_payload_allocations() {
    let mut writer = Store::new(usize::MAX);
    let id = writer
        .import_value(&Value::String("retained".into()), limits())
        .unwrap();
    let prefix = writer.snapshot(0).unwrap();
    let base = writer.freeze_vocabulary_with(0, PERMIT).unwrap();
    assert!(Arc::ptr_eq(
        &prefix.data.vocabulary.segments[0],
        &base.data.segments[0]
    ));
    assert!(Read::from(&prefix).same_vocabulary(Read::from(&base)));
    assert_eq!(
        TermRef::new(&prefix, id).unwrap(),
        TermRef::new(&base, id).unwrap()
    );
}

#[test]
fn every_freeze_stop_preserves_prior_prefix() {
    fn prepared() -> (Store, super::Snapshot, TermId) {
        let mut writer = Store::new(usize::MAX);
        let id = writer.import_value(&Value::Number(1), limits()).unwrap();
        let prefix = writer.snapshot(0).unwrap();
        writer.import_value(&Value::Number(2), limits()).unwrap();
        (writer, prefix, id)
    }
    let (writer, _, _) = prepared();
    let mut total = 0;
    writer
        .freeze_vocabulary_with(0, || {
            total += 1;
            PERMIT()
        })
        .unwrap();
    for cutoff in 0..total {
        let (writer, prefix, id) = prepared();
        let mut done = 0;
        let result = writer.freeze_vocabulary_with(0, || {
            if done == cutoff {
                return Err(cutoff);
            }
            done += 1;
            Ok(())
        });
        assert!(matches!(result, Err(Failure::Stopped(value)) if value == cutoff));
        assert_eq!(prefix.term_count(), 1);
        assert_eq!(
            TermRef::new(&prefix, id).unwrap().descriptor(),
            crate::ValueNodeRef::Number(1)
        );
    }
}

#[test]
fn freeze_admits_external_metadata_simultaneously() {
    let mut writer = Store::new(usize::MAX);
    writer.import_value(&Value::Number(1), limits()).unwrap();
    let held = writer.current_bytes();
    let metadata = 1024;
    writer
        .ceiling(usize::try_from(held + metadata).unwrap())
        .unwrap();
    assert!(matches!(
        writer.freeze_vocabulary_with(metadata, PERMIT),
        Err(Failure::Storage(Fault::Storage { .. }))
    ));
}

#[test]
fn frozen_prefix_does_not_retain_future_rows() {
    let (base, predicate, first, second) = base();
    let mut writer = Store::with_vocabulary(base.clone(), usize::MAX).unwrap();
    row(&mut writer, &base, predicate, first);
    let previous = writer.snapshot(0).unwrap();
    let later = row(&mut writer, &base, predicate, second);
    let current = writer.snapshot(0).unwrap();
    let future = Arc::downgrade(writer.sealed.last().unwrap());
    assert!(!Read::from(&previous).contains_atom(later));
    drop(current);
    drop(writer);
    assert!(future.upgrade().is_none());
    assert_eq!(previous.atom_count(), 1);
}
