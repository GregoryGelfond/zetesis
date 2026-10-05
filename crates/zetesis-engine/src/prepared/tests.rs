//! The configured grounder selects the claimed native preparation and execution.

use std::{convert::Infallible, num::NonZeroUsize, sync::Arc};

use themelios_macros::program;
use themelios_program::program::Program;
use zetesis_cpu::Cancellation;
use zetesis_solve::{ExecutionObservation, ExecutionObserver, PreparedProfile, Session};

use super::Prepared;
use crate::{Config, Grounder};

#[derive(Default)]
struct Route {
    lazy: bool,
    hybrid_constraints: usize,
    terminal_definitions: usize,
}

impl ExecutionObserver for Route {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::CpuClosure {
                grounder: zetesis_solve::Grounder::Lazy,
                ..
            } => {
                self.lazy = true;
            }
            ExecutionObservation::HybridGrounding {
                streamed_templates, ..
            } => {
                self.hybrid_constraints = streamed_templates;
            }
            ExecutionObservation::TerminalDefinitions {
                deferred_templates, ..
            } => {
                self.terminal_definitions = deferred_templates;
            }
            _ => {}
        }
        Ok(())
    }
}

fn execute(program: Program, grounder: Grounder) -> (PreparedProfile, Route) {
    let config = Config {
        grounder,
        workers: NonZeroUsize::MIN,
        ..Config::default()
    };
    let token = Cancellation::default();
    let prepared = Prepared::new(Arc::new(program), &config, &token).unwrap();
    let profile = prepared.input().profile();
    let mut route = Route::default();
    let session =
        Session::enumerate_observed(prepared.input(), config.session(), token, &mut route).unwrap();
    for answer in session {
        assert!(answer.is_ok());
    }
    (profile, route)
}

#[test]
fn lazy_configuration_uses_source_closure() {
    let (profile, route) = execute(program! { { p }. }, Grounder::Lazy);
    assert_eq!(profile, PreparedProfile::Relational);
    assert!(route.lazy);
}

#[test]
fn hybrid_configuration_streams_constraints() {
    let (profile, route) = execute(program! { { p; q }. :- p, q. }, Grounder::Hybrid);
    assert_eq!(profile, PreparedProfile::Hybrid);
    assert!(route.hybrid_constraints > 0);
}

#[test]
fn eager_configuration_materializes_the_full_theory() {
    let (profile, _) = execute(program! { p(1..2). q(X) :- p(X). }, Grounder::Eager);
    assert_eq!(profile, PreparedProfile::Formula);
}

#[test]
fn automatic_configuration_reconstructs_terminal_definitions() {
    let (profile, route) = execute(program! { p(1..2). q(X) :- p(X). }, Grounder::Auto);
    assert_eq!(profile, PreparedProfile::TerminalDefinitions);
    assert!(route.terminal_definitions > 0);
}
