use super::*;
use crate::catalog::Limits;
use crate::{Sign, Value, ValueLimits, ValueNode, ValueNodeRef, ValueResource};

fn limits() -> Limits {
    Limits {
        max_nodes: usize::MAX,
        max_depth: usize::MAX,
        max_bytes: usize::MAX,
    }
}

fn value_limits() -> ValueLimits {
    ValueLimits {
        max_nodes: usize::MAX,
        max_depth: usize::MAX,
        max_bytes: usize::MAX,
    }
}

fn store() -> Store {
    Store::new(usize::MAX)
}

fn atom(name: &str, values: Vec<Value>) -> crate::Atom {
    crate::Atom::new(crate::Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

#[test]
fn text_is_shared_across_names_and_typed_terms() {
    let mut store = store();
    store
        .import_atom(
            &atom(
                "shared",
                vec![
                    Value::String("shared".into()),
                    Value::Symbol("shared".into()),
                ],
            ),
            limits(),
        )
        .unwrap();
    assert_eq!(store.counts().texts, 1);
}

#[test]
fn string_and_symbol_keep_distinct_identity() {
    let mut store = store();
    let string = store
        .import_value(&Value::String("x".into()), limits())
        .unwrap();
    let symbol = store
        .import_value(&Value::Symbol("x".into()), limits())
        .unwrap();
    assert_ne!(string, symbol);
}

#[test]
fn positive_nullary_function_normalizes_to_symbol() {
    let mut store = store();
    let symbol = store
        .import_value(&Value::Symbol("x".into()), limits())
        .unwrap();
    let function = store
        .intern_node(
            ValueNodeRef::Function {
                name: "x",
                sign: Sign::Positive,
                arity: 0,
            },
            &[],
            limits(),
        )
        .unwrap();
    assert_eq!(function, symbol);
}

#[test]
fn negative_nullary_function_remains_structural() {
    let mut store = store();
    let id = store
        .intern_node(
            ValueNodeRef::Function {
                name: "x",
                sign: Sign::Negative,
                arity: 0,
            },
            &[],
            limits(),
        )
        .unwrap();
    let snapshot = store.snapshot(0).unwrap();
    assert_eq!(
        Read::from(&snapshot).term(id).unwrap().descriptor(),
        ValueNodeRef::Function {
            name: "x",
            sign: Sign::Negative,
            arity: 0
        }
    );
}

#[test]
fn malformed_constructor_is_refused_before_normalization() {
    let mut store = store();
    assert_eq!(
        store.intern_node(
            ValueNodeRef::Function {
                name: "",
                sign: Sign::Positive,
                arity: 0
            },
            &[],
            limits()
        ),
        Err(Fault::Shape)
    );
}

#[test]
fn repeated_atom_import_preserves_canonical_id() {
    let mut store = store();
    let value = atom("p", vec![Value::Number(7)]);
    let first = store.import_atom(&value, limits()).unwrap();
    let _published = store.snapshot(0).unwrap();
    let second = store.import_atom(&value, limits()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn compound_preserves_ordered_children_and_expanded_ends() {
    let value = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 3 },
            ValueNode::Number(1),
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 1,
            },
            ValueNode::Number(2),
            ValueNode::Number(3),
        ],
        value_limits(),
    )
    .unwrap();
    let mut store = store();
    let id = store.import_value(&value, limits()).unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let term = Read::from(&snapshot).term(id).unwrap();
    let descriptors: Vec<_> = (0..3)
        .map(|i| {
            Read::from(&snapshot)
                .term(term.child(i).unwrap())
                .unwrap()
                .descriptor()
        })
        .collect();
    assert_eq!(
        descriptors,
        vec![
            ValueNodeRef::Number(1),
            ValueNodeRef::Function {
                name: "f",
                sign: Sign::Positive,
                arity: 1
            },
            ValueNodeRef::Number(3)
        ]
    );
    assert_eq!(
        (0..3)
            .map(|i| term.child_end(i).unwrap())
            .collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
}

#[test]
fn shared_children_count_repeatedly_toward_limits() {
    let mut store = store();
    let mut id = store.import_value(&Value::Number(0), limits()).unwrap();
    let bound = Limits {
        max_nodes: 15,
        ..limits()
    };
    let head = ValueNodeRef::Tuple { arity: 2 };
    for _ in 0..3 {
        id = store.intern_node(head, &[id, id], bound).unwrap();
    }
    assert!(matches!(
        store.intern_node(head, &[id, id], bound),
        Err(Fault::Value(ValueError::Limit {
            resource: ValueResource::Nodes,
            observed: 31,
            limit: 15
        }))
    ));
}

#[test]
fn intern_hit_still_checks_requested_node_limit() {
    let value = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(3)],
        value_limits(),
    )
    .unwrap();
    let mut store = store();
    store.import_value(&value, limits()).unwrap();
    assert!(matches!(
        store.import_value(
            &value,
            Limits {
                max_nodes: 1,
                ..limits()
            }
        ),
        Err(Fault::Value(ValueError::Limit {
            resource: ValueResource::Nodes,
            ..
        }))
    ));
}

