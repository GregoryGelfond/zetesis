//! A composed request preserves selection, control and failure boundaries.

use std::{collections::BTreeSet, io};

use zetesis_cli::{
    AnswerSelection, Backend, Completion, ExecutionObservation, ExecutionObserver,
    ExecutionResources, Grounder, Oracle, PreparedInput, RunError, Session, SolveConfig, Subject,
};
use zetesis_cpu::Control;
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

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..Default::default()
    }
}

#[test]
fn all_selection_preserves_nonoptimal_answers() {
    let owner = objective();
    let resources = ExecutionResources::default();
    let mut session =
        Session::builder(PreparedInput::formula(&owner), config(), Control::default())
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
    let mut session =
        Session::builder(PreparedInput::formula(&owner), config(), Control::default())
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
    let control = Control::default();
    let builder = Session::builder(
        PreparedInput::admitted(&owner),
        SolveConfig {
            backend: Backend::Metal,
            grounder: Grounder::Eager,
            ..config()
        },
        control.clone(),
    );
    // Cancellation after request construction must still precede device setup,
    // including in a CPU-only build where that setup would otherwise fail.
    control.cancel();
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
        Control::default(),
    )
    .start()
    .err()
    .unwrap();
    assert!(matches!(*failure.cause, RunError::PreparedInput { .. }));
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
        Control::default(),
    )
    .selection(AnswerSelection::All)
    .resources(&ExecutionResources::default())
    .start_observed(&mut RefusePreparation)
    .err()
    .unwrap();
    let RunError::ExecutionObservation(cause) = &*failure.cause else {
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
