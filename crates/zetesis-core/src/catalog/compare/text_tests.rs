use std::{cmp::Ordering, convert::Infallible};

use crate::catalog::{Limits, TermRef, storage::Store};
use crate::{Sign, Value, ValueLimits, ValueNode, ValueNodeRef};

use super::Order;

fn compound(name: &str, sign: Sign, numbers: &[i32]) -> Value {
    let mut nodes = vec![ValueNode::Function {
        name: name.into(),
        sign,
        arity: numbers.len(),
    }];
    nodes.extend(numbers.iter().copied().map(ValueNode::Number));
    Value::from_nodes(nodes, ValueLimits::default()).unwrap()
}

fn with_terms<T>(
    left: &Value,
    right: &Value,
    same_owner: bool,
    inspect: impl for<'a> FnOnce(TermRef<'a>, TermRef<'a>) -> T,
) -> T {
    let mut first = Store::new(1_048_576);
    let left_id = first.import_value(left, Limits::default()).unwrap();
    if same_owner {
        let right_id = first.import_value(right, Limits::default()).unwrap();
        let snapshot = first.snapshot(0).unwrap();
        inspect(
            TermRef::new(&snapshot, left_id).unwrap(),
            TermRef::new(&snapshot, right_id).unwrap(),
        )
    } else {
        let mut second = Store::new(1_048_576);
        let right_id = second.import_value(right, Limits::default()).unwrap();
        let first = first.snapshot(0).unwrap();
        let second = second.snapshot(0).unwrap();
        inspect(
            TermRef::new(&first, left_id).unwrap(),
            TermRef::new(&second, right_id).unwrap(),
        )
    }
}

fn name(term: TermRef<'_>) -> &str {
    let ValueNodeRef::Function { name, .. } = term.descriptor() else {
        panic!("compound fixture");
    };
    name
}

fn compare<E>(
    left: TermRef<'_>,
    right: TermRef<'_>,
    asp: bool,
    before: impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    if asp {
        left.compare_terms_with(right, before)
    } else {
        left.compare_ref_with(right, before)
    }
}

fn measured(left: TermRef<'_>, right: TermRef<'_>, asp: bool) -> (Ordering, usize) {
    let mut calls = 0;
    let order = compare(left, right, asp, || {
        calls += 1;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    (order, calls)
}

#[test]
fn canonical_shared_names_need_no_byte_visits() {
    let text = "pressurized_by";
    let left = compound(text, Sign::Positive, &[1]);
    let right = compound(text, Sign::Positive, &[2]);
    for asp in [false, true] {
        let shared = with_terms(&left, &right, true, |left, right| {
            assert!(std::ptr::eq(name(left), name(right)));
            assert!(!left.same_identity(right));
            measured(left, right, asp)
        });
        let separate = with_terms(&left, &right, false, |left, right| {
            assert!(!std::ptr::eq(name(left), name(right)));
            measured(left, right, asp)
        });
        assert_eq!(shared.0, Ordering::Less);
        assert_eq!(separate.0, shared.0);
        // Both descriptor permits remain. Only the shared name's byte pairs
        // and terminating length comparison disappear.
        assert_eq!(separate.1 - shared.1, text.len() + 1);
    }
}

#[test]
fn separately_stored_equal_names_keep_byte_checks() {
    let value = compound("separate_equal_name", Sign::Positive, &[3]);
    with_terms(&value, &value, false, |left, right| {
        assert!(!std::ptr::eq(name(left), name(right)));
        for asp in [false, true] {
            let (order, calls) = measured(left, right, asp);
            assert_eq!(order, Ordering::Equal);
            assert!(calls > name(left).len() + 1);
            assert_eq!(compare(left, right, asp, || Err("stop")), Err("stop"));
        }
    });
}

#[test]
fn shared_text_preserves_complete_descriptor_order() {
    let text = "shared_name";
    let values = [
        Value::String(text.into()),
        Value::Symbol(text.into()),
        compound(text, Sign::Negative, &[]),
        compound(text, Sign::Positive, &[1]),
        compound(text, Sign::Negative, &[1]),
        compound(text, Sign::Positive, &[1, 2]),
        Value::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
            ValueLimits::default(),
        )
        .unwrap(),
    ];
    for left in &values {
        for right in &values {
            with_terms(left, right, true, |stored_left, stored_right| {
                assert_eq!(
                    measured(stored_left, stored_right, false).0,
                    left.cmp(right)
                );
                assert_eq!(
                    measured(stored_left, stored_right, true).0,
                    left.compare_terms(right)
                );
            });
        }
    }
}

#[test]
fn shared_slice_prefixes_keep_their_distinct_lengths() {
    let text = String::from("shared_suffix");
    let short = &text[..6];
    let long = text.as_str();
    assert_eq!(short.as_ptr(), long.as_ptr());
    assert!(!std::ptr::eq(short, long));
    for order in [
        Order::Storage { metered: true },
        Order::Asp { metered: true },
    ] {
        let mut calls = 0;
        assert_eq!(
            order.compare(
                ValueNodeRef::Symbol(short),
                ValueNodeRef::Symbol(long),
                true,
                &mut || {
                    calls += 1;
                    Ok::<_, Infallible>(())
                },
            ),
            Ok(Ordering::Less)
        );
        assert_eq!(calls, 1 + short.len() + 1);
    }
}

#[test]
fn shared_text_comparison_preserves_each_stop() {
    let left = compound("shared_constructor", Sign::Positive, &[1, 2]);
    let right = compound("shared_constructor", Sign::Positive, &[1, 3]);
    for same_owner in [false, true] {
        with_terms(&left, &right, same_owner, |left, right| {
            for asp in [false, true] {
                let (expected, total) = measured(left, right, asp);
                for cutoff in 0..=total {
                    let mut accepted = 0;
                    let result = compare(left, right, asp, || {
                        if accepted == cutoff {
                            Err(cutoff)
                        } else {
                            accepted += 1;
                            Ok(())
                        }
                    });
                    if cutoff == total {
                        assert_eq!(result, Ok(expected));
                    } else {
                        assert_eq!(result, Err(cutoff));
                    }
                    assert_eq!(accepted, cutoff);
                }
            }
        });
    }
}

#[test]
fn ingress_asp_comparison_keeps_shared_text_visits() {
    let names = ["f", "long_shared_constructor"];
    let calls: Vec<_> = names
        .iter()
        .map(|name| {
            let value = compound(name, Sign::Positive, &[1]);
            // Both reads borrow the very same ingress name, but remain on
            // the old ingress schedule rather than the canonical shortcut.
            let (order, calls) = measured((&value).into(), (&value).into(), true);
            assert_eq!(order, Ordering::Equal);
            calls
        })
        .collect();
    assert_eq!(calls[1] - calls[0], names[1].len() - names[0].len());
}
