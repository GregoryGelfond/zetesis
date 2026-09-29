use std::{cmp::Ordering, convert::Infallible};

use super::AtomRef;
use crate::catalog::{Limits, storage::Store};
use crate::{
    Atom, AtomCatalog, AtomIndex, AtomIndexError, Predicate, Sign, Value, ValueLimits, ValueNode,
};

const PERMIT: fn() -> Result<(), Infallible> = || Ok(());

fn atom(sign: Sign, value: Value) -> Atom {
    Atom::new(Predicate::with_sign("p", 1, sign).unwrap(), vec![value]).unwrap()
}

fn compound(suffix: &str) -> Value {
    Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 1,
            },
            ValueNode::String(suffix.into()),
        ],
        ValueLimits::default(),
    )
    .unwrap()
}

fn assert_foreign_equality(left: AtomRef<'_>, right: AtomRef<'_>, expected: bool) {
    let mut calls = 0;
    assert_eq!(
        left.equals_ref_with(right, || {
            calls += 1;
            PERMIT()
        }),
        Ok(expected)
    );
    assert!(
        calls > 1,
        "independent atom scopes require semantic comparison"
    );
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

#[test]
fn scoped_atom_equality_admits_one_identity_probe_across_prefixes() {
    let prefix = "shared".repeat(1024);
    let mut store = Store::new(16_000_000);
    let first = store
        .import_atom(
            &atom(Sign::Positive, compound(&format!("{prefix}z"))),
            Limits::default(),
        )
        .unwrap();
    let earlier = store.snapshot(0).unwrap();
    let different = store
        .import_atom(
            &atom(Sign::Positive, compound(&format!("{prefix}a"))),
            Limits::default(),
        )
        .unwrap();
    let negative = store
        .import_atom(
            &atom(Sign::Negative, compound(&format!("{prefix}z"))),
            Limits::default(),
        )
        .unwrap();
    let later = store.snapshot(0).unwrap();
    assert!(AtomRef::new(&earlier, different).is_none());
    let left = AtomRef::new(&earlier, first).unwrap();
    for (id, expected) in [(first, true), (different, false), (negative, false)] {
        let right = AtomRef::new(&later, id).unwrap();
        let mut calls = 0;
        assert_eq!(
            left.equals_ref_with(right, || {
                calls += 1;
                if calls == 1 {
                    Ok(())
                } else {
                    Err("payload was visited")
                }
            }),
            Ok(expected)
        );
        assert_eq!(calls, 1);
        assert_eq!(left == right, expected);
        assert_eq!(left.equals_ref_with(right, || Err("stop")), Err("stop"));
    }
    // Insertion order is deliberately opposite semantic argument order.
    assert!(first < different);
    assert_eq!(
        left.compare_ref_with(AtomRef::new(&later, different).unwrap(), PERMIT),
        Ok(Ordering::Greater)
    );
}

#[test]
fn independent_atom_ids_require_signed_compound_correspondence() {
    let positive = atom(Sign::Positive, compound("a"));
    let mut left = Store::new(1 << 20);
    let original = left.import_atom(&positive, Limits::default()).unwrap();
    let left = left.snapshot(0).unwrap();
    let mut right = Store::new(1 << 20);
    let different = right
        .import_atom(&atom(Sign::Positive, compound("b")), Limits::default())
        .unwrap();
    let equal = right.import_atom(&positive, Limits::default()).unwrap();
    let negative = right
        .import_atom(&atom(Sign::Negative, compound("a")), Limits::default())
        .unwrap();
    assert_eq!(original, different);
    assert_ne!(original, equal);
    let right = right.snapshot(0).unwrap();
    let left = AtomRef::new(&left, original).unwrap();
    for (id, expected) in [(different, false), (equal, true), (negative, false)] {
        assert_foreign_equality(left, AtomRef::new(&right, id).unwrap(), expected);
    }
    assert_foreign_equality(left, (&positive).into(), true);
}

#[test]
fn frozen_vocabulary_does_not_share_tuple_writer_identity() {
    let a = atom(Sign::Positive, compound("a"));
    let b = atom(Sign::Positive, compound("b"));
    let mut base = Store::new(1 << 20);
    base.import_value(&a.values()[0], Limits::default())
        .unwrap();
    base.import_value(&b.values()[0], Limits::default())
        .unwrap();
    base.import_predicate_with(a.predicate().into(), PERMIT)
        .unwrap();
    let base = base.freeze_vocabulary_with(0, PERMIT).unwrap();
    let mut left = Store::with_vocabulary(base.clone(), 1 << 20).unwrap();
    let mut right = Store::with_vocabulary(base, 1 << 20).unwrap();
    let first = left.import_atom(&a, Limits::default()).unwrap();
    let different = right.import_atom(&b, Limits::default()).unwrap();
    let equal = right.import_atom(&a, Limits::default()).unwrap();
    assert_eq!(first, different);
    assert_ne!(first, equal);
    let left = left.snapshot(0).unwrap();
    let right = right.snapshot(0).unwrap();
    let left = AtomRef::new(&left, first).unwrap();
    assert_foreign_equality(left, AtomRef::new(&right, different).unwrap(), false);
    assert_foreign_equality(left, AtomRef::new(&right, equal).unwrap(), true);
}

#[test]
fn ingress_atom_equality_retains_the_legacy_permit_schedule() {
    let left = atom(Sign::Positive, compound("a"));
    for right in [
        left.clone(),
        atom(Sign::Positive, compound("b")),
        atom(Sign::Negative, compound("a")),
    ] {
        let mut legacy = 0;
        let expected = crate::identity::atom(&left, &right, &mut || {
            legacy += 1;
            PERMIT()
        })
        .unwrap()
        .is_eq();
        let mut actual = 0;
        assert_eq!(
            AtomRef::from(&left).equals_ref_with((&right).into(), || {
                actual += 1;
                PERMIT()
            }),
            Ok(expected)
        );
        assert_eq!(actual, legacy);
        for cutoff in 0..legacy {
            let mut accepted = 0;
            assert_eq!(
                AtomRef::from(&left).equals_ref_with((&right).into(), || {
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
fn canonical_index_refuses_duplicate_occurrences_without_losing_dense_positions() {
    let a = atom(Sign::Positive, compound("a"));
    let b = atom(Sign::Positive, compound("b"));
    let catalog = AtomCatalog::new(vec![b, a.clone(), a]).unwrap();
    let mut calls = 0;
    let duplicate = AtomIndex::from_catalog_with(catalog.atoms(), || {
        calls += 1;
        PERMIT()
    });
    assert!(matches!(
        duplicate,
        Err(AtomIndexError::Duplicate {
            first: 1,
            second: 2
        })
    ));
    for cutoff in 0..calls {
        let mut accepted = 0;
        let stopped = AtomIndex::from_catalog_with(catalog.atoms(), || {
            if accepted == cutoff {
                return Err("stop");
            }
            accepted += 1;
            Ok(())
        });
        assert!(matches!(stopped, Err(AtomIndexError::Stopped("stop"))));
        assert_eq!(accepted, cutoff);
    }
    assert_eq!(catalog.atoms().at(1), catalog.atoms().at(2));
    assert_ne!(catalog.atoms().at(0), catalog.atoms().at(1));
}
