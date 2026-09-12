//! Borrowed substitution identity is the identity of the materialized atom.
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use proptest::prelude::*;
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
            atoms.push(key.to_atom());
            for atom in &atoms {
                assert_eq!(key.compare(atom), key.to_atom().cmp(atom));
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
            assert_eq!(trace(&key), trace(&key.to_atom()));
        }
    }
}

#[test]
fn membership_borrows_the_stored_atom() {
    let pattern = pattern(Sign::Negative);
    let frame = [None, Some(Value::Number(7))];
    let key = pattern.key(frame.as_slice()).unwrap();
    let atoms = BTreeSet::from([key.to_atom()]);
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
    assert_eq!(view.get(1), Some(&Value::Number(7)));
    assert_eq!(view.get(2), None);
}

#[test]
fn catalog_key_receipts_match_owned_lookup() {
    let pattern = pattern(Sign::Positive);
    let mut catalog = Catalog::new(pattern.predicate().clone(), Limits::default()).unwrap();
    for value in values() {
        let frame = [None, Some(value)];
        let key = pattern.key(frame.as_slice()).unwrap();
        catalog.insert(key.to_atom(), Limits::default()).unwrap();
        let expected = catalog.lookup(&key.to_atom(), Limits::default()).unwrap();
        assert_eq!(
            catalog.lookup_key(&key, Limits::default()).unwrap(),
            expected
        );
        let exact = Limits {
            max_work: u64::try_from(expected.storage.construction_work).unwrap(),
            ..Limits::default()
        };
        assert_eq!(catalog.lookup_key(&key, exact).unwrap(), expected);
        let short = Limits {
            max_work: exact.max_work - 1,
            ..exact
        };
        assert_eq!(
            catalog.lookup_key(&key, short).unwrap_err(),
            catalog.lookup(&key.to_atom(), short).unwrap_err()
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
        prop_assert_eq!(key.to_atom(), Atom::new(pattern.predicate().clone(), vec![complete[1].clone(), complete[0].clone(), complete[1].clone()]).unwrap());
        prop_assert_eq!(key, pattern.key(complete.as_slice()).unwrap());
    }
}
