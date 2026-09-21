//! The bounds of each argument are an upper domain of every closure's atoms:
//! constants contribute themselves, a head variable ranges within the
//! positions binding it, and a too-wide argument is unknown.
use zetesis_core::{AtomPattern, Predicate, Program, Template, Term, Value};

use super::{ArgumentBounds, Bound, infer};
use crate::oracle::{PreparationLimits, Work};
use crate::{Cancellation, Limits, Stop};

#[path = "../../tests/support/programs.rs"]
mod programs;

use programs::{fact, number, pattern, program};

fn numbers(values: &[i32]) -> Bound {
    Bound::Finite(values.iter().map(|&value| Value::Number(value)).collect())
}

fn rule(head: AtomPattern, body: Vec<AtomPattern>) -> Template {
    Template::new(Some(head), body, vec![], vec![], vec![])
}

/// The bound of one argument; unknown for a predicate or an argument the
/// program does not have.
fn bound<'a>(bounds: &'a ArgumentBounds, predicate: &Predicate, argument: usize) -> &'a Bound {
    const UNKNOWN: Bound = Bound::Unknown;
    bounds
        .bounds(predicate)
        .and_then(|bounds| bounds.get(argument))
        .unwrap_or(&UNKNOWN)
}

/// The bounds under preparation's ceiling and an unlimited work.
fn bounds(program: &Program) -> ArgumentBounds {
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, Limits::default().max_work);
    infer(
        program,
        PreparationLimits::default().max_dense_atoms,
        &mut work,
    )
    .unwrap()
}

/// e(0,1). e(1,2). r(0). r(Y) :- r(X), e(X,Y).
fn reachability() -> Program {
    program(vec![
        fact("e", vec![number(0), number(1)]),
        fact("e", vec![number(1), number(2)]),
        fact("r", vec![number(0)]),
        rule(
            pattern("r", vec![Term::Variable(1)]),
            vec![
                pattern("r", vec![Term::Variable(0)]),
                pattern("e", vec![Term::Variable(0), Term::Variable(1)]),
            ],
        ),
    ])
}

#[test]
fn facts_bound_their_arguments() {
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, Limits::default().max_work);
    let bounds = infer(
        &reachability(),
        PreparationLimits::default().max_dense_atoms,
        &mut work,
    )
    .unwrap();
    let e = Predicate::new("e", 2).unwrap();
    assert_eq!(*bound(&bounds, &e, 0), numbers(&[0, 1]));
    assert_eq!(*bound(&bounds, &e, 1), numbers(&[1, 2]));
    assert!(work.statistics.work > 0);
}

#[test]
fn a_derivation_closes_its_head_argument_over_the_positions_binding_it() {
    // r's argument takes r(0)'s constant and every value e's second
    // argument may bind Y to.
    let bounds = bounds(&reachability());
    let r = Predicate::new("r", 1).unwrap();
    assert_eq!(*bound(&bounds, &r, 0), numbers(&[0, 1, 2]));
    assert_eq!(bounds.bounds(&r).map(<[Bound]>::len), Some(1));
}

#[test]
fn a_variable_bound_at_two_positions_takes_their_intersection() {
    // a(1). a(2). b(2). b(3). q(X) :- a(X), b(X).
    let program = program(vec![
        fact("a", vec![number(1)]),
        fact("a", vec![number(2)]),
        fact("b", vec![number(2)]),
        fact("b", vec![number(3)]),
        rule(
            pattern("q", vec![Term::Variable(0)]),
            vec![
                pattern("a", vec![Term::Variable(0)]),
                pattern("b", vec![Term::Variable(0)]),
            ],
        ),
    ]);
    let bounds = bounds(&program);
    assert_eq!(
        *bound(&bounds, &Predicate::new("q", 1).unwrap(), 0),
        numbers(&[2])
    );
}

#[test]
fn a_constant_head_argument_contributes_itself() {
    // a(1). s(7,X) :- a(X).
    let program = program(vec![
        fact("a", vec![number(1)]),
        rule(
            pattern("s", vec![number(7), Term::Variable(0)]),
            vec![pattern("a", vec![Term::Variable(0)])],
        ),
    ]);
    let bounds = bounds(&program);
    let s = Predicate::new("s", 2).unwrap();
    assert_eq!(*bound(&bounds, &s, 0), numbers(&[7]));
    assert_eq!(*bound(&bounds, &s, 1), numbers(&[1]));
    // An argument the program does not have is unknown.
    assert_eq!(*bound(&bounds, &s, 2), Bound::Unknown);
    assert_eq!(
        *bound(&bounds, &Predicate::new("missing", 1).unwrap(), 0),
        Bound::Unknown
    );
}

/// a(1). a(2). a(3). q(X) :- a(X). t(X) :- q(X).
fn three_values_through_two_rules() -> Program {
    program(vec![
        fact("a", vec![number(1)]),
        fact("a", vec![number(2)]),
        fact("a", vec![number(3)]),
        rule(
            pattern("q", vec![Term::Variable(0)]),
            vec![pattern("a", vec![Term::Variable(0)])],
        ),
        rule(
            pattern("t", vec![Term::Variable(0)]),
            vec![pattern("q", vec![Term::Variable(0)])],
        ),
    ])
}

/// The bounds with two values the widest bound kept.
fn narrow_bounds(program: &Program) -> ArgumentBounds {
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, Limits::default().max_work);
    infer(program, 2, &mut work).unwrap()
}

#[test]
fn an_argument_wider_than_the_ceiling_is_unknown() {
    let bounds = narrow_bounds(&three_values_through_two_rules());
    assert_eq!(
        *bound(&bounds, &Predicate::new("a", 1).unwrap(), 0),
        Bound::Unknown
    );
}

#[test]
fn an_unknown_argument_makes_the_arguments_bound_through_it_unknown() {
    let bounds = narrow_bounds(&three_values_through_two_rules());
    for name in ["q", "t"] {
        assert_eq!(
            *bound(&bounds, &Predicate::new(name, 1).unwrap(), 0),
            Bound::Unknown,
            "{name}"
        );
    }
}

#[test]
fn a_work_ceiling_stops_the_inference() {
    let program = program(vec![fact("a", vec![number(1)]), fact("a", vec![number(2)])]);
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, 1);
    let stopped = infer(
        &program,
        PreparationLimits::default().max_dense_atoms,
        &mut work,
    );
    assert!(matches!(stopped, Err(Stop::WorkLimit)));
}
