//! Grounder selection is independent of hardware and preserves coverage.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::support::options::plain as options;
use crate::support::prepared;
use clap::Parser;
use zetesis_cli::{Backend, Completion, Grounder, Options, Report, RunError, run_with_diagnostics};
use zetesis_core::StaticError;
use zetesis_cpu::{Cancellation, Stop};

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
    let result = prepared::relational(
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
        assert!(diagnostics.contains("requested=eager, effective=eager"));
        assert!(diagnostics.contains("static atoms="));
        assert!(diagnostics.contains("eager static closure scans"));
    }
}

#[test]
fn lazy_ignores_static_lowering_caps() {
    let mut config = prepared::config(&["--grounder", "lazy"]);
    config.solve.max_ground_rules = 0;
    config.solve.max_substitutions = 0;
    let (report, output) = typed("{a}. {b}. {c}. {d}. {e}. {f}.", &config).unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!((report.models, report.checked), (64, 64));
    assert_eq!(models(&output).len(), 64);
    assert!(matches!(
        report.closure_execution.unwrap().route,
        zetesis_cli::ClosureRoute::Lazy(_)
    ));
}

#[test]
fn eager_cpu_enforces_every_static_lowering_cap() {
    for resource in ["atoms", "ground rules", "substitutions"] {
        let mut config = prepared::config(&["--grounder", "eager"]);
        match resource {
            "atoms" => config.solve.max_atoms = 0,
            "ground rules" => config.solve.max_ground_rules = 0,
            "substitutions" => config.solve.max_substitutions = 0,
            _ => unreachable!(),
        }
        let error = typed("a.", &config).unwrap_err();
        assert!(
            matches!(error, RunError::Static(StaticError::LimitExceeded { resource: actual, .. }) if actual == resource)
        );
    }
}

#[test]
fn eager_auto_never_silently_substitutes_lazy_when_lowering_is_refused() {
    let mut config = prepared::config(&["--grounder", "eager"]);
    config.solve.max_ground_rules = 0;
    assert!(matches!(typed("a.", &config), Err(RunError::Static(_))));
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
        assert_eq!(
            report.interruption,
            Some(zetesis_cli::Interruption::Oracle(Stop::WorkLimit))
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
            Some(zetesis_cli::Interruption::Oracle(Stop::CandidateLimit))
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
    assert_eq!(
        report.closure_execution.unwrap().route,
        zetesis_cli::ClosureRoute::Eager
    );
    assert!(report.lazy_execution.is_none());
}
