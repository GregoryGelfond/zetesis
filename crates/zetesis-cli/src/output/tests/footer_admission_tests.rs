//! Output capacity cannot retract an already checked CPU conclusion.

use zetesis_test_support::document::spelled;

use super::document_fixture::{Document, summary};
use crate::failure::Progress;
use crate::{Options, PublicationOutcome, RunError};
use clap::Parser;
use std::io::{self, Write};
use zetesis_cpu::Cancellation;
use zetesis_themelios::observation::ViewError;

const PREFIX: &[u8] = b"{\"schema\":2,\"format\":\"zetesis\",\"models\":[";
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

fn checked_footer(outcome: &PublicationOutcome) -> serde_json::Value {
    assert_eq!(outcome.publication().models(), 0);
    checked_footer_records(outcome, &[])
}

fn checked_footer_records(outcome: &PublicationOutcome, records: &[u8]) -> serde_json::Value {
    let expected_prefix = [PREFIX, records].concat();
    let expected = summary(&Ok(progress(outcome)), FIXTURE_BYTES).unwrap();
    assert!(!expected.is_empty());
    for maximum in 0..expected.len() {
        let mut sink = Vec::new();
        let mut document = Document::new(&mut sink, true).unwrap();
        document.write_all(records).unwrap();
        let failure = document.finish(Ok(progress(outcome)), maximum).unwrap_err();
        assert!(matches!(
            *failure.cause,
            RunError::JsonRecord(ViewError::Bytes)
        ));
        let semantic = failure.semantic().unwrap();
        assert_eq!(semantic.completion(), outcome.semantic().completion());
        assert_eq!(semantic.interruption(), outcome.semantic().interruption());
        assert_eq!(semantic.unsatisfiable(), outcome.semantic().unsatisfiable());
        assert_eq!(
            semantic.candidate_progress(),
            outcome.semantic().candidate_progress()
        );
        assert_eq!(
            semantic.shared_execution(),
            outcome.semantic().shared_execution()
        );
        assert!(!failure.publication().unwrap().summary());
        assert_eq!(
            sink, expected_prefix,
            "footer must be admitted before its first byte"
        );
    }
    let mut sink = Vec::new();
    let mut document = Document::new(&mut sink, true).unwrap();
    document.write_all(records).unwrap();
    let completed = document
        .finish(Ok(progress(outcome)), expected.len())
        .unwrap();
    assert!(completed.publication().summary());
    assert_eq!(&sink[expected_prefix.len()..], expected);
    serde_json::from_slice(&sink).unwrap()
}

#[test]
fn every_footer_byte_ceiling_preserves_completed_cpu_evidence() {
    for (source, oracle) in [
        ("p. :- p.", "closure"),
        ("a | b. :- a. :- b.", "countermodel"),
    ] {
        let options = options(oracle);
        let outcome = crate::run_finalized_with_diagnostics(
            source.into(),
            &options,
            &mut io::sink(),
            &mut io::sink(),
            &Cancellation::default(),
        )
        .unwrap();
        assert!(outcome.semantic().unsatisfiable());
        assert_eq!(outcome.publication().models(), 0);
        assert!(outcome.report().unwrap().phase_timings.is_some());
        let document = checked_footer(&outcome);
        assert_eq!(document["outcome"]["status"], "unsatisfiable");
        assert_eq!(document["outcome"]["verified_models"], 0);
        assert_eq!(document["models"], serde_json::json!([]));
    }
}

