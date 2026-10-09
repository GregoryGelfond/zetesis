//! Lazy grounding defers terminal definitions over a hybrid base: a core answer
//! is checked against the streamed constraints, then extended, then published.

use std::{collections::BTreeSet, num::NonZeroUsize};
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    Backend, Completion, Grounder, Oracle, PreparedInput, Session, SolveConfig, SolveError,
    WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, BaseKind, ConstraintCheckLimits, ExpansionLimits, FormulaFailure,
    FormulaLimits, FormulaMaterialization, HybridFormula, TerminalBase, TerminalFormula,
    prepare_formula,
};

fn lazy(source: &str) -> Result<FormulaMaterialization<HybridFormula>, FormulaFailure> {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_lazy()
}

fn lazy_terminal(source: &str) -> TerminalFormula {
    let FormulaMaterialization::Terminal(owner) = lazy(source).unwrap() else {
        panic!("fixture requires a certified terminal partition");
    };
    owner
}

fn adaptive_terminal(source: &str) -> TerminalFormula {
    let FormulaMaterialization::Terminal(owner) = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_adaptive()
    .unwrap() else {
        panic!("fixture requires a certified terminal partition");
    };
    owner
}

fn config(grounder: Grounder) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder,
        oracle: Oracle::Auto,
        models: 0,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        ..SolveConfig::default()
    }
}

fn family(
    input: PreparedInput<'_>,
    config: SolveConfig,
) -> (BTreeSet<Model>, zetesis_solve::WorldView) {
    let view = Session::builder(input, config, Cancellation::default())
        .collect(WorldViewLimits::default())
        .unwrap();
    let family = view
        .answer_sets()
        .iter()
        .map(|answer| answer.interpretation().clone())
        .collect();
    (family, view)
}

/// Whether a collection failed with a setup or execution error satisfying `is`.
fn refused(failure: &zetesis_solve::WorldViewFailure, is: impl Fn(&SolveError) -> bool) -> bool {
    matches!(failure.cause(), zetesis_solve::WorldViewError::Solve(solve) if is(&solve.cause))
}

const OPTIONAL: &str = "{seed(1);seed(2)}. receipt(X):-seed(X). :- seed(1), seed(2).";

#[test]
fn lazy_grounding_defers_terminal_definitions_over_a_hybrid_base() {
    let owner = lazy_terminal(OPTIONAL);
    assert_eq!(owner.base_kind(), BaseKind::Hybrid);
    assert!(owner.base_theory().is_none());
    assert!(matches!(owner.base(), TerminalBase::Hybrid(core) if core.streamed_templates() == 1));
    let (actual, view) = family(PreparedInput::terminal(&owner), config(Grounder::Lazy));
    assert_eq!(view.outcome().completion(), Some(Completion::Exhausted));
    let reference = adaptive_terminal(OPTIONAL);
    let (expected, _) = family(PreparedInput::terminal(&reference), config(Grounder::Auto));
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 3);
}

#[test]
fn a_rejected_core_answer_is_never_extended() {
    // The core answers {b} and {a, b} violate `:- b.` and would extend by t/1;
    // only the two accepted ones are reconstructed. Clause search proposes
    // every core answer, so each one reaches the full check.
    let owner = lazy_terminal("{a; b}. d(1..2). t(X) :- b, d(X). :- b.");
    let config = SolveConfig {
        search: zetesis_solve::SearchMethod::Clauses,
        ..config(Grounder::Lazy)
    };
    let (actual, view) = family(PreparedInput::terminal(&owner), config);
    assert_eq!(actual.len(), 2);
    assert!(actual.iter().all(|model| {
        model
            .atoms()
            .iter()
            .all(|atom| atom.predicate().name() != "t")
    }));
    let outcome = view.outcome();
    assert_eq!(
        outcome
            .terminal_execution()
            .unwrap()
            .reconstruction
            .attempts,
        2
    );
    let hybrid = outcome.hybrid_execution().unwrap();
    assert_eq!((hybrid.core_answers, hybrid.rejected), (4, 2));
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
}

