use super::*;
use crate::{Predicate, Sign, Value, ValueLimits, ValueNode};
use std::convert::Infallible;

fn limits() -> Limits {
    Limits {
        max_atoms: 32,
        max_bytes: 1_048_576,
    }
}
const SUCCESS: fn() -> Result<(), Infallible> = || Ok(());

#[test]
fn term_admission_does_not_discover_an_atom() {
    let mut owner = AtomInterner::new();
    let (_, mut append) = owner.split();
    let key = append
        .import_term_with(
            (&Value::Number(4)).into(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    assert_eq!(append.read().term(&key).unwrap(), Value::Number(4));
    assert!(append.is_empty());
}

#[test]
fn constructed_compounds_reuse_children_and_preserve_expanded_measures() {
    let source = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "inner".into(),
                sign: Sign::Positive,
                arity: 1,
            },
            ValueNode::String("shared".repeat(100)),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let mut owner = AtomInterner::new();
    let (_, mut append) = owner.split();
    let child = append
        .import_term_with((&source).into(), TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    let mut values = append.read().assignment();
    values.resize_with(1, usize::MAX, SUCCESS).unwrap();
    values.set_with(0, &child, SUCCESS).unwrap();
    let result = append
        .construct_term_with(
            ValueNodeRef::Tuple { arity: 2 },
            values.as_slice(),
            &[0, 0],
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    let result = append.read().term(&result).unwrap();
    assert_eq!(result.expanded_nodes(), 5);
    assert_eq!(result.depth(), 3);
    let first = result.child(0).unwrap().child(0).unwrap();
    let second = result.child(1).unwrap().child(0).unwrap();
    let (ValueNodeRef::String(left), ValueNodeRef::String(right)) =
        (first.descriptor(), second.descriptor())
    else {
        panic!("shared text fixture");
    };
    assert_eq!(left.as_ptr(), right.as_ptr());
    assert!(append.is_empty());
}

#[test]
fn assigned_atoms_use_the_same_discovery_identity_as_ingress() {
    let mut owner = AtomInterner::new();
    let (_, mut append) = owner.split();
    let predicate = Predicate::new("p", 2).unwrap();
    let declared = append
        .declare_predicate_with(&predicate, limits(), SUCCESS)
        .unwrap();
    let key = append
        .import_term_with(
            (&Value::Number(7)).into(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    let mut values = append.read().assignment();
    values.resize_with(1, usize::MAX, SUCCESS).unwrap();
    values.set_with(0, &key, SUCCESS).unwrap();
    let position = append
        .insert_assigned_with(
            &declared,
            values.as_slice(),
            &[0, 0],
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    let ingress = crate::Atom::new(predicate, vec![Value::Number(7), Value::Number(7)]).unwrap();
    assert_eq!(
        append
            .entry_atom_with(&ingress, limits(), SUCCESS)
            .unwrap()
            .position(),
        Some(position)
    );
    assert_eq!(append.len(), 1);
}

#[test]
fn assigned_constructor_rejects_foreign_frame_even_if_empty() {
    let first = AtomInterner::new();
    let values = first.read().assignment();
    let mut second = AtomInterner::new();
    let (_, mut append) = second.split();
    assert!(matches!(
        append.construct_term_with(
            ValueNodeRef::Number(1),
            values.as_slice(),
            &[],
            TermLimits::default(),
            limits(),
            SUCCESS
        ),
        Err(AssignedFailure::Assignment(AssignmentError::Read(
            crate::catalog::ReadError::ForeignCatalog
        )))
    ));
}

#[test]
fn repeated_children_obey_expanded_node_limit() {
    let mut owner = AtomInterner::new();
    let (_, mut append) = owner.split();
    let key = append
        .import_term_with(
            (&Value::Number(1)).into(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    let mut values = append.read().assignment();
    values.resize_with(1, usize::MAX, SUCCESS).unwrap();
    values.set_with(0, &key, SUCCESS).unwrap();
    assert!(matches!(
        append.construct_term_with(
            ValueNodeRef::Tuple { arity: 2 },
            values.as_slice(),
            &[0, 0],
            TermLimits {
                max_nodes: 2,
                ..TermLimits::default()
            },
            limits(),
            SUCCESS
        ),
        Err(AssignedFailure::Interner(Failure::Catalog(
            super::super::super::Error::Value(crate::ValueError::Limit {
                resource: crate::ValueResource::Nodes,
                observed: 3,
                limit: 2
            })
        )))
    ));
}

fn compound() -> Value {
    Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 3 },
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 1,
            },
            ValueNode::String("reused text".into()),
            ValueNode::Number(7),
            ValueNode::String("reused text".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap()
}
fn imported(value: &Value) -> (AtomInterner, TermKey) {
    let mut owner = AtomInterner::new();
    let key = owner
        .split()
        .1
        .import_term_with(value.into(), TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    (owner, key)
}

#[test]
fn frozen_lookup_matches_content_across_scopes() {
    let value = compound();
    let (owner, expected) = imported(&value);
    let mut foreign = AtomInterner::new();
    let (_, mut append) = foreign.split();
    append
        .import_term_with(
            (&Value::Symbol("different first ID".into())).into(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    let foreign_key = append
        .import_term_with((&value).into(), TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    let foreign_term = append.read().term(&foreign_key).unwrap();
    let bytes = owner.storage_bytes();
    let mut lookup = owner.term_lookup();
    for source in [TermRef::from(&value), foreign_term] {
        let found = lookup
            .find_term_with(source, TermLimits::default(), limits(), SUCCESS)
            .unwrap()
            .unwrap();
        assert_eq!(found.id, expected.id);
        assert!(found.scope.same(&expected.scope));
        assert_eq!(lookup.read().term(&found).unwrap(), value);
    }
    assert_eq!(owner.storage_bytes(), bytes);
    assert!(owner.is_empty());
}

#[test]
fn frozen_constructed_lookup_preserves_argument_order() {
    let (mut owner, _) = imported(&compound());
    let (_, mut append) = owner.split();
    let a = append
        .import_term_with(
            (&Value::Number(1)).into(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    let b = append
        .import_term_with(
            (&Value::Number(2)).into(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    let mut values = append.read().assignment();
    values.resize_with(2, usize::MAX, SUCCESS).unwrap();
    values.set_with(0, &a, SUCCESS).unwrap();
    values.set_with(1, &b, SUCCESS).unwrap();
    let expected = append
        .construct_term_with(
            ValueNodeRef::Tuple { arity: 3 },
            values.as_slice(),
            &[0, 1, 0],
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    let mut lookup = owner.term_lookup();
    let found = lookup
        .find_constructed_with(
            ValueNodeRef::Tuple { arity: 3 },
            values.as_slice(),
            &[0, 1, 0],
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap()
        .unwrap();
    assert_eq!(found.id, expected.id);
    assert!(
        lookup
            .find_constructed_with(
                ValueNodeRef::Tuple { arity: 3 },
                values.as_slice(),
                &[1, 0, 0],
                TermLimits::default(),
                limits(),
                SUCCESS
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn frozen_lookup_never_admits_a_missing_component() {
    let (owner, _) = imported(&compound());
    let bytes = owner.storage_bytes();
    let mut lookup = owner.term_lookup();
    for value in [Value::String("new spelling".into()), Value::Number(-39)] {
        assert!(
            lookup
                .find_term_with((&value).into(), TermLimits::default(), limits(), SUCCESS)
                .unwrap()
                .is_none()
        );
        assert!(
            lookup
                .find_term_with((&value).into(), TermLimits::default(), limits(), SUCCESS)
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(owner.storage_bytes(), bytes);
    assert!(owner.is_empty());
}

#[test]
fn frozen_lookup_refusals_leave_shared_state_unchanged() {
    let value = compound();
    let (owner, _) = imported(&value);
    let bytes = owner.storage_bytes();
    let mut measured = owner.term_lookup();
    let mut calls = 0;
    measured
        .find_term_with((&value).into(), TermLimits::default(), limits(), || {
            calls += 1;
            SUCCESS()
        })
        .unwrap();
    for cut in 0..calls {
        let mut lookup = owner.term_lookup();
        let mut visited = 0;
        let result =
            lookup.find_term_with((&value).into(), TermLimits::default(), limits(), || {
                if visited == cut {
                    return Err(cut);
                }
                visited += 1;
                Ok(())
            });
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == cut));
        assert_eq!(visited, cut);
        assert_eq!(owner.storage_bytes(), bytes);
        assert!(
            lookup
                .find_term_with((&value).into(), TermLimits::default(), limits(), SUCCESS)
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn frozen_lookup_bounds_temporary_ids_in_owner_scope() {
    let value = compound();
    let (owner, _) = imported(&value);
    let mut measured = owner.term_lookup();
    measured
        .find_term_with((&value).into(), TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    let exact = measured.storage_peak();
    assert!(exact > owner.storage_bytes());
    let mut refused = owner.term_lookup();
    assert!(
        matches!(refused.find_term_with((&value).into(), TermLimits::default(),
        Limits { max_bytes: exact - 1, ..limits() }, SUCCESS),
        Err(Failure::Bytes { required, limit }) if required == exact && limit == exact - 1)
    );
    let mut accepted = owner.term_lookup();
    assert!(
        accepted
            .find_term_with(
                (&value).into(),
                TermLimits::default(),
                Limits {
                    max_bytes: exact,
                    ..limits()
                },
                SUCCESS
            )
            .unwrap()
            .is_some()
    );
}

#[test]
fn assigned_atom_refusal_withholds_discovery() {
    fn fixture() -> (
        AtomInterner,
        DeclaredPredicate,
        crate::catalog::TermAssignment,
    ) {
        let (mut owner, key) = imported(&Value::Number(11));
        let (_, mut append) = owner.split();
        let predicate = append
            .declare_predicate_with(&Predicate::new("assigned", 1).unwrap(), limits(), SUCCESS)
            .unwrap();
        let mut values = append.read().assignment();
        values.resize_with(1, usize::MAX, SUCCESS).unwrap();
        values.set_with(0, &key, SUCCESS).unwrap();
        (owner, predicate, values)
    }
    let (mut measured, predicate, values) = fixture();
    let mut calls = 0;
    measured
        .split()
        .1
        .insert_assigned_with(
            &predicate,
            values.as_slice(),
            &[0],
            TermLimits::default(),
            limits(),
            || {
                calls += 1;
                SUCCESS()
            },
        )
        .unwrap();
    for cut in 0..calls {
        let (mut owner, predicate, values) = fixture();
        let mut visited = 0;
        let result = owner.split().1.insert_assigned_with(
            &predicate,
            values.as_slice(),
            &[0],
            TermLimits::default(),
            limits(),
            || {
                if visited == cut {
                    return Err(cut);
                }
                visited += 1;
                Ok(())
            },
        );
        assert!(
            matches!(result, Err(AssignedFailure::Interner(Failure::Stopped(actual))) if actual == cut)
        );
        assert!(owner.is_empty());
        assert_eq!(
            owner
                .split()
                .1
                .insert_assigned_with(
                    &predicate,
                    values.as_slice(),
                    &[0],
                    TermLimits::default(),
                    limits(),
                    SUCCESS
                )
                .unwrap(),
            0
        );
        assert_eq!(owner.len(), 1);
    }
}

mod projected;
