//! Display-only queries preserve full enumeration and never emit partial answers.

use clap::Parser;
use zetesis_cli::{Completion, Options, Report, RunError, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_themelios::observation::{ErrorKind, Resource};

fn solve(source: &str, arguments: &[&str]) -> (Result<Report, RunError>, String, String) {
    let options =
        Options::try_parse_from(["zetesis"].into_iter().chain(arguments.iter().copied())).unwrap();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = run_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    );
    (
        result,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}
#[test]
fn auto_observation_uses_original_models_and_separate_output_channels() {
    let (result, output, diagnostics) = solve("a. #show a.", &[]);
    assert_eq!(result.unwrap().models, 1);
    assert!(output.starts_with("Answer: 1\na a\n"), "{output}");
    assert!(diagnostics.contains("oracle: Ferraris reduct membership"));
    let (result, output, _) = solve(
        "p(1;2). #show. #show f(g(X),(X,)):p(X).",
        &["--models", "0"],
    );
    assert_eq!(result.unwrap().completion, Completion::Exhausted);
    assert!(
        output.starts_with("Answer: 1\nf(g(1),(1,)) f(g(2),(2,))\n"),
        "{output}"
    );
}
#[test]
fn hidden_full_models_and_every_optimal_tie_keep_their_multiplicity() {
    for source in [
        "{a;b}. #show. #show x.",
        "{a;b}. #show. #show x. #minimize{0@1,k:a}.",
        "{a;b}. #show. #show x. #maximize{0@1,k:a}.",
    ] {
        for bounds in ["0", "10000000"] {
            let (result, output, _) = solve(
                source,
                &["--models", "0", "--max-objective-bound-work", bounds],
            );
            let report = result.unwrap();
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, 4);
            assert_eq!(output.lines().filter(|line| *line == "x").count(), 4);
            assert!(output.contains("Coverage: exhausted"));
            if source.contains("@1") {
                assert!(output.contains("OPTIMUM FOUND"));
                assert_eq!(output.matches("Optimization: 0\n").count(), 4);
            }
        }
    }
}
#[test]
fn incompatible_observation_routes_are_refused_without_fallback() {
    let source = "p(1). #show f(X):p(X).";
    for arguments in [
        vec!["--oracle", "closure"],
        vec!["--backend", "metal", "--grounder", "lazy"],
        vec!["--backend", "nvidia", "--grounder", "lazy"],
    ] {
        let (result, output, _) = solve(source, &arguments);
        assert!(result.is_err(), "{arguments:?}");
        assert!(
            output.is_empty(),
            "no answer from unsupported route: {output}"
        );
    }
}
#[test]
fn observation_failures_emit_no_partial_answer_or_false_completion() {
    for (flag, value, resource) in [
        ("--max-observation-work", "0", Resource::Work),
        ("--max-observation-bindings", "0", Resource::Bindings),
        ("--max-observation-terms", "0", Resource::Terms),
        ("--max-observation-bytes", "0", Resource::OutputBytes),
    ] {
        for source in ["a. #show a.", "a. #show a. #minimize{0:a}."] {
            let (result, output, _) = solve(source, &[flag, value]);
            assert!(
                matches!(result, Err(RunError::Observation(error)) if matches!(error.kind(), ErrorKind::Limit { resource: actual, .. } if *actual == resource)),
                "{flag}"
            );
            assert!(output.is_empty(), "{flag}: {output}");
        }
    }
    let (result, output, _) = solve(
        "a.b.c.d.e.f.g.h. #show x.",
        &["--max-observation-bytes", "20"],
    );
    assert!(matches!(
        result,
        Err(RunError::ObservationOutputLimit { .. })
    ));
    assert!(output.is_empty());
}
#[test]
fn plain_models_do_not_consume_observation_work() {
    let answer = "Answer: 1\na\n";
    let record_limit = answer.len().to_string();
    let (result, output, _) = solve(
        "a.",
        &[
            "--backend",
            "cpu",
            "--max-observation-work",
            "0",
            "--max-observation-bytes",
            &record_limit,
        ],
    );
    assert_eq!(result.unwrap().models, 1);
    assert!(output.starts_with("Answer: 1\na\n"));
}