#[test]
fn old_snapshot_rejects_later_term_id() {
    let mut store = store();
    store.import_value(&Value::Number(1), limits()).unwrap();
    let old = store.snapshot(0).unwrap();
    let later = store.import_value(&Value::Number(2), limits()).unwrap();
    let _new = store.snapshot(0).unwrap();
    assert!(Read::from(&old).term(later).is_none());
}

#[test]
fn old_snapshot_does_not_retain_future_segments() {
    let mut store = store();
    store.import_value(&Value::Number(1), limits()).unwrap();
    let old = store.snapshot(0).unwrap();
    store.import_value(&Value::Number(2), limits()).unwrap();
    let later = store.snapshot(0).unwrap();
    let future = Arc::downgrade(store.vocabulary.growing().unwrap().sealed.last().unwrap());
    drop(later);
    drop(store);
    assert!(future.upgrade().is_none());
    assert_eq!(old.term_count(), 1);
}

#[test]
fn snapshot_named_bytes_do_not_grow_after_append() {
    let mut store = store();
    store.import_value(&Value::Number(1), limits()).unwrap();
    let old = store.snapshot(0).unwrap();
    let before = old.retained_bytes();
    store
        .import_value(&Value::Symbol("later allocation".into()), limits())
        .unwrap();
    let _later = store.snapshot(0).unwrap();
    assert_eq!(old.retained_bytes(), before);
}

#[test]
fn publication_checks_combined_segment_and_directory_bytes() {
    let mut store = store();
    store.import_value(&Value::Number(1), limits()).unwrap();
    // Pre-reserve the writer directory so the tested cutoff concerns the
    // simultaneous new immutable directory and moved segment envelope.
    budget::reserve(
        &mut store.vocabulary.growing().unwrap().sealed,
        1,
        &mut store.budget,
    )
    .unwrap();
    let publication = size_of::<SnapshotData>()
        + size_of::<VocabularyData>()
        + size_of::<VocabularySegment>()
        + 4 * size_of::<Arc<VocabularySegment>>();
    store.budget.limit = usize::try_from(store.budget.used).unwrap() + publication - 1;
    assert!(matches!(store.snapshot(0), Err(Fault::Storage { .. })));
    assert!(store.vocabulary.growing().unwrap().sealed.is_empty());
    assert_eq!(store.counts().terms, 1);
}

#[test]
fn publication_refuses_before_uncovered_writer_growth() {
    let mut store = store();
    store.import_value(&Value::Number(1), limits()).unwrap();
    let before = store.budget.used;
    let extra = 64;
    let pending = extra
        + size_of::<SnapshotData>() as u128
        + size_of::<VocabularyData>() as u128
        + size_of::<VocabularySegment>() as u128;
    // The pending consumer and envelopes fit exactly. No allowance remains
    // for the writer directory, even though a writer-only check would allow it.
    store.budget.limit = usize::try_from(before + pending).unwrap();
    store.check_publication(extra).unwrap();
    assert!(matches!(store.snapshot(extra), Err(Fault::Storage { .. })));
    assert_eq!(store.vocabulary.growing().unwrap().sealed.capacity(), 0);
    assert_eq!(store.budget.used, before);
}

