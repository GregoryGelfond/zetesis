//! Typed failure decomposition preserves input identity and established evidence.

use std::{fmt, sync::Arc};

use zetesis_cpu::Control;
use zetesis_solve::{
    Backend, ExecutionObservation, ExecutionObserver, PreparedInput, Session, SolveConfig,
    SolveError, SolveFailure, Subject,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit, admit_formula};

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
    subject: &Subject,
    token: &Arc<()>,
) -> zetesis_solve::FailureParts {
    assert!(failure.subject().unwrap().same_instance(subject));
    let cause = std::ptr::from_ref(failure.cause.as_ref());
    let semantic = failure.semantic().map(std::ptr::from_ref);
    let phases = failure.phase_timings.as_deref().copied();
    let parts = failure.into_parts();
    assert!(parts.subject.as_ref().unwrap().same_instance(subject));
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
    let owner = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let token = Arc::new(());
    let failure = Session::builder(
        PreparedInput::admitted(&owner),
        config(),
        Control::default(),
    )
    .start_observed(&mut Refuse(Arc::clone(&token)))
    .err()
    .unwrap();
    let parts = decompose(failure, &Subject::Program(owner.program().clone()), &token);
    assert!(parts.semantic.is_none());
    assert!(parts.phase_timings.is_some());
}

#[test]
fn execution_failure_decomposition_preserves_evidence() {
    let owner = admit_formula(
        "1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let mut session =
        Session::new(PreparedInput::formula(&owner), config(), Control::default()).unwrap();
    assert!(session.outcome().is_none());
    let token = Arc::new(());
    let failure = session
        .next_observed(&mut Refuse(Arc::clone(&token)))
        .unwrap()
        .unwrap_err();
    let parts = decompose(failure, &Subject::Theory(owner.theory().clone()), &token);
    let semantic = parts.semantic.unwrap();
    assert!(
        semantic
            .subject()
            .unwrap()
            .same_instance(parts.subject.as_ref().unwrap())
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
