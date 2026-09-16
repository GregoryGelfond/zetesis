//! Compare ordered lookup with an independent linear prefix selection.

use zetesis_core::{Predicate, Sign, Term, ValueLimits, ValueNode};

use super::*;
use crate::Control;

fn tuple(values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new("row", values.len()).unwrap(), values).unwrap()
}

fn pattern(terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new("row", terms.len()).unwrap(), terms).unwrap()
}

fn values() -> Vec<Value> {
    let function = |sign| {
        Value::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f".into(),
                    sign,
                    arity: 1,
                },
                ValueNode::Number(1),
            ],
            ValueLimits::default(),
        )
        .unwrap()
    };
    vec![
        Value::Infimum,
        Value::Number(-1),
        Value::Number(0),
        Value::String("a".into()),
        Value::Symbol("a".into()),
        function(Sign::Positive),
        function(Sign::Negative),
        Value::Supremum,
    ]
}

fn lookup(
    pattern: &AtomPattern,
    rows: &[&Atom],
    assignment: &[Option<&Value>],
    limit: u64,
) -> Result<(Range<usize>, u64), Stop> {
    let control = Control::default();
    let mut work = Work::source(&control, limit);
    let range = matching_prefix(pattern, Rows::Borrowed(rows), 0, assignment, &mut work)?;
    Ok((range, work.statistics.work))
}

fn assert_linear_selection(pattern: &AtomPattern, rows: &[&Atom], assignment: &[Option<&Value>]) {
    // Build a test-only concrete key without calling resolve or the ordered
    // comparator. An unknown value ends the prefix even if later terms are known.
    let mut key = Vec::new();
    for term in pattern.terms() {
        match term {
            Term::Constant(value) => key.push(value.clone()),
            Term::Variable(slot) => match assignment[*slot] {
                Some(value) => key.push(value.clone()),
                None => break,
            },
        }
    }
    let expected: Vec<_> = rows
        .iter()
        .copied()
        .filter(|row| row.values().starts_with(&key))
        .collect();
    let (range, _) = lookup(pattern, rows, assignment, u64::MAX).unwrap();
    assert_eq!(&rows[range], expected);
}

#[test]
fn windows_equal_linear_prefix_selection() {
    let values = values();
    let mut rows: Vec<_> = values
        .iter()
        .flat_map(|left| {
            values
                .iter()
                .map(move |right| tuple(vec![left.clone(), right.clone()]))
        })
        .collect();
    rows.sort();
    let rows: Vec<_> = rows.iter().collect();
    let mut terms: Vec<_> = values.iter().cloned().map(Term::Constant).collect();
    terms.extend([Term::Variable(0), Term::Variable(1)]);
    let mut assignments: Vec<_> = values.iter().map(Some).collect();
    assignments.push(None);
    for first in &terms {
        for second in &terms {
            let pattern = pattern(vec![first.clone(), second.clone()]);
            for left in &assignments {
                for right in &assignments {
                    assert_linear_selection(&pattern, &rows, &[*left, *right]);
                }
            }
        }
    }
}

#[test]
fn absent_keys_return_empty_windows() {
    let rows: Vec<_> = [-2, 0, 2]
        .into_iter()
        .map(|n| tuple(vec![Value::Number(n)]))
        .collect();
    let rows: Vec<_> = rows.iter().collect();
    for n in [-3, -1, 1, 3] {
        let pattern = pattern(vec![Term::Constant(Value::Number(n))]);
        assert_linear_selection(&pattern, &rows, &[]);
    }
}

#[test]
fn empty_relations_need_no_lookup_work() {
    let (range, work) = lookup(&pattern(vec![Term::Variable(0)]), &[], &[None], 0).unwrap();
    assert_eq!(range, 0..0);
    assert_eq!(work, 0);
}

#[test]
fn nullary_relations_keep_their_complete_window() {
    let row = tuple(vec![]);
    let (range, work) = lookup(&pattern(vec![]), &[&row], &[], 0).unwrap();
    assert_eq!(range, 0..1);
    assert_eq!(work, 0);
}

#[test]
fn lookup_work_limit_is_inclusive() {
    let rows: Vec<_> = (0..128).map(|n| tuple(vec![Value::Number(n)])).collect();
    let rows: Vec<_> = rows.iter().collect();
    let pattern = pattern(vec![Term::Constant(Value::Number(63))]);
    let (range, work) = lookup(&pattern, &rows, &[], u64::MAX).unwrap();
    assert_eq!(range, 63..64);
    assert!(work > 1);
    assert_eq!(lookup(&pattern, &rows, &[], work), Ok((range, work)));
    assert_eq!(lookup(&pattern, &rows, &[], work - 1), Err(Stop::WorkLimit));
}

#[test]
fn lookup_charges_compared_text_payloads() {
    let row = tuple(vec![Value::String("long-key".into())]);
    let pattern = pattern(vec![Term::Constant(Value::String("long-key".into()))]);
    assert_eq!(lookup(&pattern, &[&row], &[], 2), Err(Stop::WorkLimit));
}
