//! Constructed programs use the same owner for preparation and relational solves.

use std::{collections::BTreeSet, num::NonZeroUsize, sync::Arc};

use zetesis_cpu::Cancellation;
use zetesis_reference_support::canonical;
use zetesis_solve::{
    Backend, ClosureRoute, Completion, Grounder, PreparedInput, PreparedProfile, Session,
    SolveConfig, Subject,
};
use zetesis_themelios::logical::{
    Name, Symbol, Term,
    program::{
        Atom, Body, Choice, ChoiceElement, Condition, Direction, Optimize, OptimizeElement,
        Program, Project, Rule, Show, Statement, weight,
    },
};
use zetesis_themelios::{
    FormulaPurpose, ProgramRelationalOptions, observation, prepare_program_relational,
};

fn input() -> Arc<Program> {
    let atom = |name| Atom::constant(Name::new(name).unwrap());
    let statements: [Statement; 5] = [
        Rule::fact(Choice::new(
            None,
            [ChoiceElement::new(atom("p").into(), Condition::empty())],
            None,
        ))
        .into(),
        Rule::fact(atom("q")).into(),
        Show::Term(7.into()).into(),
        Optimize::new(
            Direction::Minimize,
            [OptimizeElement::new(
                weight(Term::from(1) / Term::from(0)),
                [],
                Condition::empty(),
            )],
        )
        .into(),
        Project::atom_body(
            Atom::new(
                Name::new("selected").unwrap(),
                [Term::from(1) / Term::from(0)],
            ),
            Body::empty(),
        )
        .into(),
    ];
    Arc::new(Program::of(statements))
}

fn complete(grounder: Grounder) -> ClosureRoute {
    let original = input();
    let owner = prepare_program_relational(
        Arc::clone(&original),
        ProgramRelationalOptions {
            purpose: FormulaPurpose::AnswerSets,
            ..ProgramRelationalOptions::default()
        },
    )
    .unwrap();
    assert!(std::ptr::eq(owner.original_program(), original.as_ref()));
    let prepared = PreparedInput::relational(&owner);
    assert_eq!(prepared.profile(), PreparedProfile::Relational);
    assert!(std::ptr::eq(prepared.metadata().unwrap(), owner.metadata()));
    let cancellation = Cancellation::default();
    let mut session = Session::enumerate(
        prepared,
        SolveConfig {
            backend: Backend::Cpu,
            grounder,
            workers: NonZeroUsize::MIN,
            models: 0,
            ..SolveConfig::default()
        },
        cancellation.clone(),
    )
    .unwrap();
    let mut answers = BTreeSet::new();
    for answer in session.by_ref() {
        let answer = answer.unwrap();
        let Subject::Program(subject) = answer.subject() else {
            panic!("the answer retains its original relational subject");
        };
        assert!(subject.same_instance(owner.program()));
        assert!(answer.score().is_none());
        let shown = owner
            .metadata()
            .observations()
            .evaluate(
                answer.interpretation(),
                observation::Limits::default(),
                &cancellation,
            )
            .unwrap();
        assert_eq!(shown.symbols(), &[Symbol::Number(7)]);
        assert!(
            answers.insert(
                answer
                    .interpretation()
                    .atoms()
                    .iter()
                    .map(canonical)
                    .collect::<BTreeSet<_>>()
            )
        );
    }
    assert_eq!(
        answers,
        BTreeSet::from([
            BTreeSet::from(["q".to_owned()]),
            BTreeSet::from(["p".to_owned(), "q".to_owned()]),
        ])
    );
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    outcome.closure_execution().unwrap().route
}

#[test]
fn constructed_answers_complete_through_lazy_closure() {
    assert!(matches!(complete(Grounder::Lazy), ClosureRoute::Lazy(_)));
}

#[test]
fn constructed_answers_complete_through_eager_closure() {
    assert_eq!(complete(Grounder::Eager), ClosureRoute::Eager);
}
