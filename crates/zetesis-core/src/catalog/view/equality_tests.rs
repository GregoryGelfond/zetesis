use super::{PredicateRef, TermRef};
use crate::catalog::{CatalogRead, DerivedTerms, Limits, storage::Store};
use crate::test_support::PERMIT;
use crate::{Predicate, Sign, Value, ValueLimits, ValueNode, ValueNodeRef};

fn compound(suffix: &str) -> Value {
    let mut nodes = vec![
        ValueNode::Function {
            name: "nested".into(),
            sign: Sign::Positive,
            arity: 1,
        };
        32
    ];
    nodes.push(ValueNode::String(format!(
        "{}{suffix}",
        "shared".repeat(512)
    )));
    Value::from_nodes(nodes, ValueLimits::default()).unwrap()
}

fn one_permit(calls: &mut usize) -> Result<(), &'static str> {
    *calls += 1;
    if *calls == 1 {
        Ok(())
    } else {
        Err("payload was visited")
    }
}

#[test]
fn scoped_equality_uses_one_permit_across_prefixes() {
    let mut store = Store::new(16_000_000);
    let first = store
        .import_value(&compound("a"), Limits::default())
        .unwrap();
    let earlier = store.snapshot(0).unwrap();
    let second = store
        .import_value(&compound("b"), Limits::default())
        .unwrap();
    let later = store.snapshot(0).unwrap();
    assert_ne!(first, second);
    assert!(TermRef::new(&earlier, second).is_none());
    let left = TermRef::new(&earlier, first).unwrap();
    for (id, expected) in [(first, true), (second, false)] {
        let right = TermRef::new(&later, id).unwrap();
        let mut calls = 0;
        assert_eq!(
            left.equals_ref_with(right, || one_permit(&mut calls)),
            Ok(expected)
        );
        assert_eq!(calls, 1);
        assert_eq!(left == right, expected);
    }
}

#[test]
fn scoped_equality_preserves_the_first_refusal() {
    let mut store = Store::new(16_000_000);
    let first = store
        .import_value(&compound("a"), Limits::default())
        .unwrap();
    let second = store
        .import_value(&compound("b"), Limits::default())
        .unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let left = TermRef::new(&snapshot, first).unwrap();
    for id in [first, second] {
        let mut calls = 0;
        assert_eq!(
            left.equals_ref_with(TermRef::new(&snapshot, id).unwrap(), || {
                calls += 1;
                Err("cancelled")
            }),
            Err("cancelled")
        );
        assert_eq!(calls, 1);
    }
}

#[test]
fn foreign_ids_cannot_determine_equality() {
    let mut first = Store::new(16_000_000);
    let a = first
        .import_value(&Value::String("common-a".into()), Limits::default())
        .unwrap();
    let first = first.snapshot(0).unwrap();
    let mut second = Store::new(16_000_000);
    let b = second
        .import_value(&Value::String("common-b".into()), Limits::default())
        .unwrap();
    let equal = second
        .import_value(&Value::String("common-a".into()), Limits::default())
        .unwrap();
    assert_eq!(a, b);
    assert_ne!(a, equal);
    let second = second.snapshot(0).unwrap();
    let left = TermRef::new(&first, a).unwrap();
    for (id, expected) in [(b, false), (equal, true)] {
        let right = TermRef::new(&second, id).unwrap();
        let mut calls = 0;
        assert_eq!(
            left.equals_ref_with(right, || {
                calls += 1;
                PERMIT()
            }),
            Ok(expected)
        );
        assert!(calls > 1);
        assert_eq!(left == right, expected);
        for cutoff in 0..calls {
            let mut accepted = 0;
            assert_eq!(
                left.equals_ref_with(right, || {
                    if accepted == cutoff {
                        return Err("stop");
                    }
                    accepted += 1;
                    Ok(())
                }),
                Err("stop")
            );
            assert_eq!(accepted, cutoff);
        }
    }
}

#[test]
fn frozen_forks_share_term_and_predicate_equality() {
    let mut store = Store::new(16_000_000);
    let a = store
        .import_value(&compound("a"), Limits::default())
        .unwrap();
    let b = store
        .import_value(&compound("b"), Limits::default())
        .unwrap();
    let predicate = Predicate::new("shared_name".repeat(64), 1).unwrap();
    let p = store
        .import_predicate_with((&predicate).into(), PERMIT)
        .unwrap();
    let negative =
        Predicate::with_sign(predicate.name(), predicate.arity(), Sign::Negative).unwrap();
    let q = store
        .import_predicate_with((&negative).into(), PERMIT)
        .unwrap();
    let base = store.freeze_vocabulary_with(0, PERMIT).unwrap();
    let left = Store::with_vocabulary(base.clone(), 16_000_000).unwrap();
    let right = Store::with_vocabulary(base, 16_000_000).unwrap();
    let left = left.empty_snapshot();
    let right = right.empty_snapshot();
    assert!(!super::Read::from(&left).same_atoms(super::Read::from(&right)));
    for (id, expected) in [(a, true), (b, false)] {
        let mut calls = 0;
        assert_eq!(
            TermRef::new(&left, a)
                .unwrap()
                .equals_ref_with(TermRef::new(&right, id).unwrap(), || one_permit(&mut calls)),
            Ok(expected)
        );
        assert_eq!(calls, 1);
    }
    for (id, expected) in [(p, true), (q, false)] {
        let mut calls = 0;
        let left = PredicateRef::new(&left, p).unwrap();
        let right = PredicateRef::new(&right, id).unwrap();
        assert_eq!(
            left.equals_ref_with(right, || one_permit(&mut calls)),
            Ok(expected)
        );
        assert_eq!(left == right, expected);
        assert_eq!(calls, 1);
        assert_eq!(left.equals_ref_with(right, || Err("stop")), Err("stop"));
    }
}

