//! Host-produced failures retain their typed identity across the CLI adapter.

use std::{error::Error, fmt, io, num::NonZeroUsize, sync::Arc};
use zetesis_cli::{
    Backend, ExecutionObservation, ExecutionObserver, Grounder, Oracle, PreparedInput,
    PreparedProfile, PublicationFailure, RunError, Session, SolveConfig, Subject,
};
use zetesis_cpu::Cancellation;
use zetesis_reference_support::formula;

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

/// A setup failure returned by the first pull establishes no coverage: no
/// model verified, no completion and no claim of unsatisfiability.
fn no_coverage(semantic: Option<&zetesis_cli::SemanticOutcome>) {
    if let Some(semantic) = semantic {
        assert_eq!(semantic.verified_models(), 0);
        assert_eq!(semantic.completion(), None);
        assert!(!semantic.unsatisfiable());
    }
}

#[test]
fn typed_observer_failure_keeps_its_external_owner() {
    let owner = formula("a.");
    let subject = Subject::Theory(owner.theory().clone());
    let token = Arc::new(());
    // Formula initialization owns an observer failure at its setup and returns
    // it from the first pull, with the original cause.
    let original = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .start_observed(&mut FailTyped(Arc::clone(&token)))
    .unwrap()
    .next()
    .unwrap()
    .unwrap_err();
    no_coverage(original.semantic());
    let failure = PublicationFailure::from(original);
    drop(owner);
    assert!(failure.subject().unwrap().same_instance(&subject));
    no_coverage(failure.semantic());
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
    let owner = formula("a.");
    let token = Arc::new(());
    // As above, the first pull returns the setup observer's failure.
    let original = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .start_observed(&mut FailIo(Arc::clone(&token)))
    .unwrap()
    .next()
    .unwrap()
    .unwrap_err();
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
    no_coverage(failure.semantic());
    assert!(failure.publication().is_none());
}

#[test]
fn prepared_refusal_keeps_the_requested_policy() {
    let owner = formula("a | b.");
    let original = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            oracle: Oracle::Auto,
            grounder: Grounder::Lazy,
            ..config()
        },
        Cancellation::default(),
    )
    .err()
    .unwrap();
    let failure = PublicationFailure::from(original);
    assert!(matches!(
        failure.cause.as_ref(),
        RunError::PreparedInput {
            profile: PreparedProfile::Formula,
            oracle: Oracle::Auto,
            grounder: Grounder::Lazy,
        }
    ));
    assert_eq!(
        failure.to_string(),
        "prepared Formula cannot honor oracle auto with grounder lazy"
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
