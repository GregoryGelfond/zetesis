//! Equivalent source programs with explicit templates and anchored table facts.

use zetesis_core::{
    AdmissionLimits, FactTables, Predicate, Program, Template, Term, Value, ValueNode,
};

use super::{fact, function, pattern, program, value};

/// A shared structured argument in the interleaved source fixture.
///
/// # Panics
/// Panics if constructing this fixed, valid value fails.
#[must_use]
pub fn structured() -> Value {
    value(vec![function("f", 1), ValueNode::Number(7)])
}

/// The same ordered source, first as templates alone, then with table facts:
/// `seed(f(7)). joined(X) :- seed(X), not blocked(X). ready. seed(f(7)).`
/// `:- joined(f(7)), not permit. tail.`
///
/// The duplicate fact straddles a template and the two nullary facts have
/// different anchors. Both templates remain templates in both programs.
///
/// # Panics
/// Panics if this fixed, safe program cannot be admitted.
#[must_use]
pub fn interleaved() -> [Program; 2] {
    let argument = structured();
    let joined = Template::new(
        Some(pattern("joined", vec![Term::Variable(0)])),
        vec![pattern("seed", vec![Term::Variable(0)])],
        vec![],
        vec![pattern("blocked", vec![Term::Variable(0)])],
        vec![],
    );
    let constraint = Template::new(
        None,
        vec![pattern("joined", vec![Term::Constant(argument.clone())])],
        vec![],
        vec![pattern("permit", vec![])],
        vec![],
    );
    let explicit = program(vec![
        fact("seed", vec![Term::Constant(argument.clone())]),
        joined.clone(),
        fact("ready", vec![]),
        fact("seed", vec![Term::Constant(argument.clone())]),
        constraint.clone(),
        fact("tail", vec![]),
    ]);
    let mut facts = FactTables::default();
    for (before, name, arguments) in [
        (0, "seed", vec![argument.clone()]),
        (1, "ready", vec![]),
        (1, "seed", vec![argument]),
        (2, "tail", vec![]),
    ] {
        let predicate = Predicate::new(name, arguments.len()).unwrap();
        facts.push_row(before, predicate, arguments).unwrap();
    }
    let table = Program::new(vec![joined, constraint], facts, AdmissionLimits::default()).unwrap();
    [explicit, table]
}

/// Three occurrences of `ready.` without any domain values, first as explicit
/// templates and then as table facts sharing one anchor.
///
/// # Panics
/// Panics if this fixed, safe program cannot be admitted.
#[must_use]
pub fn nullary_duplicates() -> [Program; 2] {
    let explicit = program(vec![fact("ready", vec![]); 3]);
    let mut facts = FactTables::default();
    for _ in 0..3 {
        facts
            .push_row(0, Predicate::new("ready", 0).unwrap(), vec![])
            .unwrap();
    }
    let table = Program::new(vec![], facts, AdmissionLimits::default()).unwrap();
    [explicit, table]
}
