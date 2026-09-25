use std::{cmp::Ordering, convert::Infallible};

use crate::catalog::{Limits, TermRef, storage::Store};
use crate::{Sign, Value, ValueLimits, ValueNode};

fn nested(number: i32) -> Value {
    Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "outer".into(),
                sign: Sign::Positive,
                arity: 2,
            },
            ValueNode::Function {
                name: "inner".into(),
                sign: Sign::Negative,
                arity: 1,
            },
            ValueNode::Number(number),
            ValueNode::String("last".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap()
}

#[test]
fn canonical_numeric_order_fits_root_work() {
    // One identity probe, two descriptor reads and the two storage-order
    // comparisons suffice. Neither scalar has descendants to navigate.
    const ROOT_WORK: usize = 5;
    let mut store = Store::new(1_048_576);
    let left = store
        .import_value(&Value::Number(2), Limits::default())
        .unwrap();
    let right = store
        .import_value(&Value::Number(7), Limits::default())
        .unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let left = TermRef::new(&snapshot, left).unwrap();
    let right = TermRef::new(&snapshot, right).unwrap();
    let mut remaining = ROOT_WORK;
    assert_eq!(
        left.compare_ref_with(right, || {
            remaining = remaining.checked_sub(1).ok_or("unexpected navigation")?;
            Ok::<_, &str>(())
        }),
        Ok(Ordering::Less)
    );
}

#[test]
fn root_consumption_preserves_descendant_order() {
    let tuple = |nodes| Value::from_nodes(nodes, ValueLimits::default()).unwrap();
    let values = [
        Value::Infimum,
        Value::Number(3),
        Value::String("a".into()),
        Value::Symbol("a".into()),
        tuple(vec![ValueNode::Tuple { arity: 0 }]),
        tuple(vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(3)]),
        tuple(vec![ValueNode::Function {
            name: "a".into(),
            sign: Sign::Negative,
            arity: 0,
        }]),
        nested(3),
        nested(4),
        Value::Supremum,
    ];
    let mut store = Store::new(1_048_576);
    let ids: Vec<_> = values
        .iter()
        .map(|value| store.import_value(value, Limits::default()).unwrap())
        .collect();
    let snapshot = store.snapshot(0).unwrap();
    for (left_position, left_value) in values.iter().enumerate() {
        for (right_position, right_value) in values.iter().enumerate() {
            let left = TermRef::new(&snapshot, ids[left_position]).unwrap();
            let right = TermRef::new(&snapshot, ids[right_position]).unwrap();
            assert_eq!(
                left.compare_ref_with(right, || Ok::<_, Infallible>(())),
                Ok(left_value.cmp(right_value))
            );
            assert_eq!(
                left.compare_terms_with(right, || Ok::<_, Infallible>(())),
                Ok(left_value.compare_terms(right_value))
            );
        }
    }
}

#[test]
fn descendant_comparison_preserves_each_stop() {
    let mut store = Store::new(1_048_576);
    let left = store.import_value(&nested(3), Limits::default()).unwrap();
    let right = store.import_value(&nested(4), Limits::default()).unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let left = TermRef::new(&snapshot, left).unwrap();
    let right = TermRef::new(&snapshot, right).unwrap();
    let mut visits = 0;
    left.compare_ref_with(right, || {
        visits += 1;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    for cutoff in 0..visits {
        let mut accepted = 0;
        let result = left.compare_ref_with(right, || {
            if accepted == cutoff {
                Err(cutoff)
            } else {
                accepted += 1;
                Ok(())
            }
        });
        assert_eq!(result, Err(cutoff));
        assert_eq!(accepted, cutoff);
    }
}
