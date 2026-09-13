//! Host-produced failures retain their typed identity across the CLI adapter.

use std::{error::Error, fmt, io, num::NonZeroUsize, sync::Arc};
use zetesis_cli::{
    Backend, ExecutionObservation, ExecutionObserver, Grounder, Oracle, PreparedInput,
    PreparedProfile, PublicationFailure, RunError, Session, SolveConfig, SolveError, Subject,
};
use zetesis_core::{GroundProgram, Seed, StaticLimits, WordError};
use zetesis_cpu::{BatchError, BatchOracle, Control, Limits};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit, admit_formula};

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        workers: NonZeroUsize::MIN,
        stats: true,
        ..Default::default()
    }
}

#[derive(Debug)]
struct External(Arc<()>);

impl fmt::Display for External {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("host observer closed")
    }
}
impl Error for External {}

struct FailTyped(Arc<()>);
impl ExecutionObserver for FailTyped {
    type Error = External;
    fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        Err(External(Arc::clone(&self.0)))
    }
}

struct FailIo(Arc<()>);
impl ExecutionObserver for FailIo {
    type Error = io::Error;
    fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            External(Arc::clone(&self.0)),
        ))
    }
}

#[test]
fn typed_observer_failure_keeps_its_external_owner() {
    let owner = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let subject = Subject::Program(owner.program().clone());
    let token = Arc::new(());
    let original = Session::builder(
        PreparedInput::admitted(&owner),
        config(),
        Control::default(),
    )
    .start_observed(&mut FailTyped(Arc::clone(&token)))
    .err()
    .unwrap();
    assert!(original.semantic().is_none());
    let failure = PublicationFailure::from(original);
    drop(owner);
    assert!(failure.subject().unwrap().same_instance(&subject));
    assert!(failure.semantic().is_none());
    assert!(failure.publication().is_none());
    assert!(failure.phase_timings.is_some());
    assert_eq!(
        failure.to_string(),
        "execution observer: host observer closed"
    );
    let cause = failure
        .source()
        .unwrap()
        .downcast_ref::<RunError>()
        .unwrap();
    assert!(std::ptr::eq(cause, failure.cause.as_ref()));
    assert!(matches!(cause, RunError::ExecutionObservation(_)));
    let external = cause.source().unwrap().downcast_ref::<External>().unwrap();
    assert!(Arc::ptr_eq(&external.0, &token));
    assert!(external.source().is_none());
}

#[test]
fn io_observer_failure_keeps_its_writer_kind() {
    let owner = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let token = Arc::new(());
    let original = Session::builder(
        PreparedInput::admitted(&owner),
        config(),
        Control::default(),
    )
    .start_observed(&mut FailIo(Arc::clone(&token)))
    .err()
    .unwrap();
    let failure = PublicationFailure::from(original);
    assert_eq!(failure.to_string(), "output: host observer closed");
    let RunError::Output(error) = failure.cause.as_ref() else {
        panic!("an actual observer I/O failure must use the writer boundary");
    };
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    let external = error.get_ref().unwrap().downcast_ref::<External>().unwrap();
    assert!(Arc::ptr_eq(&external.0, &token));
    let linked = failure
        .cause
        .source()
        .unwrap()
        .downcast_ref::<io::Error>()
        .unwrap();
    assert!(std::ptr::eq(linked, error));
    assert!(failure.semantic().is_none());
    assert!(failure.publication().is_none());
}

#[test]
fn prepared_refusal_keeps_the_requested_policy() {
    let owner = admit_formula(
        "a | b.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let original = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            oracle: Oracle::Closure,
            grounder: Grounder::Eager,
            ..config()
        },
        Control::default(),
    )
    .err()
    .unwrap();
    let failure = PublicationFailure::from(original);
    assert!(matches!(
        failure.cause.as_ref(),
        RunError::PreparedInput {
            profile: PreparedProfile::Formula,
            oracle: Oracle::Closure,
            grounder: Grounder::Eager,
        }
    ));
    assert_eq!(
        failure.to_string(),
        "prepared Formula cannot honor oracle closure with grounder eager"
    );
    assert!(failure.cause.source().is_none());
    assert!(failure.semantic().is_none());
    assert!(
        failure
            .subject()
            .unwrap()
            .same_instance(&Subject::Theory(owner.theory().clone()))
    );
}

#[test]
fn batch_refusal_keeps_its_admission_counts() {
    let owner = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let seed = Seed::new(owner.program(), []).unwrap();
    let pool = BatchOracle::new(NonZeroUsize::MIN, NonZeroUsize::MIN).unwrap();
    let refusal = pool
        .check_batch(
            owner.program(),
            &[seed.clone(), seed.clone()],
            Limits::default(),
            &Control::default(),
        )
        .unwrap_err();
    // This is the typed failure adapter; the oversized batch did no membership work.
    let failure = RunError::from(SolveError::Batch(refusal));
    assert_eq!(failure.to_string(), "batch of 2 exceeds capacity 1");
    assert!(matches!(
        failure.source().unwrap().downcast_ref::<BatchError>(),
        Some(BatchError::Capacity {
            limit: 1,
            actual: 2
        })
    ));
    let checks = pool
        .check_batch(
            owner.program(),
            &[seed],
            Limits::default(),
            &Control::default(),
        )
        .unwrap();
    assert_eq!(checks.len(), 1);
    assert!(checks[0].as_ref().unwrap().accepted());
}

#[test]
fn malformed_host_words_keep_the_decode_cause() {
    let owner = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let ground = GroundProgram::compile(owner.program(), StaticLimits::default()).unwrap();
    let refusal = ground.model_from_words(&[]).unwrap_err();
    let expected = refusal.to_string();
    // Malformed host decode input is real; no device execution is represented.
    let failure = RunError::from(SolveError::Words(refusal));
    assert_eq!(failure.to_string(), expected);
    assert!(matches!(failure, RunError::Words(_)));
    assert!(
        failure
            .source()
            .unwrap()
            .downcast_ref::<WordError>()
            .is_some()
    );
    assert_eq!(ground.model_from_words(&[1]).unwrap().atoms().len(), 1);
}
