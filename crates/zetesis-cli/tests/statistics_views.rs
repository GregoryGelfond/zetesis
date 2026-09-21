//! Human statistics derive from receipts, independently of requested hardware.

use std::{io, num::NonZeroUsize};

use clap::Parser;
use zetesis_cli::{
    Backend, ColorMode, Options, PublicationOutcome, SemanticOutcome, SolveConfig,
    SolveMeasurements, run_finalized_with_diagnostics, statistics_view::Statistics,
};
use zetesis_cpu::Control;
use zetesis_presentation::Layout;

fn solve(source: &str, arguments: &[&str], stats: bool) -> (Options, PublicationOutcome) {
    let mut options = Options::try_parse_from(
        ["zetesis", "--workers", "1", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap();
    options.stats = stats;
    let outcome = run_finalized_with_diagnostics(
        source.into(),
        &options,
        &mut io::sink(),
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap();
    (options, outcome)
}

fn render(config: &SolveConfig, semantic: Option<&SemanticOutcome>) -> String {
    let timings = SolveMeasurements::new(true).snapshot().unwrap();
    let view = Statistics {
        requested: config,
        timings: &timings,
        semantic,
        publication: None,
        failed: false,
    };
    let mut bytes = Vec::new();
    view.write_human(
        &mut bytes,
        Layout::new(NonZeroUsize::new(256).unwrap(), ColorMode::Never),
    )
    .unwrap();
    String::from_utf8(bytes).unwrap()
}

fn value<'a>(text: &'a str, label: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.strip_prefix(label))
        .unwrap()
        .trim()
}

#[test]
fn formula_work_preserves_the_retained_count() {
    let (options, outcome) = solve("a|b. a:-b. b:-a.", &["--oracle", "countermodel"], true);
    let semantic = outcome.semantic();
    let statistics = semantic.countermodel_statistics().unwrap();
    assert!(statistics.search.work > 0);
    let text = render(&SolveConfig::from(&options), Some(semantic));
    assert_eq!(
        value(&text, "Formula search work")
            .split_whitespace()
            .next()
            .unwrap(),
        statistics.search.work.to_string()
    );
}

#[test]
fn recorded_route_is_independent_of_automatic_policy() {
    let (options, outcome) = solve("a|b.", &["--oracle", "countermodel"], true);
    let text = render(&SolveConfig::from(&options), Some(outcome.semantic()));
    assert!(value(&text, "Recorded execution").starts_with("CPU native formula (general reduct)"));
    assert!(value(&text, "Requested device").starts_with("auto"));
}

#[test]
fn requested_device_cannot_supply_missing_execution() {
    let config = SolveConfig {
        backend: Backend::Metal,
        stats: true,
        ..Default::default()
    };
    let text = render(&config, None);
    assert!(value(&text, "Requested device").starts_with("metal"));
    assert_eq!(value(&text, "Recorded execution"), "unavailable");
    assert!(value(&text, "Execution work").starts_with("unavailable"));
    assert!(!text.contains("GPU decoded work"));
}

#[test]
fn closure_work_names_its_completed_check_scope() {
    let (options, outcome) = solve(
        "p:-not q.q:-not p.",
        &["--backend", "cpu", "--grounder", "lazy"],
        true,
    );
    let semantic = outcome.semantic();
    let execution = semantic.closure_execution().unwrap();
    assert!(execution.work > 0);
    let text = render(&SolveConfig::from(&options), Some(semantic));
    let work = value(&text, "Closure work");
    assert_eq!(
        work.split_whitespace().next().unwrap(),
        execution.work.to_string()
    );
    assert!(work.contains("join/copy units; completed checks only"));
    assert!(text.contains("partial work omitted from closure totals"));
    assert!(value(&text, "Recorded execution").starts_with("CPU independent lazy closure"));
}

#[test]
fn disabled_measurements_keep_mandatory_work_receipts() {
    let (options, outcome) = solve("a|b.", &["--oracle", "countermodel"], false);
    let text = render(&SolveConfig::from(&options), Some(outcome.semantic()));
    assert_eq!(value(&text, "Requested detailed measurements"), "disabled");
    assert!(!value(&text, "Formula search work").starts_with("unavailable"));
    assert!(value(&text, "Eager rule work").starts_with("unavailable"));
}

#[test]
fn recorded_zero_does_not_become_unavailable() {
    let semantic = SemanticOutcome::interrupted_before_start(
        zetesis_cli::Interruption::Preparation(zetesis_cpu::Stop::Cancelled),
    );
    let text = render(&SolveConfig::default(), Some(&semantic));
    assert_eq!(
        value(&text, "Verified memberships")
            .split_whitespace()
            .next(),
        Some("0")
    );
    assert!(value(&text, "Recorded execution").starts_with("unavailable"));
}
