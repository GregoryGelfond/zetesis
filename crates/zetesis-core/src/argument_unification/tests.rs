use std::{cell::Cell, convert::Infallible, panic::AssertUnwindSafe};

use crate::{Atom, AtomCatalog, AtomPattern, PatternRef, Predicate, Term, Value};

use super::{TermRef, UnificationError, UnificationFailure};

const PERMIT: fn() -> Result<(), Infallible> = || Ok(());

fn pattern(terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new("relation", terms.len()).unwrap(), terms).unwrap()
}

fn canonical(value: Value) -> AtomCatalog {
    AtomCatalog::new(vec![
        Atom::new(Predicate::new("row", 1).unwrap(), vec![value]).unwrap(),
    ])
    .unwrap()
}

fn first(catalog: &AtomCatalog) -> TermRef<'_> {
    catalog.atoms().at(0).unwrap().values().at(0).unwrap()
}

#[test]
fn repeated_variables_use_the_first_capture() {
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(0)]);
    let values = [Value::Number(3), Value::Number(3)];
    let mut assignment = [None];
    let mut changes = Vec::with_capacity(1);
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            values.iter().map(TermRef::from),
            &mut assignment,
            &mut changes,
            PERMIT,
        ),
        Ok(true)
    );
    assert_eq!(changes, [0]);
    assert_eq!(assignment, [Some(TermRef::from(&values[0]))]);
}

#[test]
fn repeated_variable_mismatch_keeps_undo_information() {
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(0)]);
    let values = [Value::Number(3), Value::Number(4)];
    let mut assignment = [None];
    let mut changes = Vec::with_capacity(1);
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            values.iter().map(TermRef::from),
            &mut assignment,
            &mut changes,
            PERMIT,
        ),
        Ok(false)
    );
    assert_eq!(changes, [0]);
    assert_eq!(assignment, [Some(TermRef::from(&values[0]))]);
}

#[test]
fn bound_variables_need_no_trail_capacity() {
    let pattern = pattern(vec![Term::Variable(0)]);
    let value = Value::Number(3);
    let mut assignment = [Some(TermRef::from(&value))];
    let mut changes = Vec::new();
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            [TermRef::from(&value)],
            &mut assignment,
            &mut changes,
            PERMIT,
        ),
        Ok(true)
    );
    assert!(changes.is_empty());
    assert_eq!(changes.capacity(), 0);
}

#[test]
fn strings_do_not_match_symbolic_constants() {
    let pattern = pattern(vec![Term::Constant(Value::Symbol("same".into()))]);
    let value = Value::String("same".into());
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            [TermRef::from(&value)],
            &mut [],
            &mut Vec::new(),
            PERMIT,
        ),
        Ok(false)
    );
}

#[test]
fn foreign_canonical_coordinates_do_not_imply_equality() {
    let left = canonical(Value::Number(3));
    let right = canonical(Value::Number(4));
    let predicate = Predicate::new("relation", 1).unwrap();
    let arguments = [crate::TemplateTerm::Constant(first(&left))];
    let pattern = PatternRef::from_parts((&predicate).into(), &arguments).unwrap();
    assert_eq!(
        pattern
            .terms()
            .unify_with([first(&right)], &mut [], &mut Vec::new(), PERMIT,),
        Ok(false)
    );
}

#[test]
fn equal_foreign_canonical_terms_match() {
    let left = canonical(Value::Symbol("closed".into()));
    let right = canonical(Value::Symbol("closed".into()));
    let predicate = Predicate::new("relation", 1).unwrap();
    let arguments = [crate::TemplateTerm::Constant(first(&left))];
    let pattern = PatternRef::from_parts((&predicate).into(), &arguments).unwrap();
    assert_eq!(
        pattern
            .terms()
            .unify_with([first(&right)], &mut [], &mut Vec::new(), PERMIT,),
        Ok(true)
    );
}

#[test]
fn short_rows_return_the_consumed_arity() {
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    let value = Value::Number(3);
    let mut assignment = [None, None];
    let mut changes = Vec::with_capacity(2);
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            [TermRef::from(&value)],
            &mut assignment,
            &mut changes,
            PERMIT,
        ),
        Err(UnificationFailure::Input(UnificationError::RowTooShort {
            expected: 2,
            observed: 1,
        }))
    );
    assert_eq!(changes, [0]);
    assert!(assignment[1].is_none());
}

#[test]
fn the_end_probe_bounds_an_infinite_row() {
    let pattern = pattern(vec![Term::Variable(0)]);
    let value = Value::Number(3);
    let read = Cell::new(0);
    let row = std::iter::repeat(TermRef::from(&value)).inspect(|_| read.set(read.get() + 1));
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            row,
            &mut [None],
            &mut Vec::with_capacity(1),
            PERMIT,
        ),
        Err(UnificationFailure::Input(UnificationError::RowTooLong {
            expected: 1
        }))
    );
    assert_eq!(read.get(), 2);
}

