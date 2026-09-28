//! Canonical ownership and named-capacity admission, independent of physical execution.

use zetesis_core::catalog::{Catalog, Limits, ReadError, Vocabulary, VocabularyBuilder};
use zetesis_core::{Atom, Predicate, TemplateCatalogFailure, Value, ValueNodeRef};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::native_aggregate::{
    self as native, Bound, ErrorKind, Function, Group, GroupData, Guard, Resource, Tuple,
};
use zetesis_ferraris::{AdmissionLimits, AggregateComparison, Node, Theory};

fn theory() -> Theory {
    Theory::new(
        0,
        vec![Node::False, Node::Implies(0, 0)],
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}
fn vocabulary(value: &Value) -> (Vocabulary, zetesis_core::catalog::TermKey) {
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let key = builder
        .import_term_with(value.into(), Limits::default(), || Ok::<_, ()>(()))
        .unwrap();
    (builder.finish_with(0, || Ok::<_, ()>(())).unwrap(), key)
}
fn string(value: zetesis_core::catalog::TermRef<'_>) -> &str {
    match value.descriptor() {
        ValueNodeRef::String(text) => text,
        _ => panic!("string fixture"),
    }
}

#[test]
fn owned_keys_and_term_guards_share_canonical_text() {
    let group = Group::new(
        &theory(),
        Function::Count,
        vec![
            Tuple {
                key: vec![Value::String("shared".into()), Value::Number(1)],
                condition: 1,
            },
            Tuple {
                key: vec![Value::String("shared".into()), Value::Number(2)],
                condition: 1,
            },
        ],
        vec![Guard {
            comparison: AggregateComparison::Le,
            bound: Bound::Term(Value::String("shared".into())),
        }],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let first = string(group.tuples().at(0).unwrap().key.first().unwrap());
    let second = string(group.tuples().at(1).unwrap().key.first().unwrap());
    let Bound::Term(bound) = group.guards().next().unwrap().bound else {
        panic!("term guard")
    };
    assert!(std::ptr::eq(first.as_ptr(), second.as_ptr()));
    assert!(std::ptr::eq(first.as_ptr(), string(bound).as_ptr()));
}

#[test]
fn borrowed_admission_retains_the_supplied_payload_authority() {
    let text = Value::String("x".repeat(32_768));
    let (owner, key) = vocabulary(&text);
    let term = owner.read().term(&key).unwrap();
    let data = GroupData::new(
        &theory(),
        Function::Min,
        owner.read(),
        vec![Tuple {
            key: vec![term],
            condition: 1,
        }],
        vec![],
        native::AdmissionLimits {
            max_storage_bytes: 4_096,
            ..native::AdmissionLimits::default()
        },
        &Cancellation::default(),
    )
    .unwrap();
    let view = data.bind_with(owner.read(), || Ok::<_, ()>(())).unwrap();
    let retained = view.tuples().at(0).unwrap().key.first().unwrap();
    assert!(std::ptr::eq(
        string(term).as_ptr(),
        string(retained).as_ptr()
    ));
    assert!(data.statistics().storage_bytes < u64::try_from(owner.storage_bytes()).unwrap());
}

#[test]
fn borrowed_admission_rejects_foreign_equal_values() {
    let (left, key) = vocabulary(&Value::Number(1));
    let (right, _) = vocabulary(&Value::Number(1));
    let error = GroupData::new(
        &theory(),
        Function::Count,
        right.read(),
        vec![Tuple {
            key: vec![left.read().term(&key).unwrap()],
            condition: 1,
        }],
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        ErrorKind::Canonical(ReadError::ForeignCatalog)
    );
}

#[test]
fn empty_metadata_keeps_its_vocabulary_scope() {
    let (left, _) = vocabulary(&Value::Number(1));
    let (right, _) = vocabulary(&Value::Number(1));
    let data = GroupData::new(
        &theory(),
        Function::Count,
        left.read(),
        vec![],
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(matches!(
        data.bind_with(right.read(), || Ok::<_, ()>(())),
        Err(TemplateCatalogFailure::Read(ReadError::ForeignCatalog))
    ));
}

#[test]
fn a_reader_before_the_selected_term_is_refused() {
    let mut authority = Catalog::new(1 << 20);
    let make =
        |number| Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(number)]).unwrap();
    let old = authority
        .edit(Limits::default(), |builder| {
            let atom = builder.intern_atom(&make(1))?;
            builder.publish(&[atom])
        })
        .unwrap();
    let current = authority
        .edit(Limits::default(), |builder| {
            let atom = builder.intern_atom(&make(2))?;
            builder.publish(&[atom])
        })
        .unwrap();
    let value = current.atoms().at(0).unwrap().values().at(0).unwrap();
    let data = GroupData::new(
        &theory(),
        Function::Count,
        current.read(),
        vec![Tuple {
            key: vec![value],
            condition: 1,
        }],
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(matches!(
        data.bind_with(old.read(), || Ok::<_, ()>(())),
        Err(TemplateCatalogFailure::Read(ReadError::OutsidePrefix))
    ));
}

#[test]
fn owned_publication_enforces_its_combined_named_peak() {
    let subject = theory();
    let make = |limits| {
        Group::new(
            &subject,
            Function::Count,
            vec![],
            vec![],
            limits,
            &Cancellation::default(),
        )
    };
    let full = make(native::AdmissionLimits::default())
        .unwrap()
        .statistics();
    assert!(full.storage_peak_bytes > full.storage_bytes);
    let exact = native::AdmissionLimits {
        max_storage_bytes: full.storage_peak_bytes,
        ..native::AdmissionLimits::default()
    };
    assert!(make(exact).is_ok());
    let error = make(native::AdmissionLimits {
        max_storage_bytes: full.storage_peak_bytes - 1,
        ..exact
    })
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Limit(Resource::StorageBytes));
}

#[test]
fn every_borrowed_admission_work_cutoff_is_incomplete() {
    let (owner, key) = vocabulary(&Value::String("term".into()));
    let term = owner.read().term(&key).unwrap();
    let make = |max_work| {
        GroupData::new(
            &theory(),
            Function::Max,
            owner.read(),
            vec![Tuple {
                key: vec![term],
                condition: 1,
            }],
            vec![Guard {
                comparison: AggregateComparison::Eq,
                bound: Bound::Term(term),
            }],
            native::AdmissionLimits {
                max_work,
                ..native::AdmissionLimits::default()
            },
            &Cancellation::default(),
        )
    };
    let full = make(u64::MAX).unwrap().statistics().work;
    for allowance in 0..full {
        let error = make(allowance).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Limit(Resource::Work));
        assert!(error.statistics().work <= allowance);
    }
    assert!(make(full).is_ok());
}

