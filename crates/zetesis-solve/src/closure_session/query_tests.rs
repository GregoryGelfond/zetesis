//! Search evidence is independent of a post-batch ownership-snapshot fault.

use std::{num::NonZeroUsize, sync::Arc};

use zetesis_core::{AdmissionLimits, Atom, AtomPattern, Predicate, Program, Template};
use zetesis_cpu::{BatchError, Cancellation, Stop};

use super::ClosureSession;
use crate::{
    Backend, ExecutionResources, Grounder, Interruption, SearchState, SolveConfig, SolveError,
};
use crate::{execution_observation::Ignore, phase_timing::Recorder};

fn pattern(name: &str) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

fn program(choice: bool) -> Program {
    let mut templates = vec![Template::new(
        Some(pattern("a")),
        vec![],
        vec![],
        vec![],
        vec![],
    )];
    if choice {
        templates.push(Template::new(
            Some(pattern("b")),
            vec![],
            vec![pattern("b")],
            vec![],
            vec![],
        ));
    }
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

fn checked_prefix<'a>(
    program: &'a Program,
    config: &SolveConfig,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> ClosureSession<'a> {
    let mut session = ClosureSession::with_resources(
        program,
        None,
        config,
        &ExecutionResources::default(),
        &mut Ignore,
        cancellation,
        phases,
    )
    .unwrap();
    let answer = session.next(config, cancellation, phases).unwrap().unwrap();
    assert_eq!(
        answer.atoms().iter().cloned().collect::<Vec<_>>(),
        vec![Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap()]
    );
    assert!(session.ready.as_slice().is_empty());
    assert_eq!(session.verified, 1);
    session
}

fn fail_snapshot(
    session: &mut ClosureSession<'_>,
    config: &SolveConfig,
    cancellation: &Cancellation,
    phases: &Recorder,
) {
    // Exercise only the private notification/state seam after real membership
    // and prefix delivery. This does not assert that the CPU lock failed or
    // fabricate candidate work, preparation or ownership receipts.
    let fault = Arc::new(BatchError::Busy);
    session.pending_query_fault = Some(Arc::clone(&fault));
    let error = session
        .next(config, cancellation, phases)
        .unwrap()
        .unwrap_err();
    let SolveError::QueryObservation(original) = error else {
        panic!("lost snapshot cause");
    };
    assert!(Arc::ptr_eq(&original, &fault));
    assert_eq!(session.outcome().verified_models(), 1);
    assert_eq!(session.outcome().candidate_progress(), 1);
    assert!(session.next(config, cancellation, phases).is_none());
}

#[test]
fn snapshot_failure_preserves_known_search_state() {
    for expected in [
        SearchState::RequestedModels,
        SearchState::Exhausted,
        SearchState::Interrupted(Interruption::Oracle(Stop::CandidateLimit)),
    ] {
        let config = SolveConfig {
            backend: Backend::Cpu,
            grounder: Grounder::Lazy,
            models: usize::from(expected == SearchState::RequestedModels),
            max_candidates: 1,
            workers: NonZeroUsize::MIN,
            ..Default::default()
        };
        let owner = program(expected != SearchState::Exhausted);
        let cancellation = Cancellation::default();
        let phases = Recorder::new(false);
        let mut session = checked_prefix(&owner, &config, &cancellation, &phases);
        match expected {
            SearchState::Exhausted => {
                assert!(session.candidates.next_selection().is_none());
                session.finished_batch = true;
            }
            SearchState::Interrupted(Interruption::Oracle(stop)) => {
                let result = session.candidates.next_selection().unwrap();
                assert!(matches!(result, Err(actual) if actual == stop));
                session.pending_stop = Some(stop);
            }
            SearchState::RequestedModels => {}
            _ => unreachable!("the finite cases above contain only these states"),
        }
        fail_snapshot(&mut session, &config, &cancellation, &phases);
        assert_eq!(session.outcome().search_state(), Some(expected));
    }
}

#[test]
fn snapshot_failure_does_not_infer_coverage() {
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Lazy,
        models: 0,
        workers: NonZeroUsize::MIN,
        ..Default::default()
    };
    let owner = program(true);
    let cancellation = Cancellation::default();
    let phases = Recorder::new(false);
    let mut session = checked_prefix(&owner, &config, &cancellation, &phases);
    assert!(session.candidates.termination().is_none());
    fail_snapshot(&mut session, &config, &cancellation, &phases);
    assert!(session.outcome().search_state().is_none());
    assert!(session.outcome().completion().is_none());
}
