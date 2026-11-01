//! The configured grounder selects the claimed native preparation and execution.

use std::{convert::Infallible, num::NonZeroUsize, sync::Arc};

use themelios_macros::program;
use themelios_program::program::Program;
use zetesis_algebra::Cancellation;
use zetesis_solve::{ExecutionObservation, ExecutionObserver, PreparedProfile, Session};

use super::Prepared;
use crate::{Config, Grounder};

#[derive(Default)]
struct Route {
    source: bool,
    hybrid_constraints: usize,
    hybrid_retained: usize,
    terminal_definitions: usize,
    terminal_base: Option<zetesis_themelios::BaseKind>,
    terminal_streamed: Option<usize>,
    terminal_retained: Option<usize>,
}

impl ExecutionObserver for Route {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::SourceEvaluation => self.source = true,
            ExecutionObservation::HybridGrounding { streamed, .. } => {
                self.hybrid_constraints = streamed.templates;
                self.hybrid_retained = streamed.retained_templates;
            }
            ExecutionObservation::TerminalDefinitions {
                deferred_templates,
                base,
                streamed,
                ..
            } => {
                self.terminal_definitions = deferred_templates;
                self.terminal_base = Some(base);
                self.terminal_streamed = streamed.map(|streamed| streamed.templates);
                self.terminal_retained = streamed.map(|streamed| streamed.retained_templates);
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
fn lazy_formula_configuration_grounds_a_fitting_constraint() {
    // Lazy grounding takes the formula route over a producer core. The
    // constraint's one instance fits the formula's share of the ordinary memory
    // allowance, so it is grounded in the core and nothing is streamed. The
    // double negation keeps pair capture from claiming `:- p, q.` first.
    let (profile, route) = execute(program! { { p; q }. :- p, not not q. }, Grounder::Lazy);
    assert_eq!(profile, PreparedProfile::Hybrid);
    assert_eq!(route.hybrid_constraints, 0);
    assert_eq!(route.hybrid_retained, 1);
}

#[test]
fn lazy_formula_configuration_defers_terminal_definitions() {
    // r/1 is read by nothing: the lazy materialization defers it over a
    // hybrid base, which grounds the fitting constraint as above.
    let (profile, route) = execute(
        program! { { s(1); s(2) }. r(X) :- s(X). :- s(1), not not s(2). },
        Grounder::Lazy,
    );
    assert_eq!(profile, PreparedProfile::TerminalDefinitions);
    assert_eq!(
        route.terminal_base,
        Some(zetesis_themelios::BaseKind::Hybrid)
    );
    assert_eq!(route.terminal_streamed, Some(0));
    assert_eq!(route.terminal_retained, Some(1));
    assert!(route.terminal_definitions > 0);
}

#[test]
fn eager_configuration_materializes_the_full_theory() {
    let (profile, _) = execute(program! { p(1..2). q(X) :- p(X). }, Grounder::Eager);
    assert_eq!(profile, PreparedProfile::Formula);
}

#[test]
fn automatic_configuration_evaluates_an_acyclic_source() {
    let (profile, route) = execute(program! { p(1..2). q(X) :- p(X). }, Grounder::Auto);
    assert_eq!(profile, PreparedProfile::AcyclicSource);
    assert!(route.source);
    assert_eq!(route.terminal_definitions, 0);
}