#[test]
fn an_outside_slot_is_a_typed_failure() {
    let pattern = pattern(vec![Term::Variable(usize::MAX)]);
    let value = Value::Number(3);
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            [TermRef::from(&value)],
            &mut [],
            &mut Vec::new(),
            PERMIT,
        ),
        Err(UnificationFailure::Input(UnificationError::Variable {
            variable: usize::MAX,
            slots: 0,
        }))
    );
}

#[test]
fn exhausted_trail_capacity_precedes_capture() {
    let pattern = pattern(vec![Term::Variable(0)]);
    let value = Value::Number(3);
    let mut assignment = [None];
    let mut changes = Vec::new();
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            [TermRef::from(&value)],
            &mut assignment,
            &mut changes,
            PERMIT,
        ),
        Err(UnificationFailure::Input(UnificationError::TrailCapacity {
            capacity: 0
        }))
    );
    assert_eq!(assignment, [None]);
    assert_eq!(changes.capacity(), 0);
}

#[test]
fn captures_reuse_the_prepared_trail() {
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    let values = [Value::Number(3), Value::Number(4)];
    let mut assignment = [None, None];
    let mut changes = Vec::with_capacity(3);
    changes.push(7);
    let pointer = changes.as_ptr();
    let capacity = changes.capacity();
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            values.iter().map(TermRef::from),
            &mut assignment,
            &mut changes,
            PERMIT,
        ),
        Ok(true)
    );
    assert_eq!(changes, [7, 0, 1]);
    assert_eq!(changes.as_ptr(), pointer);
    assert_eq!(changes.capacity(), capacity);
}

#[test]
fn nullary_rows_need_the_end_permit() {
    let pattern = pattern(vec![]);
    assert_eq!(
        PatternRef::from(&pattern)
            .terms()
            .unify_with([], &mut [], &mut Vec::new(), || Err("stopped"),),
        Err(UnificationFailure::Stopped("stopped"))
    );
    assert_eq!(
        PatternRef::from(&pattern)
            .terms()
            .unify_with([], &mut [], &mut Vec::new(), PERMIT),
        Ok(true)
    );
}

#[test]
fn a_refused_argument_is_not_read() {
    let pattern = pattern(vec![Term::Variable(0)]);
    let value = Value::Number(3);
    let read = Cell::new(0);
    let row = [TermRef::from(&value)]
        .into_iter()
        .inspect(|_| read.set(read.get() + 1));
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            row,
            &mut [None],
            &mut Vec::with_capacity(1),
            || Err("stopped"),
        ),
        Err(UnificationFailure::Stopped("stopped"))
    );
    assert_eq!(read.get(), 0);
}

#[test]
fn mismatch_does_not_inspect_unused_arguments() {
    let pattern = pattern(vec![Term::Constant(Value::Number(1)), Term::Variable(0)]);
    let value = Value::Number(2);
    let mut calls = 0;
    let row = std::iter::from_fn(|| {
        calls += 1;
        assert_eq!(calls, 1, "mismatch must stop before the unused suffix");
        Some(TermRef::from(&value))
    });
    assert_eq!(
        PatternRef::from(&pattern).terms().unify_with(
            row,
            &mut [None],
            &mut Vec::with_capacity(1),
            PERMIT,
        ),
        Ok(false)
    );
}

#[test]
fn every_refusal_keeps_a_recoverable_trail() {
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(0)]);
    let values = [
        Value::Symbol("borrowed".into()),
        Value::Symbol("borrowed".into()),
    ];
    let terms = PatternRef::from(&pattern).terms();
    let mut required = 0;
    assert_eq!(
        terms.unify_with(
            values.iter().map(TermRef::from),
            &mut [None],
            &mut Vec::with_capacity(1),
            || {
                required += 1;
                Ok::<_, Infallible>(())
            },
        ),
        Ok(true)
    );
    for cutoff in 0..required {
        let mut assignment = [None];
        let mut changes = Vec::with_capacity(1);
        let mut accepted = 0;
        let result = terms.unify_with(
            values.iter().map(TermRef::from),
            &mut assignment,
            &mut changes,
            || {
                if accepted == cutoff {
                    return Err(cutoff);
                }
                accepted += 1;
                Ok(())
            },
        );
        assert_eq!(result, Err(UnificationFailure::Stopped(cutoff)));
        assert_eq!(accepted, cutoff);
        for variable in changes.drain(..) {
            assignment[variable] = None;
        }
        assert_eq!(assignment, [None]);
        assert_eq!(
            terms.unify_with(
                values.iter().map(TermRef::from),
                &mut assignment,
                &mut changes,
                PERMIT,
            ),
            Ok(true)
        );
    }
}

#[test]
fn unwind_retains_the_successful_capture_trail() {
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    let values = [Value::Number(3), Value::Number(4)];
    let mut assignment = [None, None];
    let mut changes = Vec::with_capacity(2);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut calls = 0;
        PatternRef::from(&pattern).terms().unify_with(
            values.iter().map(TermRef::from),
            &mut assignment,
            &mut changes,
            || {
                calls += 1;
                assert_eq!(calls, 1, "stop before the second argument");
                Ok::<_, Infallible>(())
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(changes, [0]);
    assert_eq!(assignment, [Some(TermRef::from(&values[0])), None]);
}