#[test]
fn an_empty_stream_still_gives_a_hybrid_base() {
    const SOURCE: &str = "d(1..2). {b}. t(X) :- b, d(X).";
    let owner = lazy_terminal(SOURCE);
    assert_eq!(owner.base_kind(), BaseKind::Hybrid);
    let (actual, _) = family(PreparedInput::terminal(&owner), config(Grounder::Lazy));
    let reference = adaptive_terminal(SOURCE);
    let (expected, _) = family(PreparedInput::terminal(&reference), config(Grounder::Auto));
    assert_eq!(actual, expected);
}

#[test]
fn an_eager_request_is_refused_by_a_hybrid_terminal_base() {
    let owner = lazy_terminal(OPTIONAL);
    assert!(matches!(
        Session::builder(
            PreparedInput::terminal(&owner),
            config(Grounder::Eager),
            Cancellation::default()
        )
        .collect(WorldViewLimits::default()),
        Err(failure) if refused(&failure, |error| matches!(error, SolveError::PreparedInput { .. }))
    ));
}

#[test]
fn an_automatic_request_runs_a_hybrid_terminal_base() {
    let owner = lazy_terminal(OPTIONAL);
    let (actual, view) = family(PreparedInput::terminal(&owner), config(Grounder::Auto));
    assert_eq!(view.outcome().completion(), Some(Completion::Exhausted));
    assert_eq!(actual.len(), 3);
}

#[test]
fn a_hybrid_terminal_base_runs_on_the_cpu_only() {
    let owner = lazy_terminal(OPTIONAL);
    let config = SolveConfig {
        backend: Backend::Gpu(None),
        ..config(Grounder::Lazy)
    };
    assert!(matches!(
        Session::builder(
            PreparedInput::terminal(&owner),
            config,
            Cancellation::default()
        )
        .collect(WorldViewLimits::default()),
        Err(failure) if refused(&failure, |error| matches!(error, SolveError::HybridBackend { .. }))
    ));
}

#[test]
fn objectives_under_lazy_grounding_are_materialized() {
    let FormulaMaterialization::Complete(owner) = lazy("{a}. #minimize{1:a}.").unwrap() else {
        panic!("objectives must retain complete producers");
    };
    assert!(owner.objectives().is_present());
}

#[test]
fn an_explicit_projection_takes_the_hybrid_route_without_extension() {
    assert!(matches!(
        lazy("{seed(1);seed(2)}. receipt(X):-seed(X). #project seed/1."),
        Ok(FormulaMaterialization::Complete(_))
    ));
}

#[test]
fn a_stopped_constraint_check_never_claims_exhaustion() {
    let owner = lazy_terminal(OPTIONAL);
    let config = SolveConfig {
        constraints: ConstraintCheckLimits {
            max_work: 1,
            ..ConstraintCheckLimits::default()
        },
        ..config(Grounder::Lazy)
    };
    let result = Session::builder(
        PreparedInput::terminal(&owner),
        config,
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default());
    match result {
        Ok(view) => assert_ne!(view.outcome().completion(), Some(Completion::Exhausted)),
        Err(failure) => assert!(
            refused(&failure, |error| matches!(error, SolveError::Constraint(_))),
            "{failure:?}"
        ),
    }
}

#[test]
fn a_streamed_check_constructs_terms_over_the_closed_base() {
    // The constraint's check builds f(X) for each p(X), resolving it through
    // the closed base's terms.
    const SOURCE: &str =
        "{p(1); p(2)}. {q(f(1))}. r(X) :- p(X), seed(X). seed(1..2). :- p(X), q(f(X)).";
    let owner = lazy_terminal(SOURCE);
    let (actual, _) = family(PreparedInput::terminal(&owner), config(Grounder::Lazy));
    let reference = adaptive_terminal(SOURCE);
    let (expected, _) = family(PreparedInput::terminal(&reference), config(Grounder::Auto));
    assert_eq!(actual, expected);
}
