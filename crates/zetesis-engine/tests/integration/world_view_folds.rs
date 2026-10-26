//! The native session's world-view folds agree with the agent's readings of
//! the same program's complete family.

use std::{collections::HashSet, num::NonZeroUsize, sync::Arc};

use themelios_macros::{atom, program};
use themelios_program::{AnswerSet, Program, Sign as SourceSign, Symbol};
use themelios_query::{AgentReading, Answer, Query};
use themelios_solve::{
    agent::Agent,
    contract::{Fault, Presupposition, Refused},
};
use zetesis_core::{Atom, Model, Predicate, Sign};
use zetesis_cpu::Cancellation;
use zetesis_engine::Solver;
use zetesis_solve::{
    Backend, PreparedInput, QueryAnswer, Session, SessionBuilder, SolveConfig, WorldViewVerdict,
};
use zetesis_themelios::{
    AdmittedFormula, FormulaPurpose, ProgramFormulaOptions, prepare_program_formula_with,
};

use super::support::name;

/// The program admitted natively for answer-set enumeration.
fn native(program: &Program) -> AdmittedFormula {
    prepare_program_formula_with(
        Arc::new(program.clone()),
        ProgramFormulaOptions {
            purpose: FormulaPurpose::AnswerSets,
            ..ProgramFormulaOptions::default()
        },
    )
    .unwrap()
    .ground()
    .unwrap()
}

fn request(owner: &AdmittedFormula) -> SessionBuilder<'_> {
    Session::builder(
        PreparedInput::formula(owner),
        SolveConfig {
            backend: Backend::Cpu,
            models: 0,
            workers: NonZeroUsize::MIN,
            ..Default::default()
        },
        Cancellation::default(),
    )
}

fn literal(text: &str, sign: Sign) -> Atom {
    Atom::new(Predicate::with_sign(text, 0, sign).unwrap(), vec![]).unwrap()
}

/// A native consequence set over nullary atoms, as the agent's symbols.
fn symbols(model: &Model) -> AnswerSet {
    model
        .atoms()
        .iter()
        .map(|atom| {
            assert!(atom.values().is_empty());
            let sign = match atom.predicate().sign() {
                Sign::Positive => SourceSign::Positive,
                Sign::Negative => SourceSign::Negative,
            };
            Symbol::function(name(atom.predicate().name()), [], sign)
        })
        .collect()
}

/// Programs with nonempty and empty families, with and without strong
/// negation and display directives.
fn programs() -> [Program; 6] {
    [
        program! { { a; b }. },
        program! { a. { b }. },
        program! { -c. { b }. },
        program! { a :- not b. b :- not a. -a :- b. },
        program! { a. -c. b :- not c. #show. },
        program! { :- . },
    ]
}

/// Whether the agent refused the reading because the program has no answer set.
fn no_answer_set<T: std::fmt::Debug>(reading: Result<T, Fault>) -> bool {
    matches!(
        reading.unwrap_err().refused(),
        Refused::Request(Presupposition::NoAnswerSet)
    )
}

#[test]
fn three_valued_answers_agree_with_the_agent() {
    // Every reading occurs, so agreement is not vacuous in any of them.
    let mut seen = HashSet::new();
    for program in programs() {
        let owner = native(&program);
        let mut agent = Agent::new(program, Solver::default());
        // Each literal as the native session and the agent each spell it.
        let queries = [
            (literal("a", Sign::Positive), atom!(a)),
            (literal("a", Sign::Negative), atom!(-a)),
            (literal("b", Sign::Positive), atom!(b)),
            (literal("c", Sign::Positive), atom!(c)),
            (literal("c", Sign::Negative), atom!(-c)),
        ];
        for (native, source) in queries {
            let decision = request(&owner).answer(&native).unwrap();
            let reading = agent.answer(&Query::of(source).unwrap());
            match decision.verdict().value() {
                Some(value) => {
                    let expected = match reading.unwrap() {
                        Answer::Yes => QueryAnswer::Yes,
                        Answer::No => QueryAnswer::No,
                        Answer::Unknown => QueryAnswer::Unknown,
                    };
                    assert_eq!(*value, expected, "{native:?}");
                }
                None => assert!(no_answer_set(reading)),
            }
            seen.insert(decision.verdict().value().copied());
        }
    }
    assert_eq!(seen.len(), 4);
}

#[test]
fn cautious_consequences_agree_with_the_agent() {
    for program in programs() {
        let owner = native(&program);
        let mut agent = Agent::new(program, Solver::default());
        let decision = request(&owner).cautious().unwrap();
        match decision.verdict() {
            WorldViewVerdict::Inconsistent => assert!(no_answer_set(agent.cautious())),
            verdict => assert_eq!(
                &symbols(verdict.value().unwrap()),
                agent.cautious().unwrap().as_set()
            ),
        }
    }
}

#[test]
fn brave_consequences_agree_with_the_agent() {
    for program in programs() {
        let owner = native(&program);
        let mut agent = Agent::new(program, Solver::default());
        let decision = request(&owner).brave().unwrap();
        match decision.verdict() {
            WorldViewVerdict::Inconsistent => assert!(no_answer_set(agent.brave())),
            verdict => assert_eq!(
                &symbols(verdict.value().unwrap()),
                agent.brave().unwrap().as_set()
            ),
        }
    }
}
