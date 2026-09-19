//! The bounds of each argument are an upper domain of every closure's atoms:
//! constants contribute themselves, a head variable ranges within the
//! positions binding it, and a too-wide argument is unknown.
use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Template, Term, Value};
use zetesis_cpu::{ArgumentBounds, Bound, BoundLimits, Control};

fn number(value: i32) -> Term {
    Term::Constant(Value::Number(value))
}

fn numbers(values: &[i32]) -> Bound {
    Bound::Finite(values.iter().map(|&value| Value::Number(value)).collect())
}

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}

fn fact(name: &str, values: &[i32]) -> Template {
    Template::new(
        Some(pattern(
            name,
            values.iter().map(|&value| number(value)).collect(),
        )),
        vec![],
        vec![],
        vec![],
        vec![],
    )
}

fn rule(head: AtomPattern, body: Vec<AtomPattern>) -> Template {
    Template::new(Some(head), body, vec![], vec![], vec![])
}

fn program(templates: Vec<Template>) -> Program {
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

fn bounds(program: &Program) -> ArgumentBounds {
    ArgumentBounds::infer(program, BoundLimits::default(), &Control::default()).unwrap()
}

/// e(0,1). e(1,2). r(0). r(Y) :- r(X), e(X,Y).
fn reachability() -> Program {
    program(vec![
        fact("e", &[0, 1]),
        fact("e", &[1, 2]),
        fact("r", &[0]),
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
    let bounds = bounds(&reachability());
    let e = Predicate::new("e", 2).unwrap();
    assert_eq!(*bounds.bound(&e, 0), numbers(&[0, 1]));
    assert_eq!(*bounds.bound(&e, 1), numbers(&[1, 2]));
    assert!(bounds.work() > 0);
}

#[test]
fn a_derivation_closes_its_head_argument_over_the_positions_binding_it() {
    // r's argument takes r(0)'s constant and every value e's second
    // argument may bind Y to.
    let bounds = bounds(&reachability());
    let r = Predicate::new("r", 1).unwrap();
    assert_eq!(*bounds.bound(&r, 0), numbers(&[0, 1, 2]));
    assert_eq!(bounds.bounds(&r).map(<[Bound]>::len), Some(1));
}

#[test]
fn a_variable_bound_at_two_positions_takes_their_intersection() {
    // a(1). a(2). b(2). b(3). q(X) :- a(X), b(X).
    let program = program(vec![
        fact("a", &[1]),
        fact("a", &[2]),
        fact("b", &[2]),
        fact("b", &[3]),
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
        *bounds.bound(&Predicate::new("q", 1).unwrap(), 0),
        numbers(&[2])
    );
}

#[test]
fn a_constant_head_argument_contributes_itself() {
    // a(1). s(7,X) :- a(X).
    let program = program(vec![
        fact("a", &[1]),
        rule(
            pattern("s", vec![number(7), Term::Variable(0)]),
            vec![pattern("a", vec![Term::Variable(0)])],
        ),
    ]);
    let bounds = bounds(&program);
    let s = Predicate::new("s", 2).unwrap();
    assert_eq!(*bounds.bound(&s, 0), numbers(&[7]));
    assert_eq!(*bounds.bound(&s, 1), numbers(&[1]));
    // An argument the program does not have is unknown.
    assert_eq!(*bounds.bound(&s, 2), Bound::Unknown);
    assert_eq!(
        *bounds.bound(&Predicate::new("missing", 1).unwrap(), 0),
        Bound::Unknown
    );
}

/// a(1). a(2). a(3). q(X) :- a(X). t(X) :- q(X).
fn three_values_through_two_rules() -> Program {
    program(vec![
        fact("a", &[1]),
        fact("a", &[2]),
        fact("a", &[3]),
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
    ArgumentBounds::infer(
        program,
        BoundLimits {
            max_values: 2,
            ..BoundLimits::default()
        },
        &Control::default(),
    )
    .unwrap()
}

#[test]
fn an_argument_wider_than_the_ceiling_is_unknown() {
    let bounds = narrow_bounds(&three_values_through_two_rules());
    assert_eq!(
        *bounds.bound(&Predicate::new("a", 1).unwrap(), 0),
        Bound::Unknown
    );
}

#[test]
fn an_unknown_argument_makes_the_arguments_bound_through_it_unknown() {
    let bounds = narrow_bounds(&three_values_through_two_rules());
    for name in ["q", "t"] {
        assert_eq!(
            *bounds.bound(&Predicate::new(name, 1).unwrap(), 0),
            Bound::Unknown,
            "{name}"
        );
    }
}

#[test]
fn a_work_ceiling_stops_the_inference() {
    let program = program(vec![fact("a", &[1]), fact("a", &[2])]);
    let stopped = ArgumentBounds::infer(
        &program,
        BoundLimits {
            max_work: 1,
            ..BoundLimits::default()
        },
        &Control::default(),
    );
    assert!(matches!(stopped, Err(zetesis_cpu::Stop::WorkLimit)));
}
