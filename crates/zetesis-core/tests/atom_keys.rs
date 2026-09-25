//! Borrowed substitution identity is the identity of the materialized atom.
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use proptest::prelude::*;
use zetesis_core::catalog::TermRef;
use zetesis_core::relation::{Catalog, Limits};
use zetesis_core::{
    Atom, AtomPattern, BindingView, Predicate, Sign, Term, Value, ValueLimits, ValueNode,
};

fn pattern(sign: Sign) -> AtomPattern {
    AtomPattern::new(
        Predicate::with_sign("p", 2, sign).unwrap(),
        vec![Term::Variable(1), Term::Variable(1)],
    )
    .unwrap()
}
fn values() -> Vec<Value> {
    let structural = |nodes| Value::from_nodes(nodes, ValueLimits::default()).unwrap();
    vec![
        Value::Infimum,
        Value::Supremum,
        Value::Number(-1),
        Value::String("f(1)".into()),
        Value::Symbol("f(1)".into()),
        structural(vec![
            ValueNode::Function {
                name: "f".into(),
                arity: 1,
                sign: Sign::Negative,
            },
            ValueNode::Number(1),
        ]),
        structural(vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)]),
    ]
}
#[derive(Default)]
struct Trace(Vec<u8>);
impl Hasher for Trace {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}
fn trace(value: &impl Hash) -> Vec<u8> {
    let mut state = Trace::default();
    value.hash(&mut state);
    state.0
}

#[test]
fn borrowed_keys_preserve_typed_storage_order() {
    let mut atoms = Vec::new();
    for sign in [Sign::Positive, Sign::Negative] {
        let pattern = pattern(sign);
        for value in values() {
            let frame = [None, Some(value)];
            let key = pattern.key(frame.as_slice()).unwrap();
            atoms.push(key.to_atom(ValueLimits::default()).unwrap());
            for atom in &atoms {
                assert_eq!(
                    key.compare(atom),
                    key.to_atom(ValueLimits::default()).unwrap().cmp(atom)
                );
            }
        }
    }
    for atom in &atoms {
        let pattern = AtomPattern::new(
            atom.predicate().clone(),
            atom.values().iter().cloned().map(Term::Constant).collect(),
        )
        .unwrap();
        let key = pattern.key([].as_slice() as &[Value]).unwrap();
        for other in &atoms {
            assert_eq!(key.compare(other), atom.cmp(other));
        }
    }
}

#[test]
fn borrowed_keys_preserve_hash_operations() {
    for sign in [Sign::Positive, Sign::Negative] {
        let pattern = pattern(sign);
        for value in values() {
            let frame = [None, Some(value)];
            let key = pattern.key(frame.as_slice()).unwrap();
            assert_eq!(
                trace(&key),
                trace(&key.to_atom(ValueLimits::default()).unwrap())
            );
        }
    }
}

#[test]
fn membership_borrows_the_stored_atom() {
    let pattern = pattern(Sign::Negative);
    let frame = [None, Some(Value::Number(7))];
    let key = pattern.key(frame.as_slice()).unwrap();
    let atoms = BTreeSet::from([key.to_atom(ValueLimits::default()).unwrap()]);
    assert!(std::ptr::eq(
        key.get(&atoms).unwrap(),
        atoms.first().unwrap()
    ));
    let absent = [None, Some(Value::Number(8))];
    assert!(
        pattern
            .key(absent.as_slice())
            .unwrap()
            .get(&atoms)
            .is_none()
    );
}

#[test]
fn absent_referenced_slots_refuse_keys() {
    let pattern = pattern(Sign::Positive);
    let short = [Value::Number(0)];
    let absent: [Option<Value>; 2] = [Some(Value::Number(0)), None];
    let borrowed: [Option<&Value>; 2] = [Some(&short[0]), None];
    assert_eq!(pattern.key(short.as_slice()).unwrap_err().variable, 1);
    assert_eq!(pattern.key(absent.as_slice()).unwrap_err().variable, 1);
    assert_eq!(pattern.key(borrowed.as_slice()).unwrap_err().variable, 1);
}

#[test]
fn binding_views_preserve_absence() {
    let values = [None, Some(Value::Number(7))];
    let view = BindingView::from(values.as_slice());
    assert_eq!(view.get(0), None);
    assert_eq!(view.get(1), Some(TermRef::from(&Value::Number(7))));
    assert_eq!(view.get(2), None);
}

