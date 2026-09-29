//! Human statistics derive from receipts, independently of requested hardware.

use std::{io, num::NonZeroUsize};

use clap::Parser;
use zetesis_cli::{
    Backend, ColorMode, Options, PublicationOutcome, SemanticOutcome, SolveConfig,
    SolveMeasurements, run_finalized_with_diagnostics, statistics_view::Statistics,
};
use zetesis_cpu::Cancellation;
use zetesis_presentation::Layout;

use zetesis_test_support::io::BoundedWriter;

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
        &Cancellation::default(),
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
    assert!(value(&text, "Requested backend").starts_with("cpu"));
}

#[test]
fn requested_device_cannot_supply_missing_execution() {
    let config = SolveConfig {
        backend: Backend::Gpu(Some(zetesis_backend::GpuApi::Metal)),
        stats: true,
        ..Default::default()
    };
    let text = render(&config, None);
    assert!(value(&text, "Requested backend").starts_with("metal"));
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

#[test]
fn shared_work_keeps_distinct_operation_counts() {
    let (options, outcome) = solve("{a}. {b}.", &["--source-batching", "worlds"], true);
    let semantic = outcome.semantic();
    let execution = semantic.shared_execution().unwrap();
    assert!(execution.source_work > 0);
    assert!(execution.world_work > 0);
    let text = render(&SolveConfig::from(&options), Some(semantic));
    assert_eq!(
        value(&text, "Recorded execution"),
        "CPU shared lazy closure"
    );
    for (label, expected) in [
        ("Shared source work", execution.source_work),
        ("Shared world work", execution.world_work),
    ] {
        assert_eq!(
            value(&text, label).split_whitespace().next().unwrap(),
            expected.to_string()
        );
    }
    assert!(!text.contains("Closure tuple probes"));
    assert!(!text.contains("GPU decoded work"));
}

#[test]
fn eager_closure_work_does_not_claim_tuple_probes() {
    let (options, outcome) = solve("{a}. {b}.", &["--grounder", "eager"], true);
    let semantic = outcome.semantic();
    let execution = semantic.closure_execution().unwrap();
    assert_eq!(execution.route, zetesis_cli::ClosureRoute::Eager);
    assert!(execution.work > 0);
    let text = render(&SolveConfig::from(&options), Some(semantic));
    assert_eq!(
        value(&text, "Recorded execution"),
        "CPU independent eager closure"
    );
    assert!(value(&text, "Closure work").contains("eager scan units; completed checks only"));
    assert!(!text.contains("Closure tuple probes"));
}

#[test]
fn positive_execution_is_named_from_its_certificate() {
    let (options, outcome) = solve(
        "a. b:-a. a:-b. #minimize{2@3,k:b;1@1,k:a}.",
        &["--grounder", "eager", "--search", "clauses"],
        true,
    );
    let semantic = outcome.semantic();
    let certified = semantic
        .countermodel_statistics()
        .unwrap()
        .certified
        .unwrap();
    assert!(matches!(
        certified.plan,
        Some(zetesis_sat::CertificatePlanStatistics::Positive(_))
    ));
    assert!(certified.checks > 0);
    let text = render(&SolveConfig::from(&options), Some(semantic));
    assert_eq!(
        value(&text, "Recorded execution"),
        "CPU native formula (positive consequences)"
    );
}

#[test]
fn objective_work_preserves_the_recorded_count() {
    let (options, outcome) = solve("a. #minimize{2@3,k:a}.", &[], true);
    let semantic = outcome.semantic();
    let text = render(&SolveConfig::from(&options), Some(semantic));
    let objective = semantic.incumbent().unwrap();
    assert!(objective.work > 0);
    assert_eq!(
        value(&text, "Objective evaluation work")
            .split_whitespace()
            .next()
            .unwrap(),
        objective.work.to_string()
    );
}

#[test]
fn batched_cpu_completion_does_not_claim_device_work() {
    let (options, outcome) = solve(
        "a|b.",
        &[
            "--oracle",
            "countermodel",
            "--search",
            "clauses",
            "--completion-workers",
            "2",
        ],
        true,
    );
    let semantic = outcome.semantic();
    let execution = semantic.formula_execution().unwrap();
    assert!(execution.adapter.is_empty());
    assert!(execution.completion.entered > 0);
    let text = render(&SolveConfig::from(&options), Some(semantic));
    assert_eq!(
        value(&text, "Recorded execution"),
        "CPU batched formula completion"
    );
    assert_eq!(
        value(&text, "Host completion attempts")
            .split_whitespace()
            .next()
            .unwrap(),
        execution.completion.entered.to_string()
    );
    assert!(!text.contains("GPU submitted candidates"));
    assert!(!text.contains("GPU decoded work"));
}

#[test]
fn grounding_rows_preserve_the_measured_rule_scope() {
    let (options, outcome) = solve(
        "d(1..3). {p(X)}:-d(X). #minimize{X:p(X)}.",
        &["--grounder", "eager", "--formula-joins", "table"],
        true,
    );
    let config = SolveConfig::from(&options);
    let timings = outcome.report().unwrap().phase_timings.as_ref().unwrap();
    let rules = timings
        .grounding
        .get(zetesis_cli::GroundingPhase::RuleInstantiation)
        .unwrap();
    let view = Statistics {
        requested: &config,
        timings,
        semantic: Some(outcome.semantic()),
        publication: Some(outcome.publication()),
        failed: false,
    };
    let mut output = Vec::new();
    view.write_human(
        &mut output,
        Layout::new(NonZeroUsize::new(256).unwrap(), ColorMode::Never),
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    for (label, recorded) in [
        ("Rule join probes", rules.work.join_probes),
        ("Rule table preparations", rules.work.table_preparations),
        ("Rule table reuses", rules.work.table_reuses),
        ("Rule table probes", rules.work.table_probes),
    ] {
        let row = value(&text, label);
        assert_eq!(
            row.split_whitespace().next().unwrap(),
            recorded.map_or_else(|| "unavailable".into(), |count| count.to_string())
        );
        assert!(row.contains("eager formula rule instantiation only"));
    }
}

#[test]
fn statistics_writer_failure_preserves_its_prefix() {
    let (options, outcome) = solve("a.", &[], true);
    let config = SolveConfig::from(&options);
    let view = Statistics {
        requested: &config,
        timings: outcome.report().unwrap().phase_timings.as_ref().unwrap(),
        semantic: Some(outcome.semantic()),
        publication: Some(outcome.publication()),
        failed: false,
    };
    let layout = Layout::new(NonZeroUsize::new(80).unwrap(), ColorMode::Always);
    let mut expected = Vec::new();
    view.write_human(&mut expected, layout).unwrap();
    for capacity in [0, expected.len() / 2, expected.len() - 1] {
        let mut output = BoundedWriter::new(capacity);
        let error = view.write_human(&mut output, layout).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(output.bytes(), &expected[..capacity]);
    }
    assert_eq!(
        outcome.semantic().completion(),
        Some(zetesis_cli::Completion::Exhausted)
    );
}
