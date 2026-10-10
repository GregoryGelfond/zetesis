//! Display-only queries preserve full enumeration and never emit partial answers.

use clap::Parser;
use zetesis_cli::{Completion, Options, Report, RunError, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_themelios::observation::{ErrorKind, Resource};
use zetesis_themelios::{AdmissionFailure, ExpansionFailure, ProfileFeature};

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
fn bounded(
    source: &str,
    config: &zetesis_cli::PublicationConfig,
) -> (Result<Report, RunError>, String, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = crate::support::prepared::human(
        source,
        config,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .map_err(|failure| *failure.cause);
    (
        result,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}
#[test]
fn auto_observation_uses_original_models_and_separate_output_channels() {
    let (result, output, diagnostics) = solve("a. #show a.", &["--stats"]);
    assert_eq!(result.unwrap().models, 1);
    assert!(output.contains("Answer: 1\na a\n"), "{output}");
    assert!(diagnostics.contains("oracle: Ferraris reduct membership"));
    let (result, output, _) = solve(
        "p(1;2). #show. #show f(g(X),(X,)):p(X).",
        &["--models", "0"],
    );
    assert_eq!(result.unwrap().completion, Completion::Exhausted);
    assert!(
        output.contains("Answer: 1\nf(g(1),(1,)) f(g(2),(2,))\n"),
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
        for bounds in [0, 10_000_000] {
            let mut config = crate::support::prepared::config(&[]);
            config.solve.max_objective_bound_work = bounds;
            let (result, output, _) = bounded(source, &config);
            let report = result.unwrap();
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, 4);
            assert_eq!(output.lines().filter(|line| *line == "x").count(), 4);
            assert!(crate::support::human::exhausted(&output));
            if source.contains("@1") {
                assert!(output.contains("OPTIMUM FOUND"));
                assert_eq!(output.matches("Optimization: 0\n").count(), 4);
            }
        }
    }
}
#[test]
fn closure_observation_refuses_the_unsupported_directive() {
    let (result, output, _) = solve(
        include_str!("../fixtures/observations/conditional.lp"),
        &["--oracle", "closure"],
    );
    assert!(
        matches!(
            result,
            Err(RunError::Expansion(ExpansionFailure::Admission(
                AdmissionFailure::Profile {
                    feature: ProfileFeature::ShowTerm,
                    ..
                }
            )))
        ),
        "{result:?}"
    );
    assert!(
        crate::support::human::preamble(&output),
        "no answer from unsupported route: {output}"
    );
}

#[test]
fn lazy_observation_evaluates_conditional_terms() {
    let (result, output, _) = solve(
        include_str!("../fixtures/observations/conditional.lp"),
        &["--backend", "cpu", "--grounder", "lazy", "--models", "0"],
    );
    let report = result.unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    assert!(report.hybrid_execution.is_some());
    assert!(output.contains("Answer: 1\np(1) f(1)\n"), "{output}");
}
#[test]
fn observation_failures_emit_no_partial_answer_or_false_completion() {
    for resource in [
        Resource::Work,
        Resource::Bindings,
        Resource::Terms,
        Resource::OutputBytes,
    ] {
        for source in ["a. #show a.", "a. #show a. #minimize{0:a}."] {
            let mut config = crate::support::prepared::config(&[]);
            match resource {
                Resource::Work => config.observations.max_work = 0,
                Resource::Bindings => config.observations.max_bindings = 0,
                Resource::Terms => config.observations.max_terms = 0,
                Resource::OutputBytes => config.observations.max_output_bytes = 0,
                _ => unreachable!("the test enumerates these four resource boundaries"),
            }
            let (result, output, _) = bounded(source, &config);
            assert!(
                matches!(result, Err(RunError::Observation(error)) if matches!(error.kind(), ErrorKind::Limit { resource: actual, .. } if *actual == resource)),
                "{resource:?}"
            );
            assert!(
                crate::support::human::preamble(&output),
                "{resource:?}: {output}"
            );
        }
    }
    let mut config = crate::support::prepared::config(&[]);
    config.observations.max_output_bytes = 20;
    let (result, output, _) = bounded("a.b.c.d.e.f.g.h. #show x.", &config);
    assert!(matches!(
        result,
        Err(RunError::ObservationOutputLimit { .. })
    ));
    assert!(crate::support::human::preamble(&output));
}
#[test]
fn plain_models_do_not_consume_observation_work() {
    let answer = "Answer: 1\na\n";
    let mut config = crate::support::prepared::config(&[]);
    config.observations.max_work = 0;
    config.observations.max_output_bytes = answer.len();
    let (result, output, _) = bounded("a.", &config);
    assert_eq!(result.unwrap().models, 1);
    assert!(output.contains("Answer: 1\na\n"));
}
