//! Request budgets and configured engine ceilings remain distinct outcomes.

use super::support::{constant, family};
use std::{collections::BTreeSet, time::Duration};
use themelios_macros::program;
use themelios_program::AnswerSet;
use themelios_solve::{
    bridge::Door,
    contract::{Backend, Locus, SolveRequest},
    outcome::Conclusion,
};
use zetesis_engine::{Config, Grounder, Solver};

#[test]
fn zero_request_budgets_do_not_poison_later_questions() {
    let program = program! { p :- not q. q :- not p. };
    for grounder in [Grounder::Auto, Grounder::Eager, Grounder::Lazy] {
        let mut solver = Solver::new(Config {
            grounder,
            ..Config::default()
        });
        solver.lower(Door::Program(&program)).unwrap();
        let mut request = SolveRequest::default();
        request.time = Some(Duration::ZERO);
        {
            let mut run = solver.solve(&request).unwrap();
            assert!(run.models().next().is_none(), "{grounder:?}");
            assert_eq!(run.conclusion(), Some(Conclusion::Budget), "{grounder:?}");
        }
        assert_eq!(
            family(&mut solver),
            BTreeSet::from([
                AnswerSet::from([constant("p")]),
                AnswerSet::from([constant("q")]),
            ]),
            "{grounder:?}"
        );
    }
}

#[test]
fn configured_memory_refusals_are_repeatable() {
    let mut solver = Solver::new(Config {
        memory: 0,
        ..Config::default()
    });
    for _ in 0..2 {
        let fault = solver
            .lower(Door::Program(&program! { fact. }))
            .unwrap_err();
        assert_eq!(fault.locus(), Locus::Resource);
    }
}

#[test]
fn cancellation_reaches_a_pending_budget_run() {
    let mut solver = Solver::default();
    solver.lower(Door::Program(&program! { fact. })).unwrap();
    let interrupt = solver.interrupt().expect("cancellation is declared");
    let mut request = SolveRequest::default();
    request.time = Some(Duration::ZERO);
    let mut run = solver.solve(&request).unwrap();
    // Preparation has stopped, but the returned run still owns its active
    // cancellation window until its first pull acknowledges the end.
    assert!(run.conclusion().is_none());
    interrupt.cancel();
    assert!(run.models().next().is_none());
    assert_eq!(run.conclusion(), Some(Conclusion::Interrupted));
}