#[test]
fn batched_cpu_footer_admission_preserves_exact_completion() {
    const MARKER: &[u8] = b"],\"outcome\":";
    let options = options("countermodel");
    let mut config = crate::PublicationConfig::from(&options);
    config.solve.completion_workers = std::num::NonZeroUsize::new(2).unwrap();
    config.solve.batch_size = std::num::NonZeroUsize::new(2).unwrap();
    // Two singleton answer sets remain after supported-candidate selection.
    // Both must pass the actual CPU residual-completion owner.
    let mut original = Vec::new();
    let outcome = crate::publication_fixture::json(
        "a | b.",
        &config,
        FIXTURE_BYTES,
        &mut original,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        outcome.semantic().completion(),
        Some(crate::Completion::Exhausted)
    );
    assert_eq!(outcome.semantic().verified_models(), 2);
    assert_eq!(outcome.publication().models(), 2);
    assert!(!outcome.semantic().unsatisfiable());
    let parsed: serde_json::Value = serde_json::from_slice(&original).unwrap();
    let mut models: Vec<_> = parsed["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| serde_json::Value::Array(spelled(&parsed, record)))
        .collect();
    models.sort_by_cached_key(ToString::to_string);
    assert_eq!(
        models,
        vec![
            serde_json::json!([{"predicate":"a","sign":"positive","arguments":[]}]),
            serde_json::json!([{"predicate":"b","sign":"positive","arguments":[]}]),
        ]
    );
    let execution = outcome.semantic().formula_execution().unwrap();
    assert_eq!(execution.completion.requested_workers, 2);
    assert_eq!(execution.completion.entered, 2);
    assert_eq!(execution.completion.completed, execution.completion.entered);
    assert_eq!(execution.completion.failed, 0);
    assert_eq!(execution.gpu_submitted_candidates, 0);
    assert_eq!(execution.gpu_work, 0);
    assert_eq!(execution.cpu_residuals, 2);
    assert!(original.starts_with(PREFIX));
    let boundary = original
        .windows(MARKER.len())
        .position(|bytes| bytes == MARKER)
        .unwrap();
    let document = checked_footer_records(&outcome, &original[PREFIX.len()..boundary]);
    assert_eq!(document, parsed);
    let encoded = &document["statistics"]["execution"];
    assert!(encoded["adapter"].is_null());
    assert!(encoded["gpu_limits"].is_null());
    assert_eq!(encoded["gpu_submitted_candidates"], 0);
    assert_eq!(
        encoded["completion"]["entered"],
        execution.completion.entered
    );
    assert_eq!(
        encoded["completion"]["completed"],
        execution.completion.completed
    );
    assert_eq!(encoded["completion"]["failed"], 0);
    assert_eq!(encoded["completion"]["complete"], true);
    assert_eq!(document["outcome"]["status"], "satisfiable");
}

#[test]
fn every_footer_byte_ceiling_preserves_shared_cpu_refusals() {
    use crate::SourceBatching;
    use zetesis_cpu::{Stop, lazy::shared::Cause};

    for (selection, source_stop) in [
        (SourceBatching::Union, false),
        (SourceBatching::Worlds, true),
    ] {
        let mut options = options("closure");
        options.source_batching = selection;
        let mut config = crate::PublicationConfig::from(&options);
        if source_stop {
            config.solve.max_source_work = 0;
        } else {
            config.solve.max_work = 0;
        }
        let admitted = zetesis_themelios::admit_extended(
            "a.".into(),
            zetesis_themelios::AdmissionOptions::default(),
            zetesis_themelios::ExpansionLimits::default(),
        )
        .unwrap();
        let outcome = crate::publish_prepared(
            crate::PreparedInput::admitted(&admitted),
            &config,
            &mut crate::JsonRenderer::new(io::sink(), FIXTURE_BYTES, config.solve.max_atoms),
            &mut io::sink(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            outcome.semantic().completion(),
            Some(crate::Completion::Interrupted)
        );
        assert_eq!(outcome.semantic().verified_models(), 0);
        assert!(!outcome.semantic().unsatisfiable());
        let stats = outcome.semantic().shared_execution().unwrap();
        assert_eq!(stats.submitted_candidates, 1);
        assert_eq!(stats.completed_candidates, 0);
        assert_eq!(stats.stopped_candidates, 1);
        let expected = if source_stop {
            Cause::Source(Stop::WorkLimit)
        } else {
            Cause::World {
                index: 0,
                stop: Stop::WorkLimit,
            }
        };
        assert_eq!(stats.last_stop, Some(expected));
        let document = checked_footer(&outcome);
        assert_eq!(document["outcome"]["status"], "incomplete");
        assert_eq!(document["models"], serde_json::json!([]));
        let stop = &document["statistics"]["shared_execution"]["last_stop"];
        assert_eq!(stop["scope"], if source_stop { "source" } else { "world" });
        assert_eq!(stop["reason"], "work_limit");
    }
}

#[test]
fn footer_capacity_failure_cannot_replace_a_real_source_refusal() {
    let options = options("countermodel");
    let failure = crate::run_finalized_with_diagnostics(
        "#external a.".into(),
        &options,
        &mut io::sink(),
        &mut io::sink(),
        &Cancellation::default(),
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
    let mut sink = Vec::new();
    let retained = Document::new(&mut sink, true)
        .unwrap()
        .finish(result, expected.len() - 1)
        .unwrap_err();
    assert!(matches!(*retained.cause, RunError::FormulaAdmission(_)));
    assert_eq!(retained.cause.to_string(), original);
    assert!(retained.semantic().is_none());
    assert!(retained.summary_failure().is_some());
    assert_eq!(sink, PREFIX);
}
