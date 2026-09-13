//! Cooperative publication stops preserve semantic evidence and complete records.

use crate::{
    AnswerSelection, Completion, Options, PublicationOutcome, PublicationPhase, PublicationStop,
    RunError, Session,
};
use clap::Parser;
use std::{
    io::{self, Write},
    ops::ControlFlow,
};
use zetesis_cpu::{Control, Stop};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula,
    observation::{ErrorKind, ViewError},
};

fn options(json: bool) -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--workers",
        "1",
        "--completion-workers",
        "1",
        "--models",
        "0",
        "--color",
        "never",
        "--stats",
    ])
    .unwrap();
    options.json = json;
    options
}

struct CancelAfterRecord {
    bytes: Vec<u8>,
    control: Control,
    json: bool,
    fail_footer: bool,
}
impl Write for CancelAfterRecord {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_footer
            && (bytes.starts_with(b"],\"outcome\"") || bytes.starts_with(b"INCOMPLETE"))
        {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "footer unavailable",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        if if self.json {
            bytes == b"}\n"
        } else {
            bytes.starts_with(b"Answer:")
        } {
            self.control.cancel();
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn cancellation_between_optimal_ties_preserves_exhaustion_and_framing() {
    for json in [false, true] {
        let control = Control::default();
        let mut output = CancelAfterRecord {
            bytes: Vec::new(),
            control: control.clone(),
            json,
            fail_footer: false,
        };
        let mut diagnostics = Vec::new();
        let outcome = crate::run_finalized_with_diagnostics(
            "1 {a;b} 1. #minimize{1,a:a;1,b:b}.".into(),
            &options(json),
            &mut output,
            &mut diagnostics,
            &control,
        )
        .unwrap();
        let PublicationOutcome::Stopped(stopped) = &outcome else {
            panic!("publication must stop after the first proved tie")
        };
        assert_eq!(stopped.stop().reason(), Stop::Cancelled);
        assert_eq!(stopped.semantic().completion(), Some(Completion::Exhausted));
        assert!(stopped.semantic().optimum_proved());
        assert_eq!(stopped.semantic().retained_models(), 2);
        assert_eq!(stopped.publication().models(), 1);
        assert!(stopped.publication().summary());
        let diagnostic = String::from_utf8(diagnostics).unwrap();
        assert!(diagnostic.contains("publication: incomplete"));
        assert!(!diagnostic.contains("status: failed"));
        if json {
            let value: serde_json::Value = serde_json::from_slice(&output.bytes).unwrap();
            assert_eq!(value["models"].as_array().unwrap().len(), 1);
            assert_eq!(value["outcome"]["status"], "incomplete");
            assert_eq!(value["outcome"]["completion"], "exhausted");
            assert_eq!(value["outcome"]["optimization"]["optimal"], true);
            assert_eq!(value["outcome"]["publication_stop"]["code"], "cancelled");
            assert!(value["outcome"]["error"].is_null());
        } else {
            let text = String::from_utf8(output.bytes).unwrap();
            assert_eq!(text.matches("Answer:").count(), 1);
            assert!(text.contains("Publication: incomplete"));
            assert!(text.contains("Optimum proved; delivery incomplete"));
        }
        let failure = outcome.into_legacy().unwrap_err();
        assert!(matches!(
            failure.cause.as_ref(),
            RunError::PublicationStopped(Stop::Cancelled)
        ));
        assert!(failure.semantic().unwrap().optimum_proved());
        assert_eq!(failure.publication().unwrap().models(), 1);
    }
}

#[test]
fn publication_stop_before_first_record_keeps_unclassified_search() {
    for json in [false, true] {
        let owner = admit_formula(
            "{a;b}. #show x:a.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let options = options(json);
        let control = Control::default();
        let input = crate::PreparedInput::formula(&owner);
        let mut session = Session::builder(input, (&options).into(), control.clone())
            .selection(AnswerSelection::All)
            .start()
            .unwrap();
        let answer = session.next().unwrap().unwrap();
        let mut progress = super::Progress::new();
        progress.apply(session.progress());
        assert_eq!(progress.semantic().unwrap().completion(), None);
        control.cancel();
        let metadata = input.metadata().unwrap();
        let display = crate::display::Display {
            selection: metadata.output(),
            observations: metadata.observations(),
            options: &options,
            control: &control,
        };
        let mut output = Vec::new();
        let ControlFlow::Break(stop) = display
            .write(&mut output, 1, answer.interpretation(), answer.score())
            .unwrap()
        else {
            panic!("stopped display")
        };
        assert_eq!(stop.reason(), Stop::Cancelled);
        assert_eq!(stop.phase(), PublicationPhase::Observation);
        assert!(matches!(
            stop.observation().unwrap().kind(),
            ErrorKind::Stopped(Stop::Cancelled)
        ));
        assert!(output.is_empty());
        progress.stop = Some(stop);
        let phases = super::Recorder::new(false);
        let mut document = crate::output::Document::new(&mut output, json).unwrap();
        let progress =
            super::complete(&mut document, &mut io::sink(), progress, &phases, &options).unwrap();
        let outcome = document.finish(Ok(progress), &options).unwrap();
        assert!(matches!(outcome, PublicationOutcome::Stopped(_)));
        assert_eq!(outcome.semantic().completion(), None);
        assert_eq!(outcome.semantic().verified_models(), 1);
        assert_eq!(outcome.publication().models(), 0);
        assert!(outcome.publication().summary());
        if json {
            let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(value["outcome"]["status"], "incomplete");
            assert!(value["outcome"]["completion"].is_null());
            assert_eq!(value["outcome"]["coverage"], "unavailable");
            assert_eq!(value["outcome"]["verified_models"], 1);
            assert_eq!(
                value["outcome"]["checked"],
                outcome.semantic().candidate_progress()
            );
        }
    }
}

#[test]
fn final_writer_failure_preserves_the_prior_publication_stop() {
    let control = Control::default();
    let mut output = CancelAfterRecord {
        bytes: Vec::new(),
        control: control.clone(),
        json: true,
        fail_footer: true,
    };
    let failure = crate::run_finalized(
        "1 {a;b} 1. #minimize{1,a:a;1,b:b}.".into(),
        &options(true),
        &mut output,
        &control,
    )
    .unwrap_err();
    assert!(
        matches!(failure.cause.as_ref(), RunError::Output(error) if error.kind() == io::ErrorKind::BrokenPipe)
    );
    assert_eq!(
        failure.publication_stop().unwrap().reason(),
        Stop::Cancelled
    );
    assert_eq!(failure.publication().unwrap().models(), 1);
    assert!(!failure.publication().unwrap().summary());
    assert!(failure.semantic().unwrap().optimum_proved());
}

#[test]
fn cooperative_encoding_stops_are_distinct_from_resource_refusals() {
    for reason in [Stop::Cancelled, Stop::Deadline] {
        let stop =
            PublicationStop::classify(RunError::JsonRecord(ViewError::Stopped(reason))).unwrap();
        assert_eq!(stop.reason(), reason);
        assert_eq!(stop.phase(), PublicationPhase::Encoding);
    }
    for reason in [Stop::Allocation, Stop::WorkLimit] {
        assert!(
            matches!(PublicationStop::classify(RunError::PublicationStopped(reason)), Err(RunError::PublicationStopped(original)) if original == reason)
        );
        assert!(
            matches!(PublicationStop::classify(RunError::JsonRecord(ViewError::Stopped(reason))), Err(RunError::JsonRecord(ViewError::Stopped(original))) if original == reason)
        );
    }
    assert!(matches!(
        PublicationStop::classify(RunError::JsonRecord(ViewError::Bytes)),
        Err(RunError::JsonRecord(ViewError::Bytes))
    ));
    assert!(matches!(
        PublicationStop::classify(RunError::Output(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "closed"
        ))),
        Err(RunError::Output(_))
    ));
}
