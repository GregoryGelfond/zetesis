use std::{cmp::Ordering, convert::Infallible};

use crate::catalog::{Limits, PredicateRef, TermRef, storage::Store};
use crate::{Predicate, Sign, Value, ValueLimits, ValueNode};

fn compound() -> Value {
    let mut nodes = vec![
        ValueNode::Function {
            name: "nested".into(),
            sign: Sign::Positive,
            arity: 1,
        };
        64
    ];
    nodes.push(ValueNode::String("long shared payload".repeat(128)));
    Value::from_nodes(nodes, ValueLimits::default()).unwrap()
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

#[test]
fn equal_canonical_terms_need_one_permit_across_prefixes() {
    for value in [Value::String("shared".repeat(1024)), compound()] {
        let mut store = Store::new(16_000_000);
        let id = store.import_value(&value, Limits::default()).unwrap();
        let earlier = store.snapshot(0).unwrap();
        store
            .import_value(&Value::Number(42), Limits::default())
            .unwrap();
        let later = store.snapshot(0).unwrap();
        let left = TermRef::new(&earlier, id).unwrap();
        let right = TermRef::new(&later, id).unwrap();
        for asp in [false, true] {
            let mut calls = 0;
            assert_eq!(
                compare(left, right, asp, || {
                    calls += 1;
                    if calls == 1 {
                        Ok(())
                    } else {
                        Err("payload was visited")
                    }
                }),
                Ok(Ordering::Equal)
            );
            assert_eq!(calls, 1);
        }
    }
}

#[test]
fn canonical_equality_refuses_before_its_identity_probe() {
    let mut store = Store::new(16_000_000);
    let id = store.import_value(&compound(), Limits::default()).unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let term = TermRef::new(&snapshot, id).unwrap();
    for asp in [false, true] {
        let mut calls = 0;
        assert_eq!(
            compare(term, term, asp, || {
                calls += 1;
                Err("stop")
            }),
            Err("stop")
        );
        assert_eq!(calls, 1);
    }
}

#[test]
fn distinct_ids_preserve_both_typed_orders() {
    let mut store = Store::new(16_000_000);
    let symbol = store
        .import_value(&Value::Symbol("a".into()), Limits::default())
        .unwrap();
    let string = store
        .import_value(&Value::String("z".into()), Limits::default())
        .unwrap();
    assert!(symbol < string);
    let snapshot = store.snapshot(0).unwrap();
    let symbol = TermRef::new(&snapshot, symbol).unwrap();
    let string = TermRef::new(&snapshot, string).unwrap();
    assert_eq!(
        string.compare_ref_with(symbol, || Ok::<_, Infallible>(())),
        Ok(Ordering::Less)
    );
    assert_eq!(
        string.compare_terms_with(symbol, || Ok::<_, Infallible>(())),
        Ok(Ordering::Greater)
    );
}

#[test]
fn foreign_equal_ids_do_not_establish_term_equality() {
    let mut left = Store::new(16_000_000);
    let left_id = left
        .import_value(&Value::String("a".into()), Limits::default())
        .unwrap();
    let left = left.snapshot(0).unwrap();
    let mut right = Store::new(16_000_000);
    let right_id = right
        .import_value(&Value::String("z".into()), Limits::default())
        .unwrap();
    assert_eq!(left_id, right_id);
    let right = right.snapshot(0).unwrap();
    let left = TermRef::new(&left, left_id).unwrap();
    let right = TermRef::new(&right, right_id).unwrap();
    for asp in [false, true] {
        assert_eq!(
            compare(left, right, asp, || Ok::<_, Infallible>(())),
            Ok(Ordering::Less)
        );
    }
}

#[test]
fn ingress_storage_comparison_keeps_its_legacy_trace() {
    for value in [Value::Number(7), Value::String("shared".into()), compound()] {
        let mut expected_calls = 0;
        let expected = value
            .compare_identity_with(&value, || {
                expected_calls += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap();
        let mut actual_calls = 0;
        let actual = TermRef::from(&value)
            .compare_ref_with(TermRef::from(&value), || {
                actual_calls += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual_calls, expected_calls);
    }
}

#[test]
fn canonical_predicate_equality_needs_one_permit() {
    let original = Predicate::new("long_predicate_name", 3).unwrap();
    let mut store = Store::new(16_000_000);
    let id = store
        .import_predicate_with((&original).into(), || Ok::<_, Infallible>(()))
        .unwrap();
    let earlier = store.snapshot(0).unwrap();
    store
        .import_value(&Value::Number(1), Limits::default())
        .unwrap();
    let later = store.snapshot(0).unwrap();
    let left = PredicateRef::new(&earlier, id).unwrap();
    let right = PredicateRef::new(&later, id).unwrap();
    let mut calls = 0;
    assert_eq!(
        left.compare_ref_with(right, || {
            calls += 1;
            if calls == 1 {
                Ok(())
            } else {
                Err("name was visited")
            }
        }),
        Ok(Ordering::Equal)
    );
    assert_eq!(calls, 1);
}

#[test]
fn canonical_predicate_equality_preserves_caller_stop() {
    let original = Predicate::new("p", 1).unwrap();
    let mut store = Store::new(16_000_000);
    let id = store
        .import_predicate_with((&original).into(), || Ok::<_, Infallible>(()))
        .unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let predicate = PredicateRef::new(&snapshot, id).unwrap();
    assert_eq!(
        predicate.compare_ref_with(predicate, || Err("stop")),
        Err("stop")
    );
}

#[test]
fn a_signed_view_keeps_its_changed_identity() {
    let original = Predicate::new("p", 1).unwrap();
    let negative = Predicate::with_sign("p", 1, Sign::Negative).unwrap();
    let mut store = Store::new(16_000_000);
    let id = store
        .import_predicate_with((&original).into(), || Ok::<_, Infallible>(()))
        .unwrap();
    let negative_id = store
        .import_predicate_with((&negative).into(), || Ok::<_, Infallible>(()))
        .unwrap();
    let snapshot = store.snapshot(0).unwrap();
    let positive = PredicateRef::new(&snapshot, id).unwrap();
    let negative = PredicateRef::new(&snapshot, negative_id).unwrap();
    let signed = positive.with_sign(Sign::Negative);
    assert_eq!(
        signed.compare_ref_with(positive, || Ok::<_, Infallible>(())),
        Ok(Ordering::Greater)
    );
    assert_eq!(
        signed.compare_ref_with(negative, || Ok::<_, Infallible>(())),
        Ok(Ordering::Equal)
    );
}

#[test]
fn foreign_predicate_ids_keep_their_own_signatures() {
    let mut left = Store::new(16_000_000);
    let original = Predicate::new("a", 1).unwrap();
    let left_id = left
        .import_predicate_with((&original).into(), || Ok::<_, Infallible>(()))
        .unwrap();
    let left = left.snapshot(0).unwrap();
    let mut right = Store::new(16_000_000);
    let original = Predicate::new("z", 1).unwrap();
    let right_id = right
        .import_predicate_with((&original).into(), || Ok::<_, Infallible>(()))
        .unwrap();
    assert_eq!(left_id, right_id);
    let right = right.snapshot(0).unwrap();
    assert_eq!(
        PredicateRef::new(&left, left_id).unwrap().compare_ref_with(
            PredicateRef::new(&right, right_id).unwrap(),
            || Ok::<_, Infallible>(()),
        ),
        Ok(Ordering::Less)
    );
}

#[test]
fn repeated_atom_occurrences_share_one_checked_identity() {
    let original = crate::Atom::new(Predicate::new("p", 1).unwrap(), vec![compound()]).unwrap();
    let catalog = crate::AtomCatalog::new(vec![original.clone(), original]).unwrap();
    let left = catalog.atoms().at(0).unwrap();
    let right = catalog.atoms().at(1).unwrap();
    let mut calls = 0;
    assert_eq!(
        left.compare_ref_with(right, || {
            calls += 1;
            if calls == 1 {
                Ok(())
            } else {
                Err("row was visited")
            }
        }),
        Ok(Ordering::Equal)
    );
    assert_eq!(calls, 1);
}

#[test]
fn canonical_atom_equality_preserves_caller_stop() {
    let atom = crate::Atom::new(Predicate::new("p", 1).unwrap(), vec![compound()]).unwrap();
    let catalog = crate::AtomCatalog::new(vec![atom]).unwrap();
    let atom = catalog.atoms().at(0).unwrap();
    assert_eq!(atom.compare_ref_with(atom, || Err("stop")), Err("stop"));
}

#[test]
fn foreign_atom_ids_do_not_establish_equality() {
    let atom = |value| {
        crate::Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(value)]).unwrap()
    };
    let left = crate::AtomCatalog::new(vec![atom(1)]).unwrap();
    let right = crate::AtomCatalog::new(vec![atom(2)]).unwrap();
    assert_eq!(
        left.atoms()
            .at(0)
            .unwrap()
            .compare_ref_with(right.atoms().at(0).unwrap(), || Ok::<_, Infallible>(()),),
        Ok(Ordering::Less)
    );
}

#[test]
fn ingress_atom_comparison_keeps_its_legacy_trace() {
    let atom = crate::Atom::new(Predicate::new("p", 1).unwrap(), vec![compound()]).unwrap();
    let mut expected_calls = 0;
    let expected = crate::identity::atom(&atom, &atom, &mut || {
        expected_calls += 1;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    let mut actual_calls = 0;
    let actual = crate::catalog::AtomRef::from(&atom)
        .compare_ref_with((&atom).into(), || {
            actual_calls += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual_calls, expected_calls);
}
