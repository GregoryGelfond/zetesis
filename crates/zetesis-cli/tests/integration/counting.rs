//! Exhaustive counts keep full semantic identity and require complete coverage.

use std::{io, num::NonZeroUsize};

use clap::Parser;
use zetesis_cli::{
    Completion, CountRenderer, Invocation, Options, PreparedInput, PublicationConfig,
    PublicationMode, PublicationOutcome, RunError, publish_prepared,
};
use zetesis_cpu::Cancellation;
use zetesis_reference_support::formula;

fn config() -> PublicationConfig {
    let mut config = PublicationConfig {
        mode: PublicationMode::Count,
        ..PublicationConfig::default()
    };
    config.solve.workers = NonZeroUsize::MIN;
    config
}

fn count(source: &str, config: &PublicationConfig) -> (PublicationOutcome, Vec<u8>) {
    let admitted = formula(source);
    let mut renderer = CountRenderer::new(Vec::new());
    let outcome = publish_prepared(
        PreparedInput::formula(&admitted),
        config,
        &mut renderer,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap();
    (outcome, renderer.into_inner())
}

#[test]
fn count_ignores_display_and_projection_identity() {
    let mut config = config();
    config.observations.max_work = 0;
    config.solve.max_projection_entries = 0;
    let (outcome, bytes) = count("{p;q}. #project p/0. #show 1+1.", &config);
    assert_eq!(bytes, b"4\n");
    assert_eq!(outcome.semantic().complete_answer_count(), Some(4));
    assert!(outcome.semantic().projection().is_none());
    assert_eq!(outcome.publication().models(), 0);
    assert!(outcome.publication().summary());
}

#[test]
fn optimal_count_includes_unretained_ties() {
    let mut config = config();
    config.solve.models = 0;
    config.solve.max_optimal_models = 1;
    let (outcome, bytes) = count("{a;b;c}. #minimize{1:a}.", &config);
    assert_eq!(bytes, b"4\n");
    assert!(outcome.semantic().optimum_proved());
    assert_eq!(outcome.semantic().retained_models(), 1);
    assert_eq!(outcome.semantic().incumbent().unwrap().tied_models, 4);
    assert_eq!(outcome.semantic().complete_answer_count(), Some(4));
}

#[test]
fn unsatisfiable_and_empty_answer_have_different_counts() {
    for (source, expected) in [(":-.", 0), ("", 1), (":-. #minimize{1:a}.", 0)] {
        let (outcome, bytes) = count(source, &config());
        assert_eq!(bytes, format!("{expected}\n").as_bytes());
        assert_eq!(outcome.semantic().complete_answer_count(), Some(expected));
        assert_eq!(outcome.semantic().unsatisfiable(), expected == 0);
    }
}

#[test]
fn cancelled_count_publishes_no_total() {
    let admitted = formula("{a;b}.");
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut renderer = CountRenderer::new(Vec::new());
    let outcome = publish_prepared(
        PreparedInput::formula(&admitted),
        &config(),
        &mut renderer,
        &mut io::sink(),
        &cancellation,
    )
    .unwrap();
    assert_eq!(
        outcome.semantic().completion(),
        Some(Completion::Interrupted)
    );
    assert_eq!(outcome.semantic().complete_answer_count(), None);
    assert!(!outcome.publication().summary());
    assert!(renderer.into_inner().is_empty());
}

#[test]
fn resource_stopped_count_publishes_no_total() {
    let mut config = config();
    config.solve.max_optimal_models = 0;
    let (outcome, bytes) = count("{a;b}. #minimize{0:a}.", &config);
    assert_eq!(
        outcome.semantic().completion(),
        Some(Completion::Interrupted)
    );
    assert!(matches!(
        outcome.semantic().interruption(),
        Some(zetesis_cli::Interruption::Incumbent(
            zetesis_cli::OptimizationStop::Models
        ))
    ));
    assert_eq!(outcome.semantic().complete_answer_count(), None);
    assert!(!outcome.publication().summary());
    assert!(bytes.is_empty());
}

#[test]
fn failed_count_write_preserves_complete_semantic_evidence() {
    let admitted = formula("{a;b}.");
    let mut renderer = CountRenderer::new(zetesis_test_support::io::BoundedWriter::new(0));
    let failure = publish_prepared(
        PreparedInput::formula(&admitted),
        &config(),
        &mut renderer,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Output(_)));
    assert_eq!(failure.semantic().unwrap().complete_answer_count(), Some(4));
    assert!(!failure.publication().unwrap().summary());
    assert_eq!(failure.publication().unwrap().models(), 0);
    assert!(renderer.into_inner().bytes().is_empty());
}