#[test]
fn catalog_key_receipts_match_owned_lookup() {
    use zetesis_core::atom_interner::{AtomInterner, Limits as AtomLimits};
    let pattern = pattern(Sign::Positive);
    let mut authority = AtomInterner::default();
    let atom_limits = AtomLimits::for_atoms(64, 1024 * 1024);
    let declared = authority
        .declare_predicate_with(pattern.predicate(), atom_limits, || Ok::<_, ()>(()))
        .unwrap();
    let mut catalog = Catalog::new(authority.read(), declared, Limits::default()).unwrap();
    for value in values() {
        let frame = [None, Some(value)];
        let key = pattern.key(frame.as_slice()).unwrap();
        let canonical = authority
            .entry_key_with(key, atom_limits, || Ok::<_, ()>(()))
            .unwrap()
            .insert_ref_with(atom_limits, || Ok::<_, ()>(()))
            .unwrap();
        catalog.insert(canonical, Limits::default()).unwrap();
        // Owned substitution is the independent public boundary being compared.
        let owned = key.to_atom(ValueLimits::default()).unwrap();
        let expected = catalog
            .lookup(authority.read(), (&owned).into(), Limits::default())
            .unwrap();
        assert_eq!(
            catalog
                .lookup_key(authority.read(), &key, Limits::default())
                .unwrap(),
            expected
        );
        let exact = Limits {
            max_work: u64::try_from(expected.storage.construction_work).unwrap(),
            ..Limits::default()
        };
        assert_eq!(
            catalog.lookup_key(authority.read(), &key, exact).unwrap(),
            expected
        );
        let short = Limits {
            max_work: exact.max_work - 1,
            ..exact
        };
        assert_eq!(
            catalog
                .lookup_key(authority.read(), &key, short)
                .unwrap_err(),
            catalog
                .lookup(authority.read(), (&owned).into(), short)
                .unwrap_err()
        );
    }
}

proptest! {
    #[test]
    fn mixed_bindings_preserve_materialization(first in any::<i32>(), second in any::<i32>()) {
        let pattern = AtomPattern::new(Predicate::new("p", 3).unwrap(), vec![Term::Variable(1), Term::Constant(Value::Number(first)), Term::Variable(1)]).unwrap();
        let complete = [Value::Number(first), Value::Number(second)];
        let borrowed = [None, Some(&complete[1])];
        let key = pattern.key(borrowed.as_slice()).unwrap();
        prop_assert_eq!(key.to_atom(ValueLimits::default()).unwrap(), Atom::new(pattern.predicate().clone(), vec![complete[1].clone(), complete[0].clone(), complete[1].clone()]).unwrap());
        prop_assert_eq!(key, pattern.key(complete.as_slice()).unwrap());
    }
}

#[test]
fn canonical_bindings_preserve_key_identity() {
    let source_predicate = Predicate::new("source", 1).unwrap();
    for value in values() {
        let catalog = zetesis_core::AtomCatalog::new(vec![
            Atom::new(source_predicate.clone(), vec![value.clone()]).unwrap(),
        ])
        .unwrap();
        let term = catalog.atoms().at(0).unwrap().values().at(0).unwrap();
        let frame = [None, Some(term)];
        let pattern = pattern(Sign::Negative);
        let key = pattern.key(frame.as_slice()).unwrap();
        let expected = Atom::new(pattern.predicate().clone(), vec![value.clone(), value]).unwrap();
        assert_eq!(key.value(0), Some(term));
        assert_eq!(key.value(1), Some(term));
        assert_eq!(key.compare(&expected), std::cmp::Ordering::Equal);
        assert_eq!(trace(&key), trace(&expected));
        assert_eq!(key.to_atom(ValueLimits::default()).unwrap(), expected);
    }
}

#[test]
fn canonical_frames_preserve_absent_slots() {
    let frame: [Option<TermRef<'_>>; 2] = [None, None];
    let view = BindingView::from(frame.as_slice());
    assert_eq!(view.get(0), None);
    assert_eq!(view.get(2), None);
    assert_eq!(
        pattern(Sign::Positive)
            .key(frame.as_slice())
            .unwrap_err()
            .variable,
        1
    );
}

#[test]
fn canonical_key_comparison_honors_each_permit() {
    let value = Value::String("common-prefix".into());
    let predicate = Predicate::new("p", 2).unwrap();
    let atom = Atom::new(predicate, vec![value.clone(), value]).unwrap();
    let catalog = zetesis_core::AtomCatalog::new(vec![atom.clone()]).unwrap();
    let stored = catalog.atoms().at(0).unwrap();
    let frame = [None, stored.values().at(0)];
    let pattern = pattern(Sign::Positive);
    let key = pattern.key(frame.as_slice()).unwrap();
    let mut total = 0;
    assert_eq!(
        key.compare_identity_with(&atom, || {
            total += 1;
            Ok::<(), ()>(())
        }),
        Ok(std::cmp::Ordering::Equal)
    );
    for admitted in 0..total {
        let mut calls = 0;
        let result = key.compare_identity_with(&atom, || {
            let permit = calls < admitted;
            calls += 1;
            if permit { Ok(()) } else { Err("stopped") }
        });
        assert_eq!(result, Err("stopped"));
        assert_eq!(calls, admitted + 1);
    }
}

#[test]
fn key_materialization_checks_output_bytes() {
    let pattern = pattern(Sign::Positive);
    let frame = [Value::Number(0), Value::Number(7)];
    let key = pattern.key(frame.as_slice()).unwrap();
    assert!(matches!(
        key.to_atom(ValueLimits {
            max_bytes: 0,
            ..ValueLimits::default()
        }),
        Err(zetesis_core::catalog::Error::Value(
            zetesis_core::ValueError::Limit {
                resource: zetesis_core::ValueResource::Bytes,
                ..
            }
        ))
    ));
}
