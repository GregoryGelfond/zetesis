//! Small canonical values and complete reads shared by the acceptance tests.

use std::collections::BTreeSet;
use themelios_program::{AnswerSet, Name, Source, SourceId, Symbol};
use themelios_solve::{
    bridge::Admitted,
    contract::{Backend, SolveRequest},
    outcome::{Conclusion, Model},
};
use themelios_syntax::{dialect::Dialect, parse::parse};
use zetesis_engine::Solver;

pub(super) fn admitted(text: &str) -> Admitted {
    let source = Source::new(SourceId::new(0), text.to_owned()).unwrap();
    Admitted::of(&parse(&source, Dialect::Clingo)).unwrap()
}

pub(super) fn name(text: &str) -> Name {
    Name::new(text).unwrap()
}

pub(super) fn constant(text: &str) -> Symbol {
    Symbol::constant(name(text))
}

pub(super) fn unary(text: &str, value: i32) -> Symbol {
    Symbol::function(
        name(text),
        [Symbol::Number(value)],
        themelios_program::Sign::Positive,
    )
}

pub(super) fn complete(solver: &mut Solver) -> Vec<Model> {
    let mut run = solver.solve(&SolveRequest::default()).unwrap();
    let models = run.all_models().unwrap();
    assert_eq!(run.conclusion(), Some(Conclusion::Exhausted));
    models
}

pub(super) fn family(solver: &mut Solver) -> BTreeSet<AnswerSet> {
    complete(solver)
        .into_iter()
        .map(|model| model.atoms().clone())
        .collect()
}
