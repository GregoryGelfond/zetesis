//! CPU closure setup owns its collective reservation; other routes do not.

use std::{collections::BTreeSet, convert::Infallible, num::NonZeroUsize};

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Template};
use zetesis_cpu::{Control, Stop};
use zetesis_solve::{
    Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder, Interruption,
    PreparedInput, Session, SolveConfig, SolveError, SolveMeasurements, SolvePhase, SourceBatching,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[derive(Default)]
struct Observations(usize);

impl ExecutionObserver for Observations {
    type Error = Infallible;

    fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        self.0 += 1;
        Ok(())
    }
}

/// Five independent choices: thirty-two candidates, all answer sets.
fn program() -> Program {
    let choice = |name: &str| {
        let atom = AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap();
        Template::new(Some(atom.clone()), vec![], vec![atom], vec![], vec![])
    };
    Program::new(
        ["a", "b", "c", "d", "e"].map(choice).to_vec(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn with_workers(workers: usize) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        workers: NonZeroUsize::new(workers).unwrap(),
        ..Default::default()
    }
}

#[test]
fn a_worker_product_above_the_collective_ceiling_is_refused_by_name() {
    let program = program();
    for (grounder, source_batching) in [
        (Grounder::Lazy, SourceBatching::Independent),
        (Grounder::Eager, SourceBatching::Independent),
        (Grounder::Lazy, SourceBatching::Union),
        (Grounder::Lazy, SourceBatching::Worlds),
    ] {
        let measurements = SolveMeasurements::new(true);
        let mut observations = Observations::default();
        let failure = Session::builder(
            PreparedInput::program(&program),
            SolveConfig {
                grounder,
                source_batching,
                max_atoms: 0,
                max_source_work: 0,
                max_work: 0,
                ..with_workers(5)
            },
            Control::default(),
        )
        .measurements(&measurements)
        .start_observed(&mut observations)
        .err()
        .expect("CPU closure setup must refuse its reservation");
        assert!(matches!(
            failure.cause.as_ref(),
            SolveError::ClosureReservation {
                workers: 5,
                max_closure_bytes: 134_217_728,
                max_closure_batch_bytes: 536_870_912,
            }
        ));
        let text = failure.to_string();
        assert!(text.contains('5') && text.contains("671088640"), "{text}");
        assert!(failure.semantic().is_none());
        assert_eq!(observations.0, 0);
        let timings = measurements.snapshot().unwrap();
        assert_eq!(timings.get(SolvePhase::ExecutionSetup).unwrap().calls, 1);
        assert!(timings.get(SolvePhase::CandidateSetup).is_none());
        assert!(timings.get(SolvePhase::ClosureMembership).is_none());
    }
}

#[test]
fn policy_validation_does_not_reserve_closure_storage() {
    assert!(with_workers(5).validate().is_ok());
}

#[test]
fn overflowing_cpu_reservations_are_refused() {
    let program = program();
    let failure = Session::builder(
        PreparedInput::program(&program),
        SolveConfig {
            max_closure_bytes: usize::MAX,
            max_closure_batch_bytes: usize::MAX,
            ..with_workers(2)
        },
        Control::default(),
    )
    .start()
    .err()
    .expect("overflowing CPU reservation must be refused");
    assert!(matches!(
        failure.cause.as_ref(),
        SolveError::ClosureReservation {
            workers: 2,
            max_closure_bytes: usize::MAX,
            max_closure_batch_bytes: usize::MAX,
        }
    ));
    assert!(failure.semantic().is_none());
}

#[test]
fn pre_cancelled_sessions_skip_cpu_resource_setup() {
    let program = program();
    let control = Control::default();
    control.cancel();
    let measurements = SolveMeasurements::new(true);
    let mut observations = Observations::default();
    let mut session = Session::builder(PreparedInput::program(&program), with_workers(5), control)
        .measurements(&measurements)
        .start_observed(&mut observations)
        .unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Preparation(Stop::Cancelled))
    ));
    assert_eq!(outcome.candidate_progress(), 0);
    assert!(outcome.candidate_statistics().is_none());
    assert!(outcome.closure_execution().is_none());
    assert!(outcome.query_execution().is_none());
    assert_eq!(observations.0, 0);
    let timings = measurements.snapshot().unwrap();
    for phase in [
        SolvePhase::ExecutionSetup,
        SolvePhase::CandidateSetup,
        SolvePhase::ClosureMembership,
    ] {
        assert!(timings.get(phase).is_none(), "{phase:?}");
    }
}

#[test]
fn policy_incompatibility_precedes_cancellation() {
    let program = program();
    let control = Control::default();
    control.cancel();
    let failure = Session::builder(
        PreparedInput::program(&program),
        SolveConfig {
            grounder: Grounder::Eager,
            source_batching: SourceBatching::Union,
            ..with_workers(5)
        },
        control,
    )
    .start()
    .err()
    .expect("invalid policy must remain a setup error");
    assert!(matches!(
        failure.cause.as_ref(),
        SolveError::UnsupportedSourceBatching
    ));
}

#[test]
fn formula_execution_does_not_reserve_closure_storage() {
    let owner = admit_formula(
        "{a;b}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_closure_bytes: usize::MAX,
            max_closure_batch_bytes: 0,
            ..with_workers(5)
        },
        Control::default(),
    )
    .start()
    .unwrap();
    let answers: BTreeSet<_> = session
        .by_ref()
        .map(|answer| answer.unwrap().interpretation().clone())
        .collect();
    assert_eq!(answers.len(), 4);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 4);
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(outcome.countermodel_statistics().is_some());
    assert!(outcome.closure_execution().is_none());
}

#[test]
fn a_product_at_the_ceiling_is_admitted() {
    let program = program();
    assert!(
        Session::builder(
            PreparedInput::program(&program),
            with_workers(4),
            Control::default(),
        )
        .start()
        .is_ok()
    );
}

#[test]
fn eight_workers_complete_the_family_at_their_collective_share() {
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::for_allowance(SolveConfig::REFERENCE_MEMORY, NonZeroUsize::new(8).unwrap())
    };
    let program = program();
    let mut session =
        Session::builder(PreparedInput::program(&program), config, Control::default())
            .start()
            .unwrap();
    let mut answers = 0;
    for answer in session.by_ref() {
        answer.unwrap();
        answers += 1;
    }
    assert_eq!(answers, 32);
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
}
