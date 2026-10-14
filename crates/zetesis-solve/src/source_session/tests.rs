//! An exact source receipt composes with ordinary session boundaries.

use std::{convert::Infallible, num::NonZeroUsize};
use zetesis_reference_support::prepared;

use zetesis_cpu::{Cancellation, Stop};

use crate::{
    Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder, Interruption,
    ModelConstructionStop, Oracle, PreparedExecution, PreparedProfile, Session, SolveConfig,
    WorldViewLimits, prepare_formula_execution,
};

const SIGNED: &str = include_str!("../../tests/fixtures/acyclic-source/signed.lp");

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        workers: NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    }
}

fn admitted(source: &str) -> PreparedExecution {
    let input = prepare_formula_execution(prepared(source), &config(), None).unwrap();
    assert_eq!(input.input().profile(), PreparedProfile::AcyclicSource);
    input
}

#[derive(Default)]
struct Route {
    source: usize,
    formulas: usize,
}

impl ExecutionObserver for Route {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::SourceEvaluation => self.source += 1,
            ExecutionObservation::Formula { .. } => self.formulas += 1,
            _ => {}
        }
        Ok(())
    }
}

#[test]
fn source_session_preserves_the_complete_original_answer() {
    let owner = admitted(SIGNED);
    let mut route = Route::default();
    let mut session =
        Session::enumerate_observed(owner.input(), config(), Cancellation::default(), &mut route)
            .unwrap();
    let answer = session.next().unwrap().unwrap();
    assert!(session.next().is_none());
    assert_eq!((route.source, route.formulas), (1, 0));
    let full = prepared(SIGNED).ground().unwrap();
    let mut ordinary = Session::enumerate(
        crate::PreparedInput::formula(&full),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    let expected = ordinary.next().unwrap().unwrap();
    assert!(ordinary.next().is_none());
    assert_eq!(answer.interpretation(), expected.interpretation());
    assert_eq!(answer.interpretation().atoms().len(), 7);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(outcome.source_evaluation().is_some());
    assert!(outcome.countermodel_statistics().is_none());
    assert!(outcome.subject().unwrap().same_instance(answer.subject()));
}

#[test]
fn known_uniqueness_survives_a_one_answer_request() {
    let owner = admitted(SIGNED);
    let mut one = config();
    one.models = 1;
    let mut session = Session::new(owner.input(), one, Cancellation::default()).unwrap();
    assert!(session.next().unwrap().is_ok());
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
}

#[test]
fn complete_source_rejection_establishes_inconsistency() {
    for source in [
        include_str!("../../tests/fixtures/acyclic-source/inconsistent.lp"),
        include_str!("../../tests/fixtures/acyclic-source/coherence.lp"),
    ] {
        let owner = admitted(source);
        let mut session = Session::new(owner.input(), config(), Cancellation::default()).unwrap();
        assert!(session.next().is_none());
        let outcome = session.outcome().unwrap();
        assert!(outcome.unsatisfiable());
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(outcome.verified_models(), 0);
    }
}

#[test]
fn an_empty_interpretation_is_a_complete_singleton_family() {
    let owner = admitted(include_str!("../../tests/fixtures/acyclic-source/empty.lp"));
    let family = Session::builder(owner.input(), config(), Cancellation::default())
        .collect(WorldViewLimits::default())
        .unwrap();
    assert_eq!(family.len(), 1);
}

#[test]
fn sessions_reuse_the_same_certified_source_subject() {
    let owner = admitted(SIGNED);
    let mut first = Session::new(owner.input(), config(), Cancellation::default()).unwrap();
    let mut second = Session::new(owner.input(), config(), Cancellation::default()).unwrap();
    let a = first.next().unwrap().unwrap();
    let b = second.next().unwrap().unwrap();
    assert!(a.subject().same_instance(b.subject()));
    assert_eq!(a.interpretation(), b.interpretation());
    assert_eq!(
        first.outcome().unwrap().source_evaluation(),
        second.outcome().unwrap().source_evaluation()
    );
}

#[test]
fn cancelled_source_publication_does_not_claim_exhaustion() {
    let owner = admitted(SIGNED);
    let cancellation = Cancellation::default();
    let mut session = Session::new(owner.input(), config(), cancellation.clone()).unwrap();
    cancellation.cancel();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Preparation(Stop::Cancelled))
    );
    assert!(!outcome.unsatisfiable());
    assert!(session.next().is_none());
}

#[test]
fn model_allowances_remain_per_session() {
    let owner = admitted(SIGNED);
    let mut limited = config();
    limited.max_model_work = 0;
    let mut session = Session::new(owner.input(), limited, Cancellation::default()).unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::ModelConstruction(
            ModelConstructionStop::Work { limit: 0, .. }
        ))
    ));
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.model_construction().unwrap().constructed, 0);
    let mut fresh = Session::new(owner.input(), config(), Cancellation::default()).unwrap();
    assert!(fresh.next().unwrap().is_ok());
}

#[test]
fn explicit_materialization_requests_do_not_select_source_evaluation() {
    for grounder in [Grounder::Eager, Grounder::Lazy] {
        let requested = SolveConfig {
            grounder,
            ..config()
        };
        let owner = prepare_formula_execution(prepared(SIGNED), &requested, None).unwrap();
        assert_ne!(owner.input().profile(), PreparedProfile::AcyclicSource);
    }
}

#[test]
fn explicit_oracle_requests_do_not_select_source_evaluation() {
    let requested = SolveConfig {
        oracle: Oracle::Countermodel,
        ..config()
    };
    let owner = prepare_formula_execution(prepared(SIGNED), &requested, None).unwrap();
    assert_ne!(owner.input().profile(), PreparedProfile::AcyclicSource);
}

#[test]
fn stopping_before_publication_does_not_claim_exhaustion() {
    let owner = admitted(SIGNED);
    let session = Session::new(owner.input(), config(), Cancellation::default()).unwrap();
    let outcome = session.stop();
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.verified_models(), 0);
    assert!(!outcome.unsatisfiable());
}

#[test]
fn source_observer_failure_preserves_its_external_type() {
    struct Refuse;
    impl ExecutionObserver for Refuse {
        type Error = std::io::Error;

        fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
            assert!(matches!(event, ExecutionObservation::SourceEvaluation));
            Err(std::io::Error::other("source observer refused"))
        }
    }
    let owner = admitted(SIGNED);
    let result = Session::new_observed(
        owner.input(),
        config(),
        Cancellation::default(),
        &mut Refuse,
    );
    let Err(failure) = result else {
        panic!("observer must refuse setup")
    };
    assert!(
        matches!(failure.cause.as_ref(), crate::SolveError::ExecutionObservation(error)
        if error.downcast_ref::<std::io::Error>().is_some())
    );
}