#[test]
fn foreign_and_signed_predicates_use_signature_equality() {
    let mut first = Store::new(16_000_000);
    let p = Predicate::new("p", 1).unwrap();
    let id = first.import_predicate_with((&p).into(), PERMIT).unwrap();
    let first = first.snapshot(0).unwrap();
    let mut second = Store::new(16_000_000);
    let q = Predicate::new("q", 1).unwrap();
    let other = second.import_predicate_with((&q).into(), PERMIT).unwrap();
    let equal = second.import_predicate_with((&p).into(), PERMIT).unwrap();
    let negative = Predicate::with_sign("p", 1, Sign::Negative).unwrap();
    let signed = second
        .import_predicate_with((&negative).into(), PERMIT)
        .unwrap();
    assert_eq!(id, other);
    let second = second.snapshot(0).unwrap();
    let left = PredicateRef::new(&first, id).unwrap();
    for (id, expected) in [(other, false), (equal, true), (signed, false)] {
        let right = PredicateRef::new(&second, id).unwrap();
        assert_eq!(left.equals_ref_with(right, PERMIT), Ok(expected));
        assert_eq!(left == right, expected);
    }
    let changed = left.with_sign(Sign::Negative);
    assert_eq!(changed.equals_ref_with(left, PERMIT), Ok(false));
    assert_eq!(
        changed.equals_ref_with(PredicateRef::new(&second, signed).unwrap(), PERMIT),
        Ok(true)
    );
}

#[test]
fn derived_aliases_share_equality_within_their_own_scope() {
    let mut store = Store::new(16_000_000);
    let original = store
        .import_value(&Value::Number(7), Limits::default())
        .unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let input = TermRef::new(&snapshot, original).unwrap();
    let mut arena =
        DerivedTerms::new_with(&[CatalogRead((&snapshot).into())], 1 << 20, PERMIT).unwrap();
    let borrowed = arena.borrow_with(input, PERMIT).unwrap();
    let constructed = arena
        .scalar_with(ValueNodeRef::Number(7), Limits::default(), PERMIT)
        .unwrap();
    let different = arena
        .scalar_with(ValueNodeRef::Number(8), Limits::default(), PERMIT)
        .unwrap();
    for (key, expected) in [(&constructed, true), (&different, false)] {
        let mut calls = 0;
        let left = arena.read().term(&borrowed).unwrap();
        let right = arena.read().term(key).unwrap();
        assert_eq!(
            left.equals_ref_with(right, || one_permit(&mut calls)),
            Ok(expected)
        );
        assert_eq!(calls, 1);
        assert_eq!(left == right, expected);
    }
    let mut calls = 0;
    assert_eq!(
        input.equals_ref_with(arena.read().term(&borrowed).unwrap(), || {
            calls += 1;
            PERMIT()
        }),
        Ok(true)
    );
    assert!(calls > 1);
    let mut foreign = DerivedTerms::new_with(&[], 1 << 20, PERMIT).unwrap();
    let key = foreign
        .scalar_with(ValueNodeRef::Number(8), Limits::default(), PERMIT)
        .unwrap();
    assert_eq!(borrowed.id, key.id);
    assert_eq!(
        arena
            .read()
            .term(&borrowed)
            .unwrap()
            .equals_ref_with(foreign.read().term(&key).unwrap(), PERMIT),
        Ok(false)
    );
}

#[test]
fn ingress_equality_keeps_the_legacy_callback_trace() {
    let value = compound("a");
    for other in [value.clone(), compound("b")] {
        let mut legacy = 0;
        let expected = value
            .compare_identity_with(&other, || {
                legacy += 1;
                PERMIT()
            })
            .unwrap()
            .is_eq();
        let mut actual = 0;
        let result = TermRef::from(&value).equals_ref_with((&other).into(), || {
            actual += 1;
            PERMIT()
        });
        assert_eq!(result, Ok(expected));
        assert_eq!(actual, legacy);
    }
    let predicate = Predicate::new("p", 1).unwrap();
    let other = Predicate::new("q", 1).unwrap();
    let mut legacy = 0;
    let expected = crate::identity::predicate(&predicate, &other, &mut || {
        legacy += 1;
        PERMIT()
    })
    .unwrap()
    .is_eq();
    let mut actual = 0;
    let result = PredicateRef::from(&predicate).equals_ref_with((&other).into(), || {
        actual += 1;
        PERMIT()
    });
    assert_eq!(result, Ok(expected));
    assert_eq!(actual, legacy);
}
