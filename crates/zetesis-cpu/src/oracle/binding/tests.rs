use zetesis_core::{Term, Value};

use super::{Row, TermRef, Work, bind, clear};
use crate::{Cancellation, Stop};
use zetesis_test_support::programs::{atom, pattern};

#[test]
fn captures_charge_arguments_and_one_end_probe() {
    let pattern = pattern("row", vec![Term::Variable(0), Term::Variable(1)]);
    let atom = atom("row", vec![Value::Number(3), Value::Number(4)]);
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, 3);
    let mut assignment = [None, None];
    let mut undo = Vec::with_capacity(2);
    assert_eq!(
        bind(
            (&pattern).into(),
            Row::Atom((&atom).into()),
            &mut assignment,
            &mut undo,
            &mut work
        ),
        Ok(true)
    );
    assert_eq!(work.statistics.work, 3);
    clear(&mut assignment, &mut undo);
    assert_eq!(assignment, [None, None]);
}

#[test]
fn final_probe_refusal_is_not_a_completed_binding() {
    let pattern = pattern("row", vec![Term::Variable(0), Term::Variable(1)]);
    let atom = atom("row", vec![Value::Number(3), Value::Number(4)]);
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, 2);
    let mut assignment = [None, None];
    let mut undo = Vec::with_capacity(2);
    assert_eq!(
        bind(
            (&pattern).into(),
            Row::Atom((&atom).into()),
            &mut assignment,
            &mut undo,
            &mut work
        ),
        Err(Stop::WorkLimit)
    );
    assert_eq!(work.statistics.work, 2);
    assert_eq!(undo, [0, 1]);
    clear(&mut assignment, &mut undo);
    assert_eq!(assignment, [None, None]);
}

#[test]
fn nullary_matching_charges_one_end_probe() {
    let pattern = pattern("row", vec![]);
    let atom = atom("row", vec![]);
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, 1);
    assert_eq!(
        bind(
            (&pattern).into(),
            Row::Atom((&atom).into()),
            &mut [],
            &mut Vec::new(),
            &mut work
        ),
        Ok(true)
    );
    assert_eq!(work.statistics.work, 1);
}

#[test]
fn mismatch_preserves_existing_comparison_work() {
    let expected = Value::Number(3);
    let actual = Value::Number(4);
    let cancellation = Cancellation::default();
    let mut comparison = Work::source(&cancellation, u64::MAX);
    assert!(
        !TermRef::from(&expected)
            .equals_ref_with((&actual).into(), || comparison.tick())
            .unwrap()
    );
    let pattern = pattern("row", vec![Term::Constant(expected)]);
    let atom = atom("row", vec![actual]);
    let required = 1 + comparison.statistics.work;
    let mut work = Work::source(&cancellation, required);
    assert_eq!(
        bind(
            (&pattern).into(),
            Row::Atom((&atom).into()),
            &mut [],
            &mut Vec::new(),
            &mut work
        ),
        Ok(false)
    );
    assert_eq!(work.statistics.work, required);
}

#[test]
fn malformed_row_shape_stops_the_cpu_join() {
    let pattern = pattern("row", vec![Term::Variable(0)]);
    let atom = atom("row", vec![Value::Number(3), Value::Number(4)]);
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, 2);
    assert_eq!(
        bind(
            (&pattern).into(),
            Row::Atom((&atom).into()),
            &mut [None],
            &mut Vec::with_capacity(1),
            &mut work
        ),
        Err(Stop::InvalidProgram)
    );
}

#[test]
fn missing_prepared_trail_stops_before_capture() {
    let pattern = pattern("row", vec![Term::Variable(0)]);
    let atom = atom("row", vec![Value::Number(3)]);
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, 1);
    let mut assignment = [None];
    assert_eq!(
        bind(
            (&pattern).into(),
            Row::Atom((&atom).into()),
            &mut assignment,
            &mut Vec::new(),
            &mut work
        ),
        Err(Stop::InvalidProgram)
    );
    assert_eq!(assignment, [None]);
}
