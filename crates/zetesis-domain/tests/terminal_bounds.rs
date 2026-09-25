//! Inclusive logical ceilings and no partially published structural certificate.

use themelios_program::program::{Atom, Program, Rule};
use themelios_program::symbol::{Name, Sign, Symbol};
use themelios_program::term::Term;
use zetesis_domain::terminal::{self, Status};
use zetesis_domain::{Limits, Resource};

#[path = "terminal/support.rs"]
mod support;
use support::source;

#[test]
fn work_cutoffs_withhold_partial_certificates() {
    let program = source("alpha. beta. receipt(X):-seed(X).");
    let complete = terminal::analyze(&program, Limits::default());
    assert_eq!(complete.status(), Status::Complete);
    assert_eq!(complete.definitions().len(), 3);
    let required = complete.statistics().work;
    for max_work in 0..required {
        let result = terminal::analyze(
            &program,
            Limits {
                max_work,
                ..Limits::default()
            },
        );
        let Status::Stopped(stop) = result.status() else {
            panic!("cutoff {max_work} unexpectedly completed")
        };
        assert_eq!(stop.resource, Resource::Work);
        assert_eq!(stop.limit, u128::from(max_work));
        assert_eq!(stop.observed, u128::from(max_work) + 1);
        assert_eq!(result.statistics().work, max_work);
        assert!(result.definitions().is_empty());
        assert!(result.context().is_some());
    }
}

#[test]
fn exact_work_allowance_completes_classification() {
    let program = source("alpha. beta. receipt(X):-seed(X).");
    let complete = terminal::analyze(&program, Limits::default());
    assert_eq!(complete.status(), Status::Complete);
    let result = terminal::analyze(
        &program,
        Limits {
            max_work: complete.statistics().work,
            ..Limits::default()
        },
    );
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(result.definitions().len(), complete.definitions().len());
    assert!(result.context().is_none());
}

#[test]
fn each_used_population_ceiling_has_a_typed_stop() {
    let program = source("seed(1). receipt(X):-seed(X).");
    for (limits, resource) in [
        (
            Limits {
                max_predicates: 0,
                ..Limits::default()
            },
            Resource::Predicates,
        ),
        (
            Limits {
                max_positions: 0,
                ..Limits::default()
            },
            Resource::Positions,
        ),
        (
            Limits {
                max_links: 0,
                ..Limits::default()
            },
            Resource::Links,
        ),
        (
            Limits {
                max_inspected_bytes: 0,
                ..Limits::default()
            },
            Resource::InspectedBytes,
        ),
    ] {
        let result = terminal::analyze(&program, limits);
        let Status::Stopped(stop) = result.status() else {
            panic!("{resource:?} did not stop")
        };
        assert_eq!(stop.resource, resource);
        assert_eq!(stop.limit, 0);
        assert!(stop.observed > stop.limit);
        assert!(result.definitions().is_empty());
    }
}

#[test]
fn recorded_populations_fit_their_exact_inclusive_limits() {
    let program = source("seed(1). receipt(X):-seed(X). note.");
    let complete = terminal::analyze(&program, Limits::default());
    let stats = complete.statistics();
    let result = terminal::analyze(
        &program,
        Limits {
            max_work: stats.work,
            max_predicates: stats.predicates,
            max_positions: stats.positions,
            max_links: stats.links,
            max_inspected_bytes: stats.inspected_bytes,
            ..Limits::default()
        },
    );
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(result.statistics(), stats);
    assert_eq!(result.definitions().len(), 2);
}

#[test]
fn unrelated_domain_limits_do_not_bound_terminal_scan() {
    let program = source("receipt(1).");
    let result = terminal::analyze(
        &program,
        Limits {
            max_values_per_argument: 0,
            max_value_entries: 0,
            max_rounds: 0,
            ..Limits::default()
        },
    );
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(result.definitions().len(), 1);
    assert_eq!(result.statistics().value_entries, 0);
    assert_eq!(result.statistics().rounds, 0);
}

fn closed(symbol: Symbol) -> Program {
    Program::of([Rule::fact(Atom::new(
        Name::new("receipt").unwrap(),
        [Term::Symbolic(symbol)],
    ))])
}

#[test]
fn closed_symbol_boundaries_disqualify_locally_without_evaluating_source() {
    let program = closed(Symbol::Function {
        name: Name::new("parcel").unwrap(),
        arguments: vec![Symbol::Number(7), Symbol::String("fragile".into())],
        sign: Sign::Positive,
    });
    for limits in [
        Limits {
            max_symbol_nodes: 2,
            ..Limits::default()
        },
        Limits {
            max_symbol_depth: 1,
            ..Limits::default()
        },
        Limits {
            max_symbol_bytes: 12,
            ..Limits::default()
        },
    ] {
        let result = terminal::analyze(&program, limits);
        assert_eq!(result.status(), Status::Complete);
        assert!(result.definitions().is_empty());
    }
    let result = terminal::analyze(
        &program,
        Limits {
            max_symbol_nodes: 3,
            max_symbol_depth: 2,
            max_symbol_bytes: 13,
            ..Limits::default()
        },
    );
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(result.definitions().len(), 1);
}

#[test]
fn deep_closed_values_use_bounded_iterative_inspection() {
    let mut symbol = Symbol::Number(1);
    for _ in 0..4096 {
        symbol = Symbol::Tuple(vec![symbol]);
    }
    let program = closed(symbol);
    let result = terminal::analyze(
        &program,
        Limits {
            max_symbol_depth: 8,
            ..Limits::default()
        },
    );
    assert_eq!(result.status(), Status::Complete);
    assert!(result.definitions().is_empty());
    assert!(
        result.statistics().work < 40,
        "no traversal of the unneeded deep suffix"
    );
}
