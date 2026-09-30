use super::*;
use crate::test_support::{PERMIT_WITH_UNIT_ERROR as PERMIT, unlimited};
use crate::{Predicate, Sign, TemplateComponents, TemplateTerm, Value, ValueLimits, ValueNode};

#[test]
fn frozen_owner_preserves_component_identity() {
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let key = builder
        .import_term_with((&Value::Symbol("a".into())).into(), unlimited(), PERMIT)
        .unwrap();
    let mut components = TemplateComponents::new(builder.read(), 1 << 20).unwrap();
    components
        .append_terms_with(
            builder.read(),
            [TemplateTerm::Constant(builder.read().term(&key).unwrap())],
            1 << 20,
            PERMIT,
        )
        .unwrap();
    let owner = builder
        .finish_with(components.storage_bytes(), PERMIT)
        .unwrap();
    let copy = owner.clone();
    let term = components
        .bind_with(copy.read(), PERMIT)
        .unwrap()
        .term(0)
        .unwrap();
    assert_eq!(
        term,
        TemplateTerm::Constant(owner.read().term(&key).unwrap())
    );
}

#[test]
fn declarations_do_not_require_ground_atoms() {
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let predicate = builder
        .declare_predicate_with((&Predicate::new("p", 0).unwrap()).into(), PERMIT)
        .unwrap();
    let shape = builder
        .declare_constructor_with(
            ValueNodeRef::Function {
                name: "f",
                sign: Sign::Positive,
                arity: 0,
            },
            PERMIT,
        )
        .unwrap();
    let owner = builder.finish_with(0, PERMIT).unwrap();
    assert_eq!(owner.read().predicate(&predicate).unwrap().name(), "p");
    assert_eq!(
        owner.read().constructor(&shape).unwrap(),
        ValueNodeRef::Function {
            name: "f",
            sign: Sign::Positive,
            arity: 0
        }
    );
}

#[test]
fn import_keeps_the_callers_stop_value() {
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let before = builder.storage_bytes();
    let error = builder
        .import_term_with((&Value::Number(4)).into(), unlimited(), || Err("stopped"))
        .unwrap_err();
    assert_eq!(error, VocabularyFailure::Stopped("stopped"));
    assert_eq!(builder.storage_bytes(), before);
}

#[test]
fn admitted_deep_values_do_not_acquire_a_default_depth_limit() {
    let depth = 512;
    let mut nodes = vec![ValueNode::Tuple { arity: 1 }; depth - 1];
    nodes.push(ValueNode::Number(7));
    let value = Value::from_nodes(
        nodes,
        ValueLimits {
            max_nodes: depth,
            max_depth: depth,
            max_bytes: 1 << 20,
        },
    )
    .unwrap();
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let key = builder
        .import_term_with((&value).into(), unlimited(), PERMIT)
        .unwrap();
    let owner = builder.finish_with(0, PERMIT).unwrap();
    assert_eq!(owner.read().term(&key).unwrap().depth(), depth);
}

#[test]
fn publication_checks_simultaneous_metadata() {
    let builder = VocabularyBuilder::new(1 << 20).unwrap();
    let error = builder.finish_with(1 << 20, PERMIT).unwrap_err();
    assert!(matches!(
        error,
        VocabularyFailure::Storage(Error::Storage { .. })
    ));
}

fn assigned_number(builder: &mut VocabularyBuilder) -> super::super::TermAssignment {
    let empty = builder.read().assignment();
    let number = builder
        .construct_term_with(
            ValueNodeRef::Number(7),
            empty.as_slice(),
            &[],
            unlimited(),
            PERMIT,
        )
        .unwrap();
    let mut values = builder.read().assignment();
    values.resize_with(1, 1 << 20, PERMIT).unwrap();
    values.set_with(0, &number, PERMIT).unwrap();
    values
}

