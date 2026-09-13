//! Output capacity cannot retract an already checked CPU conclusion.

use super::{Document, Progress, summary};
use crate::{Options, PublicationOutcome, RunError};
use clap::Parser;
use std::io;
use zetesis_cpu::Control;
use zetesis_themelios::observation::ViewError;

const PREFIX: &[u8] = b"{\"schema\":1,\"format\":\"zetesis\",\"models\":[";
const FIXTURE_BYTES: usize = 32_768;

fn options(oracle: &str) -> Options {
    Options::parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--models",
        "0",
        "--json",
        "--stats",
        "--oracle",
        oracle,
    ])
}

fn progress(outcome: &PublicationOutcome) -> Progress {
    let mut progress = Progress::new();
    progress.apply(outcome.semantic().clone());
    progress.publication = outcome.publication();
    // This new sink has not yet accepted the already computed summary.
    progress.publication.summary = false;
    progress.phase_timings = outcome.report().unwrap().phase_timings;
    progress
}

#[test]
fn every_footer_byte_ceiling_preserves_completed_cpu_evidence() {
    for (source, oracle) in [
        ("p. :- p.", "closure"),
        ("a | b. :- a. :- b.", "countermodel"),
    ] {
        let mut options = options(oracle);
        let outcome = crate::run_finalized_with_diagnostics(
            source.into(),
            &options,
            &mut io::sink(),
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap();
        assert!(outcome.semantic().unsatisfiable());
        assert_eq!(outcome.publication().models(), 0);
        assert!(outcome.report().unwrap().phase_timings.is_some());
        let expected = summary(&Ok(progress(&outcome)), FIXTURE_BYTES).unwrap();
        assert!(!expected.is_empty());
        for maximum in 0..expected.len() {
            options.max_json_record_bytes = maximum;
            let mut sink = Vec::new();
            let failure = Document::new(&mut sink, true)
                .unwrap()
                .finish(Ok(progress(&outcome)), &options)
                .unwrap_err();
            assert!(matches!(
                *failure.cause,
                RunError::JsonRecord(ViewError::Bytes)
            ));
            assert!(failure.semantic().unwrap().unsatisfiable());
            assert_eq!(
                failure.semantic().unwrap().candidate_progress(),
                outcome.semantic().candidate_progress()
            );
            assert!(!failure.publication().unwrap().summary());
            assert_eq!(
                sink, PREFIX,
                "footer must be admitted before its first byte"
            );
        }
        options.max_json_record_bytes = expected.len();
        let mut sink = Vec::new();
        let completed = Document::new(&mut sink, true)
            .unwrap()
            .finish(Ok(progress(&outcome)), &options)
            .unwrap();
        assert!(completed.publication().summary());
        assert_eq!(&sink[PREFIX.len()..], expected);
        let document: serde_json::Value = serde_json::from_slice(&sink).unwrap();
        assert_eq!(document["outcome"]["status"], "unsatisfiable");
        assert_eq!(document["outcome"]["verified_models"], 0);
        assert_eq!(document["models"], serde_json::json!([]));
    }
}

#[test]
fn footer_capacity_failure_cannot_replace_a_real_source_refusal() {
    let mut options = options("countermodel");
    let failure = crate::run_finalized_with_diagnostics(
        "#project a/0.".into(),
        &options,
        &mut io::sink(),
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::FormulaAdmission(_)));
    assert!(failure.semantic().is_none());
    let original = failure.cause.to_string();
    let result = Err(failure);
    let expected = summary(&result, FIXTURE_BYTES).unwrap();
    for maximum in 0..expected.len() {
        assert!(matches!(
            summary(&result, maximum),
            Err(RunError::JsonRecord(ViewError::Bytes))
        ));
    }
    options.max_json_record_bytes = expected.len() - 1;
    let mut sink = Vec::new();
    let retained = Document::new(&mut sink, true)
        .unwrap()
        .finish(result, &options)
        .unwrap_err();
    assert!(matches!(*retained.cause, RunError::FormulaAdmission(_)));
    assert_eq!(retained.cause.to_string(), original);
    assert!(retained.semantic().is_none());
    assert!(retained.summary_failure().is_some());
    assert_eq!(sink, PREFIX);
}
