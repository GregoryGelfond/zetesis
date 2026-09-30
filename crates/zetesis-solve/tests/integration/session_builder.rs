//! A composed request preserves selection, control and failure boundaries.

use std::{collections::BTreeSet, io};

use crate::support::sessions::config;
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, ExecutionObservation, ExecutionObserver,
    ExecutionResources, GpuApi, Grounder, Oracle, PreparedInput, Session, SolveConfig, SolveError,
    Subject,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit, admit_formula,
};

fn objective() -> AdmittedFormula {
    admit_formula(
        "1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}. #show.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

#[test]
fn all_selection_preserves_nonoptimal_answers() {
    let owner = objective();
    let resources = ExecutionResources::default();
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .resources(&resources)
    .selection(AnswerSelection::All)
    .start()
    .unwrap();
    let answers: BTreeSet<_> = session
        .by_ref()
        .map(|answer| {
            let answer = answer.unwrap();
            assert!(
                answer
                    .subject()
                    .same_instance(&Subject::Theory(owner.theory().clone()))
            );
            let atoms: Vec<_> = answer
                .interpretation()
                .atoms()
                .iter()
                .map(|atom| atom.predicate().name().to_string())
                .collect();
            (atoms, answer.score().unwrap().costs().to_vec())
        })
        .collect();
    assert_eq!(
        answers,
        BTreeSet::from([
            (vec!["a".into()], vec![(2, 1)]),
            (vec!["b".into()], vec![(2, 2)]),
        ])
    );
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(!outcome.optimum_proved());
}

#[test]
fn default_selection_retains_the_optimum() {
    let owner = objective();
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .start()
    .unwrap();
    let answer = session.next().unwrap().unwrap();
    assert_eq!(answer.score().unwrap().costs(), [(2, 1)]);
    assert_eq!(answer.interpretation().atoms().len(), 1);
    assert_eq!(
        answer
            .interpretation()
            .atoms()
            .first()
            .unwrap()
            .predicate()
            .name(),
        "a"
    );
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.selection(), Some(AnswerSelection::Optimal));
    assert!(outcome.optimum_proved());
}

#[test]
fn start_polls_control_before_device_setup() {
    let owner = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let cancellation = Cancellation::default();
    let builder = Session::builder(
        PreparedInput::admitted(&owner),
        SolveConfig {
            backend: Backend::Gpu(Some(GpuApi::Metal)),
            grounder: Grounder::Eager,
            ..config()
        },
        cancellation.clone(),
    );
    // Cancellation after request construction must still precede device setup,
    // including in a CPU-only build where that setup would otherwise fail.
    cancellation.cancel();
    let mut session = builder.start().unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(outcome.verified_models(), 0);
    assert!(!outcome.unsatisfiable());
}

#[test]
fn invalid_request_retains_its_original_subject() {
    let owner = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let failure = Session::builder(
        PreparedInput::admitted(&owner),
        SolveConfig {
            oracle: Oracle::Countermodel,
            ..config()
        },
        Cancellation::default(),
    )
    .start()
    .err()
    .unwrap();
    assert!(matches!(*failure.cause, SolveError::PreparedInput { .. }));
    assert!(failure.semantic().is_none());
    assert!(
        failure
            .subject()
            .unwrap()
            .same_instance(&Subject::Program(owner.program().clone()))
    );
}

struct RefusePreparation;

impl ExecutionObserver for RefusePreparation {
    type Error = io::Error;

    fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        Err(io::Error::other("preparation observation refused"))
    }
}

#[test]
fn observed_start_preserves_the_external_failure() {
    let owner = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let failure = Session::builder(
        PreparedInput::admitted(&owner),
        config(),
        Cancellation::default(),
    )
    .selection(AnswerSelection::All)
    .resources(&ExecutionResources::default())
    .start_observed(&mut RefusePreparation)
    .err()
    .unwrap();
    let SolveError::ExecutionObservation(cause) = &*failure.cause else {
        panic!("observer failure must retain its own error boundary");
    };
    assert_eq!(cause.to_string(), "preparation observation refused");
    assert!(failure.semantic().is_none());
    assert!(
        failure
            .subject()
            .unwrap()
            .same_instance(&Subject::Program(owner.program().clone()))
    );
}
