//! Compare ordered lookup with an independent linear prefix selection.

use zetesis_core::{
    Atom, AtomCatalog, AtomPattern, Predicate, Sign, Term, Value, ValueLimits, ValueNode,
    catalog::AtomRef,
};

use super::*;
use crate::Cancellation;

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
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, limit);
    let rows: Vec<_> = rows.iter().map(|atom| AtomRef::from(*atom)).collect();
    let assignment: Vec<_> = assignment
        .iter()
        .map(|value| value.map(TermRef::from))
        .collect();
    let range = matching_prefix(
        pattern.into(),
        Rows::Borrowed(&rows),
        0,
        &assignment,
        &mut work,
    )?;
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

#[test]
fn a_bound_prefix_of_a_dense_relation_is_its_block_of_positions() {
    use crate::oracle::argument_bounds::Bound;
    use crate::oracle::relations::{Catalogs, Layout, Layouts, Relation, RowSet, fixtures};

    // Three values by two: six positions, the first argument most significant.
    let predicate = Predicate::new("row", 2).unwrap();
    let source: Vec<_> = [1, 2, 3]
        .into_iter()
        .flat_map(|left| {
            [10, 20]
                .into_iter()
                .map(move |right| tuple(vec![Value::Number(left), Value::Number(right)]))
        })
        .collect();
    let program = fixtures::program(&source);
    let axis = |values: &[i32]| {
        Bound::Finite(
            values
                .iter()
                .map(|&n| program.domain().binary_search(&Value::Number(n)).unwrap())
                .collect(),
        )
    };
    let mut layouts = Layouts::default();
    layouts.push(
        Layout::new(
            &program,
            (&predicate).into(),
            &[axis(&[1, 2, 3]), axis(&[10, 20])],
            64,
        )
        .unwrap()
        .unwrap(),
    );
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, 1_000);
    work.limits.max_closure_bytes = 1 << 20;
    let mut catalogs = Catalogs::default();
    catalogs.bind_program(&program, &mut work).unwrap();
    catalogs
        .create_dense_relations(&layouts, &mut work)
        .unwrap();
    let Relation::Dense(relation) = catalogs.relation(&predicate) else {
        panic!("dense fixture");
    };
    let rows = Rows::Dense {
        relation,
        set: RowSet::Current,
    };
    let two = Value::Number(2);
    let twenty = Value::Number(20);
    let nine = Value::Number(9);
    let open = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    let constant = pattern(vec![Term::Constant(two.clone()), Term::Variable(1)]);
    let mut range = |pattern: &AtomPattern, assignment: &[Option<&Value>]| {
        let assignment: Vec<_> = assignment
            .iter()
            .map(|value| value.map(TermRef::from))
            .collect();
        matching_prefix(pattern.into(), rows, 0, &assignment, &mut work).unwrap()
    };
    assert_eq!(range(&open, &[None, None]), 0..6);
    assert_eq!(range(&open, &[Some(&two), None]), 2..4);
    assert_eq!(range(&open, &[Some(&two), Some(&twenty)]), 3..4);
    assert_eq!(range(&constant, &[None, None]), 2..4);
    // A later bound term does not narrow past an unbound one.
    assert_eq!(range(&open, &[None, Some(&twenty)]), 0..6);
    // A value outside its argument's bound matches no position.
    assert_eq!(range(&open, &[Some(&nine), None]), 0..0);
}

#[test]
fn canonical_rows_and_bindings_keep_prefix_selection_and_refusals() {
    let mut atoms: Vec<_> = values()
        .into_iter()
        .map(|value| tuple(vec![value]))
        .collect();
    atoms.sort();
    // The expected position comes from the independent owned ingress ordering.
    let wanted = Value::String("a".into());
    let expected = atoms
        .iter()
        .position(|atom| atom.values() == [wanted.clone()])
        .unwrap();
    let catalog = AtomCatalog::new(atoms).unwrap();
    let rows: Vec<_> = catalog.atoms().iter().collect();
    // A separate owner supplies the binding; local IDs cannot establish equality.
    let query = AtomCatalog::new(vec![tuple(vec![wanted])]).unwrap();
    let assignment = [Some(query.atoms().at(0).unwrap().values().at(0).unwrap())];
    let pattern = pattern(vec![Term::Variable(0)]);
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    let range = matching_prefix(
        (&pattern).into(),
        Rows::Borrowed(&rows),
        0,
        &assignment,
        &mut work,
    )
    .unwrap();
    assert_eq!(range, expected..expected + 1);
    let used = work.statistics.work;
    for limit in 0..used {
        let mut work = Work::source(&cancellation, limit);
        assert_eq!(
            matching_prefix(
                (&pattern).into(),
                Rows::Borrowed(&rows),
                0,
                &assignment,
                &mut work
            ),
            Err(Stop::WorkLimit)
        );
    }
    let mut work = Work::source(&cancellation, used);
    assert_eq!(
        matching_prefix(
            (&pattern).into(),
            Rows::Borrowed(&rows),
            0,
            &assignment,
            &mut work
        ),
        Ok(range)
    );
}
