use std::{cell::Cell, convert::Infallible};

use crate::catalog::{AtomRef, Limits, PredicateRef, TermRef};
use crate::{Atom, Predicate, Value, ValueNode, ValueResource};

use super::{AtomId, Budget, Failure, Fault, Index, Snapshot, Store, budget, control::Work};
use crate::test_support::{unlimited as limits, unlimited_values as value_limits};

fn source_atom() -> Atom {
    let nested = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 2 },
            ValueNode::String("a\nλ".into()),
            ValueNode::Number(4),
        ],
        value_limits(),
    )
    .unwrap();
    Atom::new(
        Predicate::new("p", 2).unwrap(),
        vec![nested, Value::String("a\nλ".into())],
    )
    .unwrap()
}

fn writer_with_prefix() -> (Store, Snapshot, AtomId) {
    let mut writer = Store::new(usize::MAX);
    let original = Atom::new(Predicate::new("old", 0).unwrap(), vec![]).unwrap();
    let id = writer.import_atom(&original, limits()).unwrap();
    let snapshot = writer.snapshot(0).unwrap();
    (writer, snapshot, id)
}

#[derive(Debug, PartialEq, Eq)]
struct Stop(usize);

fn permits_before(cutoff: usize) -> impl FnMut() -> Result<(), Stop> {
    let mut count = 0;
    move || {
        if count == cutoff {
            return Err(Stop(cutoff));
        }
        count += 1;
        Ok(())
    }
}

fn assert_complete_rows(store: &Store) {
    for segment in store
        .sealed
        .iter()
        .map(AsRef::as_ref)
        .chain(std::iter::once(&store.tail))
    {
        for block in &segment.columns {
            assert!(
                block
                    .arguments
                    .iter()
                    .all(|column| column.len() == block.rows)
            );
        }
    }
}

