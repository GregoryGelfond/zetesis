//! Compatibility projections must not become semantic state.

use std::{io, num::NonZeroUsize};

use clap::Parser;
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, Admitted, ExpansionLimits, FormulaLimits, admit_extended, admit_formula,
};

use crate::{Backend, Completion, PreparedInput, RunError, Session, SolveConfig, Subject};

use super::Progress;

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        workers: NonZeroUsize::MIN,
        models: 0,
        ..Default::default()
    }
}

fn normal() -> Admitted {
    admit_extended(
        "a.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap()
}

fn output_failure() -> RunError {
    RunError::Output(io::Error::new(io::ErrorKind::BrokenPipe, "closed sink"))
}

#[test]
fn unentered_progress_has_no_success_report() {
    let progress = Progress::new();
    assert!(matches!(
        progress.report(),
        Err(RunError::CompletionUnavailable)
    ));
    let failure = progress.finalize().unwrap_err();
    assert!(matches!(*failure.cause, RunError::CompletionUnavailable));
    assert!(failure.subject().is_none());
    assert!(failure.semantic().is_none());
    let partial = failure.partial_report.unwrap();
    assert_eq!(partial.completion, None);
    assert_eq!(partial.interruption, None);
    assert_eq!(partial.verified_models, 0);
}

#[test]
fn unclassified_membership_cannot_make_a_success_report() {
    let admitted = normal();
    let mut session = Session::new(
        PreparedInput::admitted(&admitted),
        config(),
        Control::default(),
    )
    .unwrap();
    assert!(session.next().unwrap().is_ok());
    let mut progress = Progress::new();
    progress.apply(session.stop());
    let failure = progress.finalize().unwrap_err();
    assert!(matches!(*failure.cause, RunError::CompletionUnavailable));
    let semantic = failure.semantic().unwrap();
    assert_eq!(semantic.verified_models(), 1);
    assert_eq!(semantic.completion(), None);
    assert!(!semantic.unsatisfiable());
    let Subject::Program(program) = failure.subject().unwrap() else {
        panic!("original program");
    };
    assert!(program.same_instance(admitted.program()));
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!(partial.completion, None);
    assert_eq!(partial.verified_models, 1);
    assert_eq!(failure.publication().unwrap().models(), 0);
}

#[test]
fn report_mutation_cannot_rewrite_later_failure() {
    let admitted = normal();
    let mut session = Session::new(
        PreparedInput::admitted(&admitted),
        config(),
        Control::default(),
    )
    .unwrap();
    assert!(session.next().unwrap().is_ok());
    assert!(session.next().is_none());
    let mut progress = Progress::new();
    progress.apply(session.stop());
    progress.publication.models = 1;
    let mut report = progress.report().unwrap();
    report.models = 7;
    report.checked = 42;
    report.completion = Completion::RequestedModels;
    assert_eq!(report.models, 7);
    let failure = progress.fail(output_failure());
    let semantic = failure.semantic().unwrap();
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!(semantic.completion(), Some(Completion::Exhausted));
    assert_eq!(partial.completion, semantic.completion());
    assert_eq!(partial.checked, semantic.candidate_progress());
    assert_ne!(partial.checked, report.checked);
    assert_eq!(partial.published_models, 1);
    assert_eq!(failure.publication().unwrap().models(), 1);
    assert!(!failure.publication().unwrap().summary());
    assert!(
        matches!(&*failure.cause, RunError::Output(error) if error.to_string() == "closed sink")
    );
}

#[test]
fn scoring_stop_remains_distinct_from_publication() {
    let admitted = admit_formula(
        "a. #minimize {1,k:a}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let mut session = Session::new(
        PreparedInput::formula(&admitted),
        SolveConfig {
            max_objective_work: 0,
            ..config()
        },
        Control::default(),
    )
    .unwrap();
    assert!(session.next().is_none());
    let mut progress = Progress::new();
    progress.apply(session.stop());
    let failure = progress.fail(output_failure());
    let semantic = failure.semantic().unwrap();
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!(semantic.verified_models(), 1);
    assert_eq!(semantic.scored_models(), 0);
    assert_eq!(semantic.retained_models(), 0);
    assert_eq!(partial.verified_models, 1);
    assert_eq!(partial.published_models, 0);
    assert_eq!(partial.completion, Some(Completion::Interrupted));
    assert!(matches!(
        partial.interruption,
        Some(crate::Interruption::Objective(_))
    ));
    assert_eq!(partial.interruption, semantic.interruption());
    assert!(partial.optimization.is_none());
    assert!(!semantic.unsatisfiable());
    let Subject::Theory(theory) = failure.subject().unwrap() else {
        panic!("original theory");
    };
    assert!(theory.same_instance(admitted.theory()));
}

#[test]
fn missing_completion_prevents_a_success_footer() {
    let options = crate::Options::try_parse_from(["zetesis", "--json"]).unwrap();
    let mut bytes = Vec::new();
    let document = crate::output::Document::new(&mut bytes, true).unwrap();
    let failure = document.finish(Ok(Progress::new()), &options).unwrap_err();
    assert!(matches!(*failure.cause, RunError::CompletionUnavailable));
    assert_eq!(failure.partial_report.as_ref().unwrap().completion, None);
    assert!(!failure.publication().unwrap().summary());
    assert_eq!(bytes, br#"{"schema":1,"format":"zetesis","models":["#);
}

#[test]
fn completion_fault_has_an_unavailable_json_outcome() {
    let options = crate::Options::try_parse_from(["zetesis", "--json"]).unwrap();
    let mut bytes = Vec::new();
    let document = crate::output::Document::new(&mut bytes, true).unwrap();
    let failed = Progress::new().fail(RunError::CompletionUnavailable);
    let failure = document.finish(Err(failed), &options).unwrap_err();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["outcome"]["status"], "failed");
    assert_eq!(value["outcome"]["completion"], serde_json::Value::Null);
    assert_eq!(value["outcome"]["coverage"], "unavailable");
    assert_eq!(value["outcome"]["error"]["kind"], "completion_unavailable");
    assert!(failure.publication().unwrap().summary());
    assert!(failure.partial_report.as_ref().unwrap().summary_published);
    assert!(failure.semantic().is_none());
}