#[test]
fn count_option_selects_the_typed_publication_mode() {
    let options = Options::try_parse_from(["zetesis", "--count"]).unwrap();
    assert_eq!(
        PublicationConfig::from(&options).mode,
        PublicationMode::Count
    );
    let Invocation::Solve(options) =
        Invocation::try_parse_from(["zetesis", "solve", "-", "--count", "--stats"]).unwrap()
    else {
        panic!("solve request")
    };
    assert!(options.count && options.stats);
}

#[test]
fn count_rejects_competing_output_and_answer_limits() {
    for arguments in [
        vec!["--json"],
        vec!["--answers", "2"],
        vec!["--all"],
        vec!["--models", "0"],
    ] {
        let error = Invocation::try_parse_from(
            ["zetesis", "solve", "-", "--count"]
                .into_iter()
                .chain(arguments),
        )
        .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}

#[test]
fn builtin_count_writes_only_the_integer_with_statistics_separate() {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--count",
        "--threads",
        "1",
        "--stats",
        "--color",
        "always",
    ])
    .unwrap();
    options.grounder = zetesis_cli::Grounder::Eager;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let outcome = zetesis_cli::run_finalized_with_diagnostics(
        "{a;b}. #show.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(output, b"4\n");
    assert_eq!(outcome.semantic().complete_answer_count(), Some(4));
    assert!(!diagnostics.is_empty());
}

#[test]
fn admission_failure_does_not_print_a_count() {
    let options = Options::try_parse_from(["zetesis", "--count", "--threads", "1"]).unwrap();
    let mut output = Vec::new();
    let result = zetesis_cli::run_finalized_with_diagnostics(
        "this is not a program".into(),
        &options,
        &mut output,
        &mut io::sink(),
        &Cancellation::default(),
    );
    assert!(result.is_err());
    assert!(output.is_empty());
}

fn process(source: &str, extra: &[&str]) -> std::process::Output {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["solve", "-", "--count", "--threads", "1"])
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn count_process_returns_only_complete_totals() {
    for (source, expected) in [("{a;b}. #show.", "4\n"), (":-.", "0\n")] {
        let output = process(source, &[]);
        assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn count_process_deadline_has_no_total() {
    let output = process("{a;b}.", &["--time-limit", "0"]);
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic.contains("answer count unavailable:"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("deadline"), "{diagnostic}");
}

#[test]
fn counting_uses_the_final_selection_in_either_builder_order() {
    use zetesis_cli::{AnswerSelection, Session};
    let admitted = formula("{a;b;c}. #minimize{1:a}.");
    for selection in [AnswerSelection::All, AnswerSelection::Optimal] {
        for counting_first in [true, false] {
            let mut config = config().solve;
            config.max_optimal_models = 1;
            let builder = Session::builder(
                PreparedInput::formula(&admitted),
                config,
                Cancellation::default(),
            );
            let mut session = if counting_first {
                builder.counting().selection(selection)
            } else {
                builder.selection(selection).counting()
            }
            .start()
            .unwrap();
            for answer in session.by_ref() {
                answer.unwrap();
            }
            let outcome = session.outcome().unwrap();
            assert_eq!(outcome.completion(), Some(Completion::Exhausted));
            assert_eq!(
                outcome.complete_answer_count(),
                Some(if selection == AnswerSelection::All {
                    8
                } else {
                    4
                })
            );
        }
    }
}

#[test]
fn later_projection_cannot_change_a_count_request() {
    let admitted = formula("{a;b}. #project a/0.");
    let mut session = zetesis_cli::Session::builder(
        PreparedInput::formula(&admitted),
        config().solve,
        Cancellation::default(),
    )
    .counting()
    .projected(zetesis_solve::ProjectionLimits {
        max_keys: 0,
        max_bytes: 0,
        max_work: 0,
    })
    .start()
    .unwrap();
    for answer in session.by_ref() {
        answer.unwrap();
    }
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.complete_answer_count(), Some(4));
    assert!(outcome.projection().is_none());
}

#[test]
fn failed_stop_diagnostic_keeps_the_interruption_without_a_count() {
    let admitted = formula("{a;b}.");
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut renderer = CountRenderer::new(Vec::new());
    let mut diagnostics = zetesis_test_support::io::BoundedWriter::new(0);
    let failure = publish_prepared(
        PreparedInput::formula(&admitted),
        &config(),
        &mut renderer,
        &mut diagnostics,
        &cancellation,
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Output(_)));
    assert_eq!(
        failure.semantic().unwrap().completion(),
        Some(Completion::Interrupted)
    );
    assert_eq!(failure.semantic().unwrap().complete_answer_count(), None);
    assert!(!failure.publication().unwrap().summary());
    assert!(renderer.into_inner().is_empty());
}