fn assert_cutoffs_preserve_authority(source: AtomRef<'_>) {
    let (mut complete, _, _) = writer_with_prefix();
    let mut permits = 0;
    complete
        .import_atom_with(source, limits(), || {
            permits += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert!(permits > 0);
    for cutoff in 0..permits {
        let (mut writer, previous, original) = writer_with_prefix();
        let result = writer.import_atom_with(source, limits(), permits_before(cutoff));
        assert_eq!(
            result,
            Err(Failure::Stopped(Stop(cutoff))),
            "cutoff {cutoff}"
        );
        assert_complete_rows(&writer);
        assert_eq!(previous.atom_count(), 1);
        assert_eq!(
            AtomRef::new(&previous, original)
                .unwrap()
                .predicate()
                .name(),
            "old"
        );
        let id = writer
            .import_atom_with(source, limits(), || Ok::<_, Infallible>(()))
            .unwrap();
        assert_eq!(
            AtomRef::new(&writer, id).unwrap(),
            source,
            "cutoff {cutoff}"
        );
    }
}

#[test]
fn every_flat_import_stop_preserves_authority() {
    let source = source_atom();
    assert_cutoffs_preserve_authority(AtomRef::from(&source));
}

#[test]
fn every_canonical_import_stop_preserves_authority() {
    let mut source = Store::new(usize::MAX);
    let id = source.import_atom(&source_atom(), limits()).unwrap();
    let snapshot = source.snapshot(0).unwrap();
    assert_cutoffs_preserve_authority(AtomRef::new(&snapshot, id).unwrap());
}

#[test]
fn same_owner_prefix_reuses_existing_term() {
    let mut writer = Store::new(usize::MAX);
    let id = writer
        .import_value(&Value::String("long".repeat(1024)), limits())
        .unwrap();
    let prefix = writer.snapshot(0).unwrap();
    writer.import_value(&Value::Number(9), limits()).unwrap();
    let before = writer.current_bytes();
    let imported = writer
        .import_term_with(TermRef::new(&prefix, id).unwrap(), limits(), || {
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert_eq!(imported, id);
    assert_eq!(writer.current_bytes(), before);
    assert_eq!(writer.counts().terms, 2);
}

#[test]
fn same_owner_reuse_obeys_logical_limits() {
    let mut writer = Store::new(usize::MAX);
    let value = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(3)],
        value_limits(),
    )
    .unwrap();
    let id = writer.import_value(&value, limits()).unwrap();
    let prefix = writer.snapshot(0).unwrap();
    let term = TermRef::new(&prefix, id).unwrap();
    for (bound, expected) in [
        (
            Limits {
                max_nodes: 1,
                ..limits()
            },
            ValueResource::Nodes,
        ),
        (
            Limits {
                max_depth: 1,
                ..limits()
            },
            ValueResource::Depth,
        ),
        (
            Limits {
                max_bytes: 1,
                ..limits()
            },
            ValueResource::Bytes,
        ),
    ] {
        let error = writer
            .import_term_with(term, bound, || Ok::<_, Infallible>(()))
            .unwrap_err();
        assert!(
            matches!(error, Failure::Storage(Fault::Value(crate::ValueError::Limit { resource, .. })) if resource == expected)
        );
    }
}

#[test]
fn foreign_terms_coalesce_by_structure() {
    let value = source_atom();
    let mut source = Store::new(usize::MAX);
    let source_id = source.import_atom(&value, limits()).unwrap();
    let mut target = Store::new(usize::MAX);
    target.import_value(&Value::Number(-1), limits()).unwrap();
    let expected = target.import_atom(&value, limits()).unwrap();
    let terms = target.counts().terms;
    let actual = target
        .import_atom_with(AtomRef::new(&source, source_id).unwrap(), limits(), || {
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(target.counts().terms, terms);
}

#[test]
fn constructed_row_resolves_each_argument_owner() {
    let mut left = Store::new(usize::MAX);
    let left_id = left
        .import_value(&Value::String("x".into()), limits())
        .unwrap();
    let mut right = Store::new(usize::MAX);
    let right_id = right
        .import_value(&Value::Symbol("x".into()), limits())
        .unwrap();
    assert_eq!(left_id, right_id);
    let predicate = Predicate::new("pair", 2).unwrap();
    let mut target = Store::new(usize::MAX);
    let id = target
        .import_row_with(
            PredicateRef::from(&predicate),
            [
                TermRef::new(&left, left_id).unwrap(),
                TermRef::new(&right, right_id).unwrap(),
            ],
            limits(),
            || Ok::<_, Infallible>(()),
        )
        .unwrap();
    let expected = Atom::new(
        predicate,
        vec![Value::String("x".into()), Value::Symbol("x".into())],
    )
    .unwrap();
    assert_eq!(AtomRef::new(&target, id).unwrap(), AtomRef::from(&expected));
}

#[test]
fn stopped_row_does_not_advance_its_arguments() {
    let predicate = Predicate::new("p", 1).unwrap();
    let value = Value::Number(1);
    let advances = Cell::new(0);
    let arguments = std::iter::from_fn(|| {
        advances.set(advances.get() + 1);
        Some(TermRef::from(&value))
    });
    let mut writer = Store::new(usize::MAX);
    let result = writer.import_row_with(
        PredicateRef::from(&predicate),
        arguments,
        limits(),
        permits_before(0),
    );
    assert_eq!(result, Err(Failure::Stopped(Stop(0))));
    assert_eq!(advances.get(), 0);
}

#[test]
fn constructed_row_refuses_wrong_argument_count() {
    let predicate = Predicate::new("p", 1).unwrap();
    let value = Value::Number(1);
    for count in [0, 2] {
        let mut writer = Store::new(usize::MAX);
        let arguments = std::iter::repeat_n(TermRef::from(&value), count);
        let result =
            writer.import_row_with(PredicateRef::from(&predicate), arguments, limits(), || {
                Ok::<_, Infallible>(())
            });
        assert_eq!(result, Err(Failure::Storage(Fault::Shape)));
        assert_eq!(writer.counts().atoms, 0);
    }
}

#[test]
fn foreign_deep_import_uses_iterative_ownership() {
    let mut nodes = vec![ValueNode::Tuple { arity: 1 }; 10_000];
    nodes.push(ValueNode::Number(0));
    let value = Value::from_nodes(nodes, value_limits()).unwrap();
    let mut source = Store::new(usize::MAX);
    let id = source.import_value(&value, limits()).unwrap();
    let mut target = Store::new(usize::MAX);
    let imported = target
        .import_term_with(TermRef::new(&source, id).unwrap(), limits(), || {
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert_eq!(
        super::Read::from(&target).term(imported).unwrap().depth(),
        10_001
    );
    drop(source);
    drop(target);
}

#[test]
fn foreign_scratch_limit_refuses_before_allocation() {
    let value = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(0)],
        value_limits(),
    )
    .unwrap();
    let mut source = Store::new(usize::MAX);
    let id = source.import_value(&value, limits()).unwrap();
    let term = TermRef::new(&source, id).unwrap();
    let bound = Limits {
        max_bytes: term.canonical_bytes() + term.rendered_bytes() + 1,
        ..limits()
    };
    let mut target = Store::new(usize::MAX);
    let before = target.current_bytes();
    let result = target.import_term_with(term, bound, || Ok::<_, Infallible>(()));
    assert!(matches!(
        result,
        Err(Failure::Storage(Fault::Value(crate::ValueError::Limit {
            resource: ValueResource::Bytes,
            ..
        })))
    ));
    assert_eq!(target.peak_bytes(), before);
}

#[test]
fn collision_probe_stops_before_next_candidate() {
    let mut index = Index::default();
    let mut budget = Budget::new(usize::MAX, 0);
    for id in 0..3 {
        index.reserve(7, &mut budget).unwrap();
        index.insert(7, id);
    }
    // Head read and first candidate are permitted. Stop before the next-link
    // read, so the remaining colliding candidates must remain unobserved.
    let mut before = permits_before(2);
    let mut inspected = Vec::new();
    let result = index.find_with(7, &mut Work::new(&mut before), |id, _| {
        inspected.push(id);
        Ok(false)
    });
    assert_eq!(result, Err(Failure::Stopped(Stop(2))));
    assert_eq!(inspected, vec![2]);
}

#[test]
fn refused_preflight_does_not_inflate_capacity_peak() {
    let mut values = vec![1u64, 2, 3, 4];
    let initial = budget::capacity(&values);
    let mut budget = Budget::new(
        usize::try_from(initial).unwrap(),
        usize::try_from(initial).unwrap(),
    );
    assert!(matches!(
        budget::reserve(&mut values, 1, &mut budget),
        Err(Fault::Storage { .. })
    ));
    assert_eq!(budget.peak, initial);
}

#[test]
fn successful_growth_records_replacement_overlap() {
    let mut values = vec![1u64, 2, 3, 4];
    let before = budget::capacity(&values);
    let mut budget = Budget::new(usize::MAX, usize::try_from(before).unwrap());
    budget::reserve(&mut values, 1, &mut budget).unwrap();
    assert_eq!(budget.used, budget::capacity(&values));
    assert_eq!(budget.peak, before + budget::capacity(&values));
}

#[test]
fn publication_peak_excludes_unallocated_envelopes() {
    let mut writer = Store::new(usize::MAX);
    writer.import_value(&Value::Number(1), limits()).unwrap();
    writer.restart_peak();
    let before = writer.current_bytes();
    writer.ceiling(usize::try_from(before).unwrap()).unwrap();
    assert!(matches!(writer.snapshot(0), Err(Fault::Storage { .. })));
    assert_eq!(writer.peak_bytes(), before);
}

#[test]
fn publication_peak_includes_live_outer_metadata() {
    let mut writer = Store::new(usize::MAX);
    writer.import_value(&Value::Number(1), limits()).unwrap();
    writer.restart_peak();
    let extra = 512;
    let snapshot = writer.snapshot(extra).unwrap();
    assert_eq!(
        writer.peak_bytes(),
        writer.current_bytes() + extra + snapshot.metadata_bytes()
    );
}
