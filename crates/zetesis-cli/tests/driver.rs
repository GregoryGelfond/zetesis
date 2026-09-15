//! User-visible model delivery, coverage, refusals, and output failures.
use clap::Parser;
use std::io;
use zetesis_cli::{Completion, Options, RunError, run};
use zetesis_cpu::{Control, Stop};

fn options(extra: &[&str]) -> Options {
    Options::try_parse_from(["zetesis"].into_iter().chain(extra.iter().copied())).unwrap()
}
fn solve(source: &str, extra: &[&str]) -> (zetesis_cli::Report, String) {
    let mut output = Vec::new();
    let report = run(
        source.into(),
        &options(extra),
        &mut output,
        &Control::default(),
    )
    .unwrap();
    (report, String::from_utf8(output).unwrap())
}

#[test]
fn exhaustive_models_are_streamed_once_with_coverage() {
    let (report, text) = solve("a :- not b. b :- not a.", &["--models", "0"]);
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(
        (report.models, report.checked, report.discovered_gate_atoms),
        (2, 4, 2)
    );
    assert!(text.contains("Answer: 1\na\nAnswer: 2\nb\nSATISFIABLE\nCoverage: exhausted"));
}

#[test]
fn first_positive_model_does_not_expand_the_carrier() {
    let (report, text) = solve("node(a). {chosen(X)} :- node(X).", &[]);
    assert_eq!(report.completion, Completion::RequestedModels);
    assert_eq!(
        (report.models, report.checked, report.discovered_gate_atoms),
        (1, 1, 0)
    );
    assert!(text.contains("Answer: 1\nnode(a)\n"));
}

#[test]
fn choices_and_constraints_have_three_models() {
    let (report, _) = solve(
        "node(a). node(b). {selected(X)} :- node(X). :- selected(a), selected(b).",
        &["--models", "0"],
    );
    // The witnessed positive constraint skips the impossible two-selection seed.
    assert_eq!((report.models, report.checked), (3, 3));
    assert_eq!(report.completion, Completion::Exhausted);
}

#[test]
fn unsat_requires_exhaustion_and_limits_preserve_incomplete_status() {
    let (report, text) = solve("a :- not a.", &["--models", "0"]);
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 0);
    assert!(text.contains("UNSATISFIABLE"));
    let (limited, text) = solve("a :- not a.", &["--models", "0", "--max-candidates", "1"]);
    assert_eq!(limited.completion, Completion::Interrupted);
    assert_eq!(
        limited.interruption,
        Some(zetesis_cli::Interruption::Oracle(Stop::CandidateLimit))
    );
    assert!(!text.contains("UNSATISFIABLE"));
}

#[test]
fn partial_batch_limit_does_not_discard_completed_models() {
    let (report, text) = solve("{a}. {b}.", &["--models", "0", "--max-candidates", "3"]);
    assert_eq!((report.models, report.checked), (3, 3));
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(text.contains("INCOMPLETE"));
}

#[test]
fn cancelled_invocation_never_claims_unsat() {
    let control = Control::default();
    control.cancel();
    let mut output = Vec::new();
    let report = run(String::new(), &options(&[]), &mut output, &control).unwrap();
    assert_eq!(
        report.interruption,
        Some(zetesis_cli::Interruption::Preparation(Stop::Cancelled))
    );
    assert_eq!(report.checked, 0);
    assert!(!String::from_utf8(output).unwrap().contains("UNSATISFIABLE"));
}

#[test]
fn admission_errors_keep_source_positions() {
    let source = "#external a.";
    let error = run(
        source.into(),
        &options(&[]),
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    let RunError::FormulaAdmission(ref refusal) = error else {
        panic!("expected a typed formula admission refusal")
    };
    let diagnostics = refusal.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    let location = diagnostics[0].primary().location;
    assert_eq!(location.source.get(), 0);
    assert_eq!(location.span.start().get(), 0);
    assert_eq!(
        usize::try_from(location.span.end().get()).unwrap(),
        source.len()
    );
    let message = error.to_string();
    assert!(message.contains("bytes "));
    assert!(message.contains("source profile does not admit"));
}

#[test]
fn output_failure_is_propagated() {
    struct Broken;
    impl io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let error = run("a.".into(), &options(&[]), &mut Broken, &Control::default()).unwrap_err();
    assert!(matches!(error, RunError::Output(_)));
}

#[test]
fn cli_rejects_zero_workers_and_batches() {
    assert!(Options::try_parse_from(["zetesis", "--workers", "0"]).is_err());
    assert!(Options::try_parse_from(["zetesis", "--batch-size", "0"]).is_err());
}

#[test]
fn kr_domains_rule_excerpts_complete_with_known_results() {
    for (source, models, seeds) in [
        (
            include_str!("fixtures/kr-domains/accepted/shortest-path-reachable.lp"),
            1,
            16,
        ),
        (
            include_str!("fixtures/kr-domains/accepted/shortest-path-disconnected-cycle-unsat.lp"),
            0,
            16,
        ),
        (
            include_str!("fixtures/kr-domains/accepted/task-allocation-projections.lp"),
            1,
            1,
        ),
    ] {
        let (report, _) = solve(source, &["--models", "0"]);
        assert_eq!((report.models, report.checked), (models, seeds), "{source}");
        assert_eq!(report.completion, Completion::Exhausted);
    }
}

#[test]
fn unchanged_shortest_path_encoding_without_an_instance_completes() {
    let source = include_str!("fixtures/kr-domains/refused/shortest-path-variant-01.lp");
    let (report, _) = solve(source, &["--models", "0"]);
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    assert!(report.countermodel_statistics.is_some());
    assert_eq!(report.optimization.unwrap().score.costs(), &[(2, 0)]);
}

#[cfg(not(feature = "gpu"))]
#[test]
fn gpu_request_never_falls_back_to_cpu() {
    let error = run(
        "a.".into(),
        &options(&["--backend", "gpu"]),
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::BackendUnavailable));
}