#[test]
fn cancelled_empty_borrowed_admission_stays_incomplete() {
    let (owner, _) = vocabulary(&Value::Number(0));
    let control = Cancellation::default();
    control.cancel();
    let error = GroupData::new(
        &theory(),
        Function::Count,
        owner.read(),
        vec![],
        vec![],
        native::AdmissionLimits::default(),
        &control,
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Stopped(Stop::Cancelled));
}

#[test]
fn equal_text_in_different_value_classes_is_not_a_duplicate_key() {
    let group = Group::new(
        &theory(),
        Function::Count,
        vec![
            Tuple {
                key: vec![Value::String("same".into())],
                condition: 1,
            },
            Tuple {
                key: vec![Value::Symbol("same".into())],
                condition: 1,
            },
        ],
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let value = group
        .reduce(
            &[true, true],
            None,
            native::ReductionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .original()
        .value();
    assert!(matches!(value, native::Value::Integer(2)));
}

#[test]
fn equal_groups_keep_distinct_eligibility_identity() {
    let (owner, key) = vocabulary(&Value::Number(4));
    let make = || {
        GroupData::new(
            &theory(),
            Function::Count,
            owner.read(),
            vec![Tuple {
                key: vec![owner.read().term(&key).unwrap()],
                condition: 1,
            }],
            vec![],
            native::AdmissionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
    };
    let left = make();
    let right = make();
    let left_view = left.bind_with(owner.read(), || Ok::<_, ()>(())).unwrap();
    let right_view = right.bind_with(owner.read(), || Ok::<_, ()>(())).unwrap();
    assert!(!left_view.same_group(right_view));
    assert!(left_view.same_group(left.bind_with(owner.read(), || Ok::<_, ()>(())).unwrap()));
}
