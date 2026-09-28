//! Independent CPU closure checks leave their summed counters in the outcome.

use std::num::NonZeroUsize;

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Template};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_solve::{
    Backend, ClosureRoute, Completion, Grounder, Interruption, PreparedInput, SemanticOutcome,
    Session, SolveConfig,
};

/// `{a}. {b}. c :- a, not b.` Its four candidates are all answer sets, and
/// their four closures hold five atoms in total: {}, {a, c}, {b}, {a, b}.
fn program() -> Program {
    let atom = |name: &str| AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap();
    let choice =
        |name: &str| Template::new(Some(atom(name)), vec![], vec![atom(name)], vec![], vec![]);
    let rule = Template::new(
        Some(atom("c")),
        vec![atom("a")],
        vec![],
        vec![atom("b")],
        vec![],
    );
    Program::new(
        vec![choice("a"), choice("b"), rule],
        AdmissionLimits::default(),
    )
    .unwrap()
}

const CANDIDATES: u64 = 4;
const CLOSURE_ATOMS: u64 = 5;

fn config(grounder: Grounder) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder,
        models: 0,
        workers: NonZeroUsize::new(2).unwrap(),
        ..Default::default()
    }
}

fn run(program: &Program, config: SolveConfig) -> (usize, SemanticOutcome) {
    let mut session = Session::builder(
        PreparedInput::program(program),
        config,
        Cancellation::default(),
    )
    .start()
    .unwrap();
    let mut answers = 0;
    for answer in session.by_ref() {
        answer.unwrap();
        answers += 1;
    }
    (answers, session.outcome().unwrap())
}

#[test]
fn lazy_closure_receipts_sum_every_completed_check() {
    let (answers, outcome) = run(&program(), config(Grounder::Lazy));
    assert_eq!(answers, 4);
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    let closure = outcome.closure_execution().unwrap();
    let ClosureRoute::Lazy(joins) = closure.route else {
        panic!("the lazy route ran: {closure:?}")
    };
    assert_eq!(closure.completed_checks, CANDIDATES);
    assert_eq!(closure.stopped_checks, 0);
    assert_eq!(closure.derived_atoms, CLOSURE_ATOMS);
    assert!(closure.rounds >= CANDIDATES);
    assert!(closure.work > 0);
    assert!(joins.catalog_work <= closure.work);
    assert!(joins.bindings > 0);
    assert!(joins.tuple_probes <= closure.work);
    assert!(joins.peak_closure_bytes > 0);
}

#[test]
fn eager_closure_receipts_carry_no_join_counters() {
    let (answers, outcome) = run(&program(), config(Grounder::Eager));
    assert_eq!(answers, 4);
    let closure = outcome.closure_execution().unwrap();
    assert_eq!(closure.route, ClosureRoute::Eager);
    assert_eq!(closure.completed_checks, CANDIDATES);
    assert_eq!(closure.stopped_checks, 0);
    assert_eq!(closure.derived_atoms, CLOSURE_ATOMS);
    assert!(closure.rounds > 0);
    assert!(closure.work > 0);
}

#[test]
fn a_stopped_check_is_counted_without_its_partial_work() {
    // The oracle returns no counters for a stopped check, so the receipt
    // counts the stop and sums only the checks that completed.
    let (answers, outcome) = run(
        &program(),
        SolveConfig {
            max_work: 1,
            ..config(Grounder::Lazy)
        },
    );
    assert_eq!(answers, 0);
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Oracle(Stop::WorkLimit))
    );
    let closure = outcome.closure_execution().unwrap();
    assert!(closure.stopped_checks >= 1);
    assert!(closure.work <= closure.completed_checks);
}

#[test]
fn routes_without_independent_closure_checks_expose_no_receipt() {
    let (_, outcome) = run(
        &program(),
        SolveConfig {
            source_batching: zetesis_solve::SourceBatching::Union,
            ..config(Grounder::Lazy)
        },
    );
    assert!(outcome.shared_execution().is_some());
    assert!(outcome.closure_execution().is_none());
}
