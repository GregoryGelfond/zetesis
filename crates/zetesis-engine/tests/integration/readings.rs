//! The upstream agent and epistemic readings run on the real native engine.

use super::support::{constant, name};
use std::collections::BTreeSet;
use themelios_macros::{atom, fact, program};
use themelios_program::{AnswerSet, Sign, Symbol};
use themelios_query::{AgentReading, Answer, Query};
use themelios_solve::{
    agent::Agent,
    contract::{Presupposition, Refused},
};
use zetesis_engine::Solver;

#[test]
fn agent_queries_read_whole_answers_despite_empty_display() {
    let mut agent = Agent::new(
        program! {
            known.
            -denied.
            p :- not q.
            q :- not p.
            #show.
        },
        Solver::default(),
    );
    assert_eq!(
        agent.answer(&Query::of(atom!(known)).unwrap()).unwrap(),
        Answer::Yes
    );
    assert_eq!(
        agent.answer(&Query::of(atom!(p)).unwrap()).unwrap(),
        Answer::Unknown
    );
    assert_eq!(
        agent.answer(&Query::of(atom!(denied)).unwrap()).unwrap(),
        Answer::No
    );
    let denied = Symbol::function(name("denied"), [], Sign::Negative);
    assert_eq!(
        agent.cautious().unwrap().as_set(),
        &AnswerSet::from([constant("known"), denied.clone()])
    );
    assert_eq!(
        agent.brave().unwrap().as_set(),
        &AnswerSet::from([constant("known"), constant("p"), constant("q"), denied])
    );
}

#[test]
fn retraction_restores_the_complete_answer_family() {
    let mut agent = Agent::new(program! { p :- not q. q :- not p. }, Solver::default());
    let expected = BTreeSet::from([
        AnswerSet::from([constant("p")]),
        AnswerSet::from([constant("q")]),
    ]);
    let asserted = agent.assert(fact!(p)).unwrap();
    let after = agent.snapshot().unwrap();
    assert_eq!(
        after
            .members()
            .map(|model| model.atoms().clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([AnswerSet::from([constant("p")])])
    );
    agent.retract(asserted).unwrap();
    assert_eq!(
        agent
            .snapshot()
            .unwrap()
            .members()
            .map(|model| model.atoms().clone())
            .collect::<BTreeSet<_>>(),
        expected
    );
}

#[test]
fn detached_snapshots_survive_knowledge_replacement() {
    let mut agent = Agent::new(program! { p :- not q. q :- not p. }, Solver::default());
    let before = agent.snapshot().unwrap();
    agent.assert(fact!(p)).unwrap();
    let after = agent.snapshot().unwrap();
    assert_eq!(
        after
            .members()
            .map(|model| model.atoms().clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([AnswerSet::from([constant("p")])])
    );
    assert_eq!(
        before
            .members()
            .map(|model| model.atoms().clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            AnswerSet::from([constant("p")]),
            AnswerSet::from([constant("q")]),
        ])
    );
}

#[test]
fn inconsistent_knowledge_refuses_a_snapshot() {
    let mut agent = Agent::new(program! { :- . }, Solver::default());
    let fault = agent.snapshot().unwrap_err();
    assert!(matches!(
        fault.refused(),
        Refused::Request(Presupposition::NoAnswerSet)
    ));
}