#[test]
fn atom_columns_can_reference_previous_segments() {
    let mut store = store();
    let first = store
        .import_atom(
            &atom("p", vec![Value::Number(1), Value::Number(2)]),
            limits(),
        )
        .unwrap();
    let old = store.snapshot(0).unwrap();
    let next = store
        .import_atom(
            &atom("p", vec![Value::Number(2), Value::Number(1)]),
            limits(),
        )
        .unwrap();
    let new = store.snapshot(0).unwrap();
    let first = Read::from(&old).atom(first).unwrap();
    let next = Read::from(&new).atom(next).unwrap();
    assert_eq!(
        [first.argument(0), first.argument(1)],
        [next.argument(1), next.argument(0)]
    );
}

#[test]
fn nullary_atom_has_one_canonical_row() {
    let mut store = store();
    let first = store.import_atom(&atom("p", vec![]), limits()).unwrap();
    let second = store.import_atom(&atom("p", vec![]), limits()).unwrap();
    let (columns, _) = store.atom_columns(first);
    assert_eq!((first, columns.rows), (second, 1));
}

#[test]
fn collision_chain_requires_exact_candidate_match() {
    let mut index = Index::default();
    let mut budget = Budget::new(usize::MAX, 0);
    for id in 0..3 {
        index.reserve(7, &mut budget).unwrap();
        index.insert(7, id);
    }
    assert_eq!(index.find(7, |id| id == 0), Some(0));
    assert_eq!(index.find(7, |id| id == 1), Some(1));
    assert_eq!(index.find(7, |id| id == 2), Some(2));
    assert_eq!(index.find(7, |_| false), None);
}

#[test]
fn row_reservation_refusal_never_publishes_partial_columns() {
    let mut refusals = 0;
    for allowance in (0..1024).step_by(16) {
        let mut store = store();
        let mut last = None;
        for n in 0..4 {
            last = Some(
                store
                    .import_atom(
                        &atom("p", vec![Value::Number(n), Value::Number(n)]),
                        limits(),
                    )
                    .unwrap(),
            );
        }
        let predicate = store.atom_columns(last.unwrap()).1.predicate;
        let next = store.import_value(&Value::Number(4), limits()).unwrap();
        store.budget.limit = usize::try_from(store.budget.used).unwrap() + allowance;
        if store.intern_atom(predicate, &[next, next]).is_err() {
            refusals += 1;
            assert_eq!(store.counts().atoms, 4);
            let columns = &store.tail.columns[0];
            assert!(
                columns
                    .arguments
                    .iter()
                    .all(|column| column.len() == columns.rows)
            );
        }
    }
    assert!(refusals > 0);
}

#[test]
fn ten_thousand_deep_import_and_drop_are_iterative() {
    let mut nodes = Vec::new();
    for _ in 0..10_000 {
        nodes.push(ValueNode::Tuple { arity: 1 });
    }
    nodes.push(ValueNode::Number(0));
    let value = Value::from_nodes(nodes, value_limits()).unwrap();
    let mut store = store();
    let id = store.import_value(&value, limits()).unwrap();
    let snapshot = store.snapshot(0).unwrap();
    drop(store);
    drop(value);
    assert_eq!(Read::from(&snapshot).term(id).unwrap().depth(), 10_001);
    drop(snapshot);
}

#[test]
fn logical_measures_match_the_ingress_description() {
    let value = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 2,
            },
            ValueNode::String("\"\\\nλ".into()),
            ValueNode::Number(i32::MIN),
        ],
        value_limits(),
    )
    .unwrap();
    let mut store = store();
    let id = store.import_value(&value, limits()).unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let term = Read::from(&snapshot).term(id).unwrap();
    let Value::Structured(source) = &value else {
        panic!("fixture is a compound value");
    };
    assert_eq!(
        (term.canonical_bytes(), term.rendered_bytes()),
        (source.canonical_bytes(), source.to_string().len())
    );
}
