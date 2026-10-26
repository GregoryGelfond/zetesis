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
    source: bool,
    lazy: bool,
    hybrid_constraints: usize,
    terminal_definitions: usize,
    terminal_base: Option<zetesis_themelios::BaseKind>,
    terminal_streamed: Option<usize>,
}

impl ExecutionObserver for Route {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::SourceEvaluation => self.source = true,
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
                deferred_templates,
                base,
                streamed,
                ..
            } => {
                self.terminal_definitions = deferred_templates;
                self.terminal_base = Some(base);
                self.terminal_streamed = streamed.map(|streamed| streamed.templates);
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
fn lazy_formula_configuration_streams_constraints() {
    // A choice rule is outside the relational profile: lazy grounding takes
    // the formula route, streaming the constraint over a producer core. The
    // double negation keeps the constraint a stream: pair capture would move
    // `:- p, q.` into the core.
    let (profile, route) = execute(program! { { p; q }. :- p, not not q. }, Grounder::Lazy);
    assert_eq!(profile, PreparedProfile::Hybrid);
    assert!(route.hybrid_constraints > 0);
}

#[test]
fn lazy_formula_configuration_defers_terminal_definitions() {
    // r/1 is read by nothing: the lazy materialization defers it over a
    // hybrid base that streams the constraint, kept a stream by its double
    // negation as above.
    let (profile, route) = execute(
        program! { { s(1); s(2) }. r(X) :- s(X). :- s(1), not not s(2). },
        Grounder::Lazy,
    );
    assert_eq!(profile, PreparedProfile::TerminalDefinitions);
    assert_eq!(
        route.terminal_base,
        Some(zetesis_themelios::BaseKind::Hybrid)
    );
    assert_eq!(route.terminal_streamed, Some(1));
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
