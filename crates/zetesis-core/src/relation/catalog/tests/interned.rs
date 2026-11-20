//! An interned owner agrees with a numbered one on everything but the numbers.

use super::{Fixture, Limits};
use crate::catalog::TermRef;
use crate::relation::Relation;
use crate::{Atom, Predicate, Value, ValueLimits, ValueNode};

fn values() -> Vec<Value> {
    let compound = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 2 },
            ValueNode::Number(1),
            ValueNode::Symbol("a".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    vec![
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
        compound,
        Value::Infimum,
        Value::Supremum,
        Value::Number(-7),
    ]
}

fn tuples() -> Vec<Atom> {
    let values = values();
    let pair = Predicate::new("pair", 2).unwrap();
    let mut tuples = Vec::new();
    for (i, left) in values.iter().enumerate() {
        for right in values.iter().skip(i % 3).step_by(2) {
            tuples.push(Atom::new(pair.clone(), vec![left.clone(), right.clone()]).unwrap());
        }
    }
    // A repeated tuple is the same row in both owners.
    tuples.push(tuples[3].clone());
    tuples
}

fn both() -> (Fixture, Fixture) {
    let pair = Predicate::new("pair", 2).unwrap();
    let mut numbered = Fixture::new(&pair, Limits::default()).unwrap();
    let mut interned = Fixture::interned(&pair, Limits::default()).unwrap();
    for tuple in tuples() {
        let left = numbered.insert(&tuple, Limits::default()).unwrap();
        let right = interned.insert(&tuple, Limits::default()).unwrap();
        assert_eq!((left.row, left.inserted), (right.row, right.inserted));
    }
    (numbered, interned)
}

fn equality(view: &Relation<'_>, column: usize, value: TermRef<'_>) -> Option<u32> {
    view.equality_attempt_with(column, value, Limits::default(), || Ok::<_, ()>(()))
        .result
        .ok()
        .unwrap()
        .map(crate::relation::Equality::value_id)
}

#[test]
fn rows_decode_to_the_same_values_and_equal_ids_mean_equal_values() {
    let (numbered, interned) = both();
    let (left, right) = (numbered.view(), interned.view());
    assert_eq!(left.row_count(), right.row_count());
    // No representative is copied into an interned owner.
    assert!(interned.rows.layout.dictionary.is_empty());
    for column in 0..2 {
        let left_ids: Vec<u32> = left.column(column).unwrap().iter().collect();
        let right_ids: Vec<u32> = right.column(column).unwrap().iter().collect();
        for row in 0..left.row_count() {
            assert_eq!(
                left.row(row).unwrap().value(column),
                right.row(row).unwrap().value(column)
            );
            for other in 0..left.row_count() {
                assert_eq!(
                    left_ids[row] == left_ids[other],
                    right_ids[row] == right_ids[other]
                );
            }
        }
    }
}

#[test]
fn lookups_agree_for_present_absent_and_foreign_values() {
    let (numbered, interned) = both();
    for (left, right) in numbered.atoms().into_iter().zip(interned.atoms()) {
        assert_eq!(
            numbered.lookup(left, Limits::default()).unwrap().row,
            interned.lookup(right, Limits::default()).unwrap().row
        );
    }
    let (left, right) = (numbered.view(), interned.view());
    let mut absent = values();
    absent.push(Value::Number(99));
    for column in 0..2 {
        for value in &absent {
            // Owned values are foreign to both reads; equal content is found.
            let foreign = TermRef::from(value);
            let (a, b) = (
                equality(&left, column, foreign),
                equality(&right, column, foreign),
            );
            assert_eq!(a.is_some(), b.is_some(), "{value:?} in column {column}");
            if let Some(b) = b {
                let row = right
                    .column(column)
                    .unwrap()
                    .iter()
                    .position(|id| id == b)
                    .unwrap();
                assert_eq!(right.row(row).unwrap().value(column), Some(foreign));
                // The interned value names the same ID by identity.
                let local = right.row(row).unwrap().value(column).unwrap();
                assert_eq!(equality(&right, column, local), Some(b));
            }
        }
    }
}

#[test]
fn clearing_an_interned_owner_starts_again() {
    let (_, mut interned) = both();
    interned.clear(Limits::default()).unwrap();
    assert!(interned.rows.is_empty());
    for tuple in tuples() {
        interned.insert(&tuple, Limits::default()).unwrap();
    }
    assert_eq!(interned.view().row_count(), tuples().len() - 1);
}

#[test]
fn a_session_agrees_with_scalar_insertion_and_resolves_each_value_once() {
    let pair = Predicate::new("pair", 2).unwrap();
    let mut scalar = Fixture::interned(&pair, Limits::default()).unwrap();
    let mut session = Fixture::interned(&pair, Limits::default()).unwrap();
    let mut rows = Vec::new();
    let mut appender = session.rows.appender(Limits::default()).unwrap();
    let mut first = true;
    // Every tuple twice: each second insertion finds the first one's row.
    let tuples: Vec<Atom> = tuples().into_iter().flat_map(|t| [t.clone(), t]).collect();
    for tuple in &tuples {
        let left = scalar.insert(tuple, Limits::default()).unwrap();
        let canonical = session
            .authority
            .entry_atom_with(tuple, super::atom_limits(), || Ok::<_, ()>(()))
            .unwrap()
            .insert_ref_with(super::atom_limits(), || Ok::<_, ()>(()))
            .unwrap();
        let right = appender.insert(canonical, Limits::default()).unwrap();
        assert_eq!(
            (left.row, left.inserted),
            (right.insertion.row, right.insertion.inserted)
        );
        assert_eq!(right.equality_ids.is_some(), right.insertion.inserted);
        // A new row resolves its values once, not once to look for the row
        // and again to plan it; a duplicate costs no more than before. The
        // first call grows the session's plan.
        if left.inserted && !first {
            assert!(
                right.insertion.storage.construction_work < left.storage.construction_work,
                "{tuple:?}"
            );
        } else if !left.inserted {
            assert!(
                right.insertion.storage.construction_work <= left.storage.construction_work,
                "{tuple:?}"
            );
        }
        first = false;
        rows.push(right.insertion.row);
    }
    drop(appender);
    let (left, right) = (scalar.view(), session.view());
    assert_eq!(left.row_count(), right.row_count());
    for row in 0..left.row_count() {
        for column in 0..2 {
            assert_eq!(
                left.column(column).unwrap().iter().nth(row),
                right.column(column).unwrap().iter().nth(row)
            );
        }
    }
    assert_eq!(rows.len(), tuples.len());
}
