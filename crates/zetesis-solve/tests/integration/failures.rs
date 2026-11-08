//! Typed failure decomposition preserves input identity and established evidence.

use std::{error::Error as _, fmt, sync::Arc};
use zetesis_reference_support::formula;

use zetesis_algebra::Cancellation;
use zetesis_solve::{
    Backend, ExecutionObservation, ExecutionObserver, Original, PreparedInput, Session,
    SolveConfig, SolveError, SolveFailure,
};

#[derive(Debug)]
struct Refusal(Arc<()>);

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("host observation refused")
    }
}

impl std::error::Error for Refusal {}

struct Refuse(Arc<()>);

impl ExecutionObserver for Refuse {
    type Error = Refusal;

    fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        Err(Refusal(Arc::clone(&self.0)))
    }
}

/// Accepts formula setup and refuses the execution route's own observation,
/// so the failure belongs to execution setup, which `start` returns at once.
struct RefuseRoute(Arc<()>);

impl ExecutionObserver for RefuseRoute {
    type Error = Refusal;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match observation {
            ExecutionObservation::CpuFormula { .. } => Err(Refusal(Arc::clone(&self.0))),
            _ => Ok(()),
        }
    }
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        stats: true,
        ..Default::default()
    }
}

fn decompose(
    failure: SolveFailure,
    expected_original: &Original,
    token: &Arc<()>,
) -> zetesis_solve::FailureParts {
    assert!(failure.original().unwrap().same_instance(expected_original));
    assert_eq!(
        failure.to_string(),
        "execution observer: host observation refused"
    );
    let linked = failure
        .source()
        .unwrap()
        .downcast_ref::<SolveError>()
        .unwrap();
    assert!(std::ptr::eq(linked, failure.cause.as_ref()));
    let original = linked.source().unwrap().downcast_ref::<Refusal>().unwrap();
    assert!(Arc::ptr_eq(&original.0, token));
    assert!(original.source().is_none());
    let cause = std::ptr::from_ref(failure.cause.as_ref());
    let semantic = failure.semantic().map(std::ptr::from_ref);
    let phases = failure.phase_timings.as_deref().copied();
    let parts = failure.into_parts();
    assert!(
        parts
            .original
            .as_ref()
            .unwrap()
            .same_instance(expected_original)
    );
    assert!(std::ptr::eq(parts.cause.as_ref(), cause));
    assert_eq!(parts.semantic.as_deref().map(std::ptr::from_ref), semantic);
    assert_eq!(parts.phase_timings.as_deref().copied(), phases);
    let SolveError::ExecutionObservation(cause) = parts.cause.as_ref() else {
        panic!("the original observer cause must retain its typed boundary");
    };
    let refusal = cause.downcast_ref::<Refusal>().unwrap();
    assert!(Arc::ptr_eq(&refusal.0, token));
    parts
}

#[test]
fn setup_failure_decomposition_preserves_evidence() {
    let owner = zetesis_reference_support::formula("a.");
    let token = Arc::new(());
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .start_observed(&mut RefuseRoute(Arc::clone(&token)))
    .err()
    .unwrap();
    let parts = decompose(failure, &Original::Theory(owner.theory().clone()), &token);
    assert!(parts.semantic.is_none());
    assert!(parts.phase_timings.is_some());
}

#[test]
fn execution_failure_decomposition_preserves_evidence() {
    let owner = formula("1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}.");
    let mut session = Session::new(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    assert!(session.outcome().is_none());
    let token = Arc::new(());
    let failure = session
        .next_observed(&mut Refuse(Arc::clone(&token)))
        .unwrap()
        .unwrap_err();
    let parts = decompose(failure, &Original::Theory(owner.theory().clone()), &token);
    let semantic = parts.semantic.unwrap();
    assert!(
        semantic
            .original()
            .unwrap()
            .same_instance(parts.original.as_ref().unwrap())
    );
    assert_eq!(semantic.verified_models(), 1);
    assert_eq!(semantic.scored_models(), 1);
    assert_eq!(semantic.retained_models(), 1);
    assert!(semantic.completion().is_none());
    assert!(!semantic.optimum_proved());
    assert!(!semantic.unsatisfiable());
    assert!(parts.phase_timings.is_some());
    assert!(session.next().is_none());
    assert_eq!(
        session.outcome().unwrap().verified_models(),
        semantic.verified_models()
    );
}
