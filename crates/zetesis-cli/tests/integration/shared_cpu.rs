//! Ordinary consumers can explicitly select shared CPU source rounds.

use std::io::{self, Write};

use clap::Parser;
use zetesis_cli::{Completion, Options, RunError, SourceBatching, run_with_diagnostics};
use zetesis_cpu::{Cancellation, Stop};

fn options(extra: &[&str]) -> Options {
    Options::try_parse_from(
        ["zetesis", "--models", "0", "--stats", "--json"]
            .into_iter()
            .chain(extra.iter().copied()),
    )
    .unwrap()
}

fn solve(source: &str, extra: &[&str]) -> (zetesis_cli::Report, serde_json::Value, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        &options(extra),
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    (
        report,
        serde_json::from_slice(&output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

#[test]
fn shared_rounds_preserve_full_ordered_answer_records() {
    let source = "d(0..3). {p(X)}:-d(X). pair(X,Y):-p(X),p(Y). :-pair(0,1).";
    let (reference, expected, _) = solve(source, &["--backend", "cpu"]);
    assert_eq!(reference.models, 12);
    assert!(reference.shared_execution.is_none());
    for selection in ["union", "worlds"] {
        let (report, actual, _) = solve(
            source,
            &["--backend", "cpu", "--source-batching", selection],
        );
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.checked, reference.checked);
        assert_eq!(actual["models"], expected["models"]);
        let stats = report.shared_execution.unwrap();
        assert_eq!(stats.submitted_candidates, reference.checked);
        assert_eq!(stats.completed_candidates, stats.submitted_candidates);
        assert_eq!(stats.stopped_candidates, 0);
        assert_eq!(stats.queued_results, 0);
        assert!(stats.source_work > 0);
        assert!(stats.world_work >= stats.world_instances);
    }
}

#[test]
fn explicit_shared_policy_runs_on_the_cpu() {
    let (report, json, diagnostics) = solve("{a}. {b}.", &["--source-batching", "worlds"]);
    assert_eq!(report.models, 4);
    assert!(report.lazy_execution.is_none());
    assert!(report.formula_execution.is_none());
    assert_eq!(
        json["statistics"]["shared_execution"]["requested_backend"],
        "cpu"
    );
    assert_eq!(json["statistics"]["shared_execution"]["backend"], "cpu");
    assert_eq!(
        json["statistics"]["shared_execution"]["source_batching"],
        "worlds"
    );
    assert!(json["statistics"]["lazy_execution"].is_null());
    assert!(diagnostics.contains("Backend: cpu (shared worlds source rounds"));
    assert!(diagnostics.contains("source and world work reported separately"));
}

#[test]
fn requested_model_stop_retains_complete_queued_checks() {
    let mut selected = options(&["--source-batching", "union"]);
    selected.models = 2;
    let report = run_with_diagnostics(
        "{a}. {b}.".into(),
        &selected,
        &mut Vec::new(),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::RequestedModels);
    assert_eq!(report.models, 2);
    let stats = report.shared_execution.unwrap();
    assert_eq!(stats.submitted_candidates, 4);
    assert_eq!(stats.completed_candidates, 4);
    assert_eq!(stats.queued_results, 2);
    assert_eq!(stats.stopped_candidates, 0);
}

#[test]
fn world_stop_never_publishes_partial_batch_checks() {
    let (report, json, _) = solve("a.", &["--source-batching", "union", "--max-work", "0"]);
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(report.models, 0);
    let stats = report.shared_execution.unwrap();
    assert_eq!(stats.submitted_candidates, 1);
    assert_eq!(stats.completed_candidates, 0);
    assert_eq!(stats.stopped_candidates, 1);
    assert_eq!(stats.queued_results, 0);
    assert_eq!(stats.world_work, 0);
    assert!(stats.source_work > 0);
    assert_eq!(
        stats.last_stop,
        Some(zetesis_cpu::lazy::shared::Cause::World {
            index: 0,
            stop: Stop::WorkLimit
        })
    );
    assert_eq!(
        json["statistics"]["shared_execution"]["last_stop"]["scope"],
        "world"
    );
    assert_eq!(
        json["statistics"]["shared_execution"]["last_stop"]["index"],
        0
    );
    assert_eq!(json["outcome"]["status"], "incomplete");
}

#[test]
fn source_stop_has_separate_progress_from_world_work() {
    let (report, json, _) = solve(
        "a.",
        &["--source-batching", "worlds", "--max-source-work", "0"],
    );
    let stats = report.shared_execution.unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(
        stats.last_stop,
        Some(zetesis_cpu::lazy::shared::Cause::Source(Stop::WorkLimit))
    );
    assert_eq!(stats.world_work, 0);
    assert_eq!(stats.source_work, 0);
    assert_eq!(stats.completed_candidates, 0);
    assert_eq!(
        json["statistics"]["shared_execution"]["last_stop"]["scope"],
        "source"
    );
}

#[test]
fn later_batch_stop_preserves_the_complete_prefix() {
    // The empty seed checks two one-gate records in one round (four work).
    // Nonempty seeds need a further round; its first work step exceeds this cap.
    let (report, json, _) = solve(
        "{a}. {b}.",
        &["--source-batching", "union", "--max-work", "4"],
    );
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(report.models, 1);
    assert_eq!(report.checked, 2); // One complete result, then one batch stop marker.
    let stats = report.shared_execution.unwrap();
    assert_eq!(stats.batches, 2);
    assert_eq!(stats.submitted_candidates, 4);
    assert_eq!(stats.completed_candidates, 1);
    assert_eq!(stats.stopped_candidates, 3);
    assert_eq!(stats.queued_results, 0);
    assert_eq!(stats.world_work, 16);
    assert_eq!(
        stats.last_stop,
        Some(zetesis_cpu::lazy::shared::Cause::World {
            index: 0,
            stop: Stop::WorkLimit
        })
    );
    assert_eq!(json["models"].as_array().unwrap().len(), 1);
    assert_eq!(
        json["statistics"]["shared_execution"]["stopped_candidates"],
        3
    );
}

#[test]
fn shared_host_ceiling_remains_a_resource_interruption() {
    let (report, _, _) = solve(
        "a.",
        &["--source-batching", "union", "--max-batch-bytes", "1"],
    );
    assert_eq!(report.completion, Completion::Interrupted);
    let stats = report.shared_execution.unwrap();
    assert_eq!(
        stats.last_stop,
        Some(zetesis_cpu::lazy::shared::Cause::Source(Stop::Allocation))
    );
    assert_eq!(stats.completed_candidates, 0);
    assert_eq!(stats.source_work, 0);
}

#[test]
fn incompatible_execution_is_refused_as_route_capability() {
    for extra in [
        vec!["--source-batching", "union", "--backend", "metal"],
        vec!["--source-batching", "worlds", "--grounder", "eager"],
        vec!["--source-batching", "union", "--oracle", "countermodel"],
    ] {
        let error = run_with_diagnostics(
            "a.".into(),
            &options(&extra),
            &mut Vec::new(),
            &mut Vec::new(),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(matches!(error, RunError::UnsupportedSourceBatching));
    }
}

#[test]
fn automatic_formula_admission_cannot_ignore_shared_policy() {
    let error = run_with_diagnostics(
        "a|b.".into(),
        &options(&["--source-batching", "worlds"]),
        &mut Vec::new(),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::UnsupportedSourceBatching));
}

#[test]
fn cancelled_solve_does_not_initialize_shared_execution() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let report = run_with_diagnostics(
        "a.".into(),
        &options(&["--source-batching", "union"]),
        &mut Vec::new(),
        &mut Vec::new(),
        &cancellation,
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(report.shared_execution.is_none());
}

struct RefuseOutput;
impl Write for RefuseOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("consumer refused output"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn output_failure_retains_completed_shared_evidence() {
    let mut selected = options(&["--source-batching", "worlds"]);
    selected.json = false;
    let error = zetesis_cli::run_detailed_with_diagnostics(
        "a.".into(),
        &selected,
        &mut RefuseOutput,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(*error.cause, RunError::Output(_)));
    let partial = error.partial_report.unwrap();
    assert_eq!(partial.published_models, 0);
    assert_eq!(partial.verified_models, 1);
    let stats = partial.shared_execution.unwrap();
    assert_eq!(stats.completed_candidates, 1);
    assert_eq!(stats.stopped_candidates, 0);
}

#[test]
fn ordinary_help_keeps_source_batching_advanced() {
    let short = Options::try_parse_from(["zetesis", "--help"])
        .unwrap_err()
        .to_string();
    let long = Options::try_parse_from(["zetesis", "--help-all"])
        .unwrap_err()
        .to_string();
    assert!(!short.contains("--source-batching"));
    assert!(!short.contains("--max-source-work"));
    assert!(long.contains("--source-batching"));
    assert!(long.contains("--max-source-work"));
    assert_eq!(
        Options::try_parse_from(["zetesis"])
            .unwrap()
            .source_batching,
        SourceBatching::Independent
    );
}

#[test]
fn prepared_relational_sessions_expose_shared_evidence() {
    let admitted = zetesis_themelios::admit_extended(
        "{a}. {b}.".into(),
        zetesis_themelios::AdmissionOptions::default(),
        zetesis_themelios::ExpansionLimits::default(),
    )
    .unwrap();
    let mut session = zetesis_cli::Session::new(
        zetesis_cli::PreparedInput::admitted(&admitted),
        zetesis_cli::SolveConfig {
            source_batching: SourceBatching::Worlds,
            models: 0,
            ..Default::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert_eq!(session.by_ref().map(Result::unwrap).count(), 4);
    let outcome = session.stop();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(outcome.verified_models(), 4);
    let statistics = outcome.shared_execution().unwrap();
    assert_eq!(statistics.completed_candidates, 4);
    assert_eq!(statistics.queued_results, 0);
}

#[test]
fn prepared_formula_cannot_ignore_shared_source_selection() {
    let admitted = zetesis_themelios::admit_formula(
        "a|b.".into(),
        zetesis_themelios::AdmissionOptions::default(),
        zetesis_themelios::ExpansionLimits::default(),
        zetesis_themelios::FormulaLimits::default(),
    )
    .unwrap();
    let Err(error) = zetesis_cli::Session::new(
        zetesis_cli::PreparedInput::formula(&admitted),
        zetesis_cli::SolveConfig {
            source_batching: SourceBatching::Union,
            ..Default::default()
        },
        Cancellation::default(),
    ) else {
        panic!("formula route cannot honor shared relational traversal");
    };
    assert!(matches!(
        *error.cause,
        zetesis_cli::SolveError::UnsupportedSourceBatching
    ));
}
