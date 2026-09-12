//! Grounder selection is independent of hardware and preserves coverage.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use clap::Parser;
use zetesis_cli::{Backend, Completion, Grounder, Options, Report, RunError, run_with_diagnostics};
use zetesis_core::StaticError;
use zetesis_cpu::{Control, Stop};

fn options(arguments: &[&str]) -> Options {
    Options::try_parse_from(["zetesis"].into_iter().chain(arguments.iter().copied())).unwrap()
}

fn solve(source: &str, arguments: &[&str]) -> (Report, String, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        &options(arguments),
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    (
        report,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn models(output: &str) -> BTreeSet<&str> {
    output
        .lines()
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|pair| pair[0].starts_with("Answer:"))
        .map(|pair| pair[1])
        .collect()
}

#[test]
fn grounder_defaults_to_auto_and_parses_independently_of_backend() {
    assert_eq!(options(&[]).grounder, Grounder::Auto);
    for (name, grounder) in [
        ("auto", Grounder::Auto),
        ("lazy", Grounder::Lazy),
        ("eager", Grounder::Eager),
    ] {
        let selected = options(&["--backend", "cpu", "--grounder", name]);
        assert_eq!(
            (selected.backend, selected.grounder),
            (Backend::Cpu, grounder)
        );
    }
    assert!(Options::try_parse_from(["zetesis", "--grounding", "lazy"]).is_err());
}

#[test]
fn eager_and_lazy_cpu_return_the_same_models_and_complete_coverage() {
    let mut wide = String::new();
    for n in 0..70 {
        write!(wide, "p({n}).").unwrap();
    }
    for source in [
        "",
        ":-.",
        "a :- not a.",
        "a :- not b. b :- not a.",
        "node(a). node(b). {selected(X)} :- node(X). :- selected(a), selected(b).",
        "p(a). edge(a,b). edge(b,c). p(Y) :- p(X), edge(X,Y).",
        "p(a). p(b). q(X) :- p(X), X != b. r(X) :- p(X), X = a.",
        "p(\"a b\"). p(a). q(X) :- p(X), X != a.",
        &wide,
    ] {
        let (lazy, lazy_text, _) = solve(
            source,
            &["--backend", "cpu", "--grounder", "lazy", "--models", "0"],
        );
        let (eager, eager_text, diagnostics) = solve(
            source,
            &["--backend", "cpu", "--grounder", "eager", "--models", "0"],
        );
        assert_eq!(lazy.completion, Completion::Exhausted, "{source}");
        assert_eq!(eager.completion, lazy.completion, "{source}");
        assert_eq!((eager.models, eager.checked), (lazy.models, lazy.checked));
        assert_eq!(models(&eager_text), models(&lazy_text), "{source}");
        assert!(diagnostics.contains("requested=eager, effective=eager"));
        assert!(diagnostics.contains("static atoms="));
        assert!(diagnostics.contains("eager static closure scans"));
    }
}

#[test]
fn lazy_ignores_static_lowering_caps() {
    let (report, output, diagnostics) = solve(
        "{a}. {b}. {c}. {d}. {e}. {f}.",
        &[
            "--grounder",
            "lazy",
            "--models",
            "0",
            "--max-ground-rules",
            "0",
            "--max-substitutions",
            "0",
        ],
    );
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!((report.models, report.checked), (64, 64));
    assert_eq!(models(&output).len(), 64);
    #[cfg(feature = "gpu")]
    assert!(diagnostics.contains("no measured GPU crossover"));
    #[cfg(not(feature = "gpu"))]
    assert!(diagnostics.contains("GPU support was not compiled"));
    assert!(diagnostics.contains("requested=lazy, effective=lazy"));
    assert!(!diagnostics.contains("static atoms="));
}

#[test]
fn eager_cpu_enforces_every_static_lowering_cap() {
    for (flag, resource) in [
        ("--max-atoms", "atoms"),
        ("--max-ground-rules", "ground rules"),
        ("--max-substitutions", "substitutions"),
    ] {
        let mut output = Vec::new();
        let error = run_with_diagnostics(
            "a.".into(),
            &options(&["--backend", "cpu", "--grounder", "eager", flag, "0"]),
            &mut output,
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, RunError::Static(StaticError::LimitExceeded { resource: actual, .. }) if actual == resource)
        );
        assert!(output.is_empty());
    }
}

#[test]
fn eager_auto_never_silently_substitutes_lazy_when_lowering_is_refused() {
    let mut output = Vec::new();
    let error = run_with_diagnostics(
        "a.".into(),
        &options(&["--grounder", "eager", "--max-ground-rules", "0"]),
        &mut output,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::Static(_)));
    assert!(output.is_empty());
}

#[test]
fn lazy_device_selection_preserves_source_diagnostics() {
    let baseline = run_with_diagnostics(
        "invalid source!!!".into(),
        &options(&["--backend", "cpu", "--grounder", "lazy"]),
        &mut Vec::new(),
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    for name in ["gpu", "metal", "vulkan", "dx12", "gl", "nvidia"] {
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let selected = options(&["--backend", name, "--grounder", "lazy"]);
        let error = run_with_diagnostics(
            "invalid source!!!".into(),
            &selected,
            &mut output,
            &mut diagnostics,
            &Control::default(),
        )
        .unwrap_err();
        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&baseline)
        );
        assert_eq!(error.to_string(), baseline.to_string());
        assert!(output.is_empty());
        assert!(diagnostics.is_empty());
    }
}

#[test]
fn both_cpu_modes_keep_work_and_candidate_stops_incomplete() {
    for mode in ["lazy", "eager"] {
        let (report, output, _) = solve(
            "a.",
            &[
                "--backend",
                "cpu",
                "--grounder",
                mode,
                "--models",
                "0",
                "--max-work",
                "0",
            ],
        );
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!(
            report.interruption,
            Some(zetesis_cli::Interruption::Oracle(Stop::WorkLimit))
        );
        assert_eq!(report.models, 0);
        assert!(output.contains("Coverage: partial"));
        assert!(!output.contains("UNSATISFIABLE"));
        let (report, output, _) = solve(
            "{a}. {b}.",
            &[
                "--backend",
                "cpu",
                "--grounder",
                mode,
                "--models",
                "0",
                "--max-candidates",
                "3",
            ],
        );
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!(
            report.interruption,
            Some(zetesis_cli::Interruption::Oracle(Stop::CandidateLimit))
        );
        assert_eq!(report.models, 3);
        assert_eq!(models(&output).len(), 3);
    }
}

#[test]
fn eager_automatic_execution_retains_cpu() {
    let (report, output, diagnostics) = solve(
        "{a}. {b}. {c}. {d}. {e}. {f}.",
        &[
            "--grounder",
            "eager",
            "--models",
            "0",
            "--max-batch-bytes",
            "0",
        ],
    );
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!((report.models, report.checked), (64, 64));
    assert_eq!(models(&output).len(), 64);
    assert!(!diagnostics.contains("effective=lazy"));
    #[cfg(feature = "gpu")]
    assert!(diagnostics.contains("no measured GPU crossover"));
    #[cfg(not(feature = "gpu"))]
    assert!(diagnostics.contains("GPU support was not compiled"));
}