#[test]
fn constructed_terms_reuse_imported_identity() {
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let values = assigned_number(&mut builder);
    let descriptor = ValueNodeRef::Tuple { arity: 2 };
    let constructed = builder
        .construct_term_with(descriptor, values.as_slice(), &[0, 0], unlimited(), PERMIT)
        .unwrap();
    let owned = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 2 },
            ValueNode::Number(7),
            ValueNode::Number(7),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let imported = builder
        .import_term_with((&owned).into(), unlimited(), PERMIT)
        .unwrap();
    let mut selection = builder.read().term_set();
    assert!(
        selection
            .insert_with(&constructed, 1 << 20, PERMIT)
            .unwrap()
    );
    assert!(!selection.insert_with(&imported, 1 << 20, PERMIT).unwrap());
    assert_eq!(
        builder.read().term(&constructed).unwrap(),
        TermRef::from(&owned)
    );
}

#[test]
fn assigned_construction_refuses_foreign_children() {
    let mut source = VocabularyBuilder::new(1 << 20).unwrap();
    let values = assigned_number(&mut source);
    let mut other = VocabularyBuilder::new(1 << 20).unwrap();
    let before = other.storage_bytes();
    assert!(matches!(
        other.construct_term_with(
            ValueNodeRef::Tuple { arity: 1 },
            values.as_slice(),
            &[0],
            unlimited(),
            PERMIT,
        ),
        Err(AssignmentFailure::Assignment(AssignmentError::Read(
            super::super::ReadError::ForeignCatalog,
        )))
    ));
    assert_eq!(other.storage_bytes(), before);
}

#[test]
fn assigned_construction_counts_repeated_children() {
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let values = assigned_number(&mut builder);
    assert!(matches!(
        builder.construct_term_with(
            ValueNodeRef::Tuple { arity: 2 },
            values.as_slice(),
            &[0, 0],
            Limits {
                max_nodes: 2,
                ..unlimited()
            },
            PERMIT,
        ),
        Err(AssignmentFailure::Assignment(AssignmentError::Storage(
            Error::Value(_)
        )))
    ));
    let key = builder
        .construct_term_with(
            ValueNodeRef::Tuple { arity: 2 },
            values.as_slice(),
            &[0, 0],
            Limits {
                max_nodes: 3,
                ..unlimited()
            },
            PERMIT,
        )
        .unwrap();
    assert_eq!(builder.read().term(&key).unwrap().expanded_nodes(), 3);
}

#[test]
fn each_assigned_construction_stop_preserves_retry() {
    let mut baseline = VocabularyBuilder::new(1 << 20).unwrap();
    let values = assigned_number(&mut baseline);
    let mut steps = 0;
    let expected = baseline
        .construct_term_with(
            ValueNodeRef::Tuple { arity: 2 },
            values.as_slice(),
            &[0, 0],
            unlimited(),
            || {
                steps += 1;
                Ok::<_, usize>(())
            },
        )
        .unwrap();
    for cutoff in 0..steps {
        let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
        let values = assigned_number(&mut builder);
        let mut visited = 0;
        let error = builder
            .construct_term_with(
                ValueNodeRef::Tuple { arity: 2 },
                values.as_slice(),
                &[0, 0],
                unlimited(),
                || {
                    if visited == cutoff {
                        return Err(cutoff);
                    }
                    visited += 1;
                    Ok(())
                },
            )
            .unwrap_err();
        assert!(matches!(error, AssignmentFailure::Stopped(at) if at == cutoff));
        let recovered = builder
            .construct_term_with(
                ValueNodeRef::Tuple { arity: 2 },
                values.as_slice(),
                &[0, 0],
                unlimited(),
                PERMIT,
            )
            .unwrap();
        assert_eq!(
            builder.read().term(&recovered).unwrap(),
            baseline.read().term(&expected).unwrap()
        );
        let repeated = builder
            .construct_term_with(
                ValueNodeRef::Tuple { arity: 2 },
                values.as_slice(),
                &[0, 0],
                unlimited(),
                PERMIT,
            )
            .unwrap();
        let mut selection = builder.read().term_set();
        assert!(selection.insert_with(&recovered, 1 << 20, PERMIT).unwrap());
        assert!(!selection.insert_with(&repeated, 1 << 20, PERMIT).unwrap());
    }
}
