//! Grounder selection is independent of hardware and preserves coverage.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::support::options::plain as options;
use crate::support::prepared;
use clap::Parser;
use zetesis_algebra::Cancellation;
use zetesis_cli::{Backend, Completion, Grounder, Options, Report, RunError, run_with_diagnostics};

fn solve(source: &str, arguments: &[&str]) -> (Report, String, String) {
    let mut configured = options(arguments);
    configured.stats = true;
    configured.statistics_view = zetesis_cli::StatisticsView::Records;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        &configured,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    (
        report,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn typed(
    source: &str,
    config: &zetesis_cli::PublicationConfig,
) -> Result<(Report, String), RunError> {
    let mut output = Vec::new();
    let result = prepared::formula(
        source,
        config,
        &mut zetesis_cli::HumanRenderer::new(
            &mut output,
            zetesis_cli::ColorMode::Never,
            config.observations.max_output_bytes,
        ),
        &mut Vec::new(),
        &Cancellation::default(),
    );
    if result.is_err() {
        assert!(!std::str::from_utf8(&output).unwrap().contains("Answer:"));
    }
    let report = result.map_err(|failure| *failure.cause)?;
    Ok((report, String::from_utf8(output).unwrap()))
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
        assert!(
            diagnostics.contains("grounder: eager (requested eager)"),
            "{diagnostics}"
        );
    }
}

#[test]
fn lazy_device_selection_preserves_source_diagnostics() {
    let baseline = run_with_diagnostics(
        "invalid source!!!".into(),
        &options(&["--backend", "cpu", "--grounder", "lazy"]),
        &mut Vec::new(),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    for name in ["gpu", "metal", "vulkan"] {
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let selected = options(&["--backend", name, "--grounder", "lazy"]);
        let error = run_with_diagnostics(
            "invalid source!!!".into(),
            &selected,
            &mut output,
            &mut diagnostics,
            &Cancellation::default(),
        )
        .unwrap_err();
        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&baseline)
        );
        assert_eq!(error.to_string(), baseline.to_string());
        assert!(!std::str::from_utf8(&output).unwrap().contains("Answer:"));
        assert!(diagnostics.is_empty());
    }
}

#[test]
fn both_cpu_modes_keep_work_and_candidate_stops_incomplete() {
    for mode in ["lazy", "eager"] {
        let mut config = prepared::config(&["--grounder", mode]);
        config.solve.max_work = 0;
        let (report, output) = typed("a.", &config).unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        // Admission's source verification charges the solve's work and stops
        // first.
        assert_eq!(
            report.interruption,
            Some(zetesis_cli::Interruption::Countermodel(
                zetesis_stable::Incomplete::Verification(zetesis_algebra::Stop::WorkLimit)
            ))
        );
        assert_eq!(report.models, 0);
        assert!(output.contains("Models: 0 (search incomplete)"));
        assert!(!output.contains("UNSATISFIABLE"));
        let mut config = prepared::config(&["--grounder", mode]);
        config.solve.max_candidates = 3;
        let (report, output) = typed("{a}. {b}.", &config).unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!(
            report.interruption,
            Some(zetesis_cli::Interruption::Countermodel(
                zetesis_stable::Incomplete::CandidateLimit
            ))
        );
        assert_eq!(report.models, 3);
        assert_eq!(models(&output).len(), 3);
    }
}

#[test]
fn eager_automatic_execution_retains_cpu() {
    let mut config = prepared::config(&["--grounder", "eager"]);
    config.solve.max_batch_bytes = 0;
    let (report, output) = typed("{a}. {b}. {c}. {d}. {e}. {f}.", &config).unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!((report.models, report.checked), (64, 64));
    assert_eq!(models(&output).len(), 64);
    assert!(report.countermodel_statistics.is_some());
    assert!(report.formula_execution.is_none());
    assert!(report.hybrid_execution.is_none());
}
