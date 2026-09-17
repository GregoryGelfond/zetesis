//! Opt-in stderr statistics preserve answer records and truthful completion.

#[path = "support/bounded_writer.rs"]
mod bounded_writer;

use std::io;
use std::path::PathBuf;

use bounded_writer::BoundedWriter;
use clap::Parser;
use zetesis_cli::{
    Completion, Options, Report, RunError, run_bundle_with_diagnostics, run_with_diagnostics,
};
use zetesis_cpu::Control;
use zetesis_themelios::{BundleLimits, SourceBundle};

fn options(arguments: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
        ]
        .into_iter()
        .chain(arguments.iter().copied()),
    )
    .unwrap()
}

fn solve(source: &str, options: &Options) -> (Report, Vec<u8>, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    (report, output, String::from_utf8(diagnostics).unwrap())
}

#[test]
fn statistics_distinguish_formula_profile_limits() {
    for (atoms, roots, expected_atoms, expected_roots) in [
        (1_000_000, 1_000_000, 1_000_000, 1_000_000),
        (11, 13, 11, 13),
    ] {
        let mut configured = options(&["--stats", "--oracle", "countermodel"]);
        configured.max_atoms = atoms;
        configured.max_ground_rules = roots;
        let (_, _, diagnostics) = solve("a.", &configured);
        assert!(
            diagnostics.contains(&format!("requested grounding limits: atoms={atoms};")),
            "{diagnostics}"
        );
        assert!(diagnostics.contains(&format!(
            "formula profile ceilings: atoms={expected_atoms}; roots={expected_roots}; nodes=1048576; source values=1000000; assignment values/operation=1000000; generated binding values=1000000; support rounds=1000000"
        )), "{diagnostics}");
        assert!(
            diagnostics.contains(&format!(
                "; work={} (applicable when formula admission is selected)",
                zetesis_themelios::FormulaLimits::default().max_work
            )),
            "{diagnostics}"
        );
        assert!(
            diagnostics.contains(&format!(
                "expansion limits: work={};",
                zetesis_themelios::ExpansionLimits::default().max_term_work
            )),
            "{diagnostics}"
        );
        assert!(
            diagnostics.contains(&format!(
                "; scalar bytes={};",
                zetesis_themelios::ExpansionLimits::default().max_scalar_bytes
            )),
            "{diagnostics}"
        );
    }
}

#[test]
fn statistics_explain_candidate_restriction_units() {
    let (_, _, text) = solve(
        "{a}. {b}. :- a,b.",
        &options(&["--stats", "--grounder", "lazy"]),
    );
    assert!(
        text.contains("conjunctions=1; skipped impossible intervals=1;"),
        "{text}"
    );
    assert!(text.contains("peak copied payload bytes="), "{text}");
    assert!(text.contains("allocator/index overhead excluded"), "{text}");
}

#[test]
fn statistics_identify_necessary_disjunctive_support() {
    let (_, _, text) = solve("a | b.", &options(&["--stats", "--oracle", "countermodel"]));
    assert!(
        text.contains("necessary disjunctive support: status=Applied;"),
        "{text}"
    );
    assert!(text.contains("(included in search work)"), "{text}");
}

#[test]
fn requested_statistics_use_accepted_policy_spelling() {
    for backend in ["auto", "cpu"] {
        for oracle in ["auto", "closure", "countermodel"] {
            let configured = Options::try_parse_from([
                "zetesis",
                "--stats",
                "--workers",
                "1",
                "--models",
                "0",
                "--backend",
                backend,
                "--oracle",
                oracle,
            ])
            .unwrap();
            let (_, _, diagnostics) = solve("a.", &configured);
            assert!(
                diagnostics.contains(&format!(
                    "requested: backend={backend}; oracle={oracle}; grounder=auto"
                )),
                "{diagnostics}"
            );
        }
    }
}

#[test]
fn solver_statistics_are_never_silently_discarded_by_the_device_command() {
    let file = Options::try_parse_from(["zetesis", "--stats", "devices"]).unwrap();
    assert!(file.stats);
    assert_eq!(file.command, None);
    assert_eq!(file.input, PathBuf::from("devices"));
    let inventory = Options::try_parse_from(["zetesis", "devices"]).unwrap();
    assert_eq!(inventory.command, Some(zetesis_cli::Command::Devices));
    assert!(Options::try_parse_from(["zetesis", "devices", "--stats"]).is_err());
}

#[test]
fn statistics_flag_is_opt_in_and_preserves_each_supported_cpu_answer_path() {
    assert!(!options(&[]).stats);
    assert!(options(&["--stats"]).stats);
    assert!(Options::try_parse_from(["zetesis", "--stats=false"]).is_err());
    for (source, arguments, effective) in [
        (
            "p.",
            vec!["--grounder", "lazy"],
            "oracle=closure; grounder=lazy",
        ),
        (
            "{p}.",
            vec!["--grounder", "eager"],
            "oracle=closure; grounder=eager",
        ),
        ("a | b.", vec![], "oracle=countermodel; grounder=eager"),
        (
            "{a;b}. #maximize{2,a:a;1,b:b}.",
            vec![],
            "oracle=tight-support; grounder=eager",
        ),
    ] {
        let mut configured = options(&arguments);
        let (baseline, expected, ordinary) = solve(source, &configured);
        assert!(!ordinary.contains("Statistics:"));
        configured.stats = true;
        let (report, output, diagnostics) = solve(source, &configured);
        assert_eq!(output, expected);
        assert_eq!(report.completion, baseline.completion);
        assert_eq!(report.models, baseline.models);
        assert!(diagnostics.starts_with(&ordinary));
        assert!(diagnostics.contains(effective), "{diagnostics}");
        assert!(diagnostics.contains("configured: workers=1; batch=64; displayed models=0"));
        assert!(diagnostics.contains("completion: exhausted"));
        assert!(diagnostics.contains(&format!("GPU compiled={}", cfg!(feature = "gpu"))));
        let timing = diagnostics
            .lines()
            .find(|line| line.starts_with("  driver wall time:"))
            .unwrap();
        let milliseconds: f64 = timing.split_whitespace().nth(3).unwrap().parse().unwrap();
        assert!(milliseconds.is_finite() && milliseconds >= 0.0);
        if source.contains("#maximize") {
            assert!(diagnostics.contains("objective: optimal; costs(priority,value)=[(0, -3)]"));
            assert!(diagnostics.contains("candidate restrictions="));
        } else if report.countermodel_statistics.is_none() {
            assert!(
                diagnostics.contains("independent closure: checks completed="),
                "{diagnostics}"
            );
        }
    }
}

#[test]
fn incomplete_and_requested_model_statistics_do_not_claim_exhaustion_or_optimality() {
    let mut one_model = options(&["--stats"]);
    one_model.models = 1;
    let (report, _, text) = solve("{a}.", &one_model);
    assert_eq!(report.completion, Completion::RequestedModels);
    assert!(text.contains("requested models reached (partial coverage)"));
    let mut bounded = options(&["--stats", "--oracle", "countermodel"]);
    bounded.max_candidates = 0;
    let (report, output, text) = solve("{a}. #minimize{1:a}.", &bounded);
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(report.optimization.is_none());
    assert!(text.contains("interrupted (partial coverage)"));
    assert!(text.contains("objective: no retained score; evaluation counters=unavailable"));
    assert!(!text.contains("objective: optimal"));
    assert!(!String::from_utf8(output).unwrap().contains("OPTIMUM FOUND"));
    bounded.max_candidates = 1;
    bounded.max_objective_bound_work = 0;
    let (report, output, text) = solve("1 {a;b} 1. #minimize{1,a:a;2,b:b}.", &bounded);
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(report.optimization.is_some());
    assert!(text.contains("objective: incumbent only"));
    assert!(!text.contains("objective: optimal"));
    assert!(!String::from_utf8(output).unwrap().contains("OPTIMUM FOUND"));
}

#[test]
fn cancellation_and_source_refusal_have_unavailable_execution_not_zero_work() {
    let options = options(&["--stats", "--oracle", "countermodel"]);
    let control = Control::default();
    control.cancel();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "p(.".into(),
        &options,
        &mut io::sink(),
        &mut diagnostics,
        &control,
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(report.countermodel_statistics.is_none());
    let text = String::from_utf8(diagnostics).unwrap();
    assert!(text.contains("effective execution: unavailable (stopped before execution counters)"));
    assert!(text.contains("oracle work: unavailable"));
    let mut diagnostics = Vec::new();
    let mut output = Vec::new();
    let error = run_with_diagnostics(
        "p(.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::FormulaAdmission(_)));
    let text = String::from_utf8(diagnostics).unwrap();
    assert!(text.contains("status: failed; completion=unavailable"));
    assert!(text.contains("effective execution: unavailable; counters=unavailable"));
    assert!(!text.contains("completion: exhausted"));
    assert!(output.is_empty());
}

#[test]
fn automatic_execution_reports_cpu() {
    let options = Options::try_parse_from(["zetesis", "--stats", "--workers", "1"]).unwrap();
    let (_, _, text) = solve("p.", &options);
    assert!(text.contains("backend=cpu; oracle=closure; grounder=lazy"));
    assert!(!text.contains("backend=untracked"));
}

#[test]
fn bundled_sources_use_the_same_statistics_boundary_without_rewriting_stdout() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kr-domains/accepted/shortest-path-reachable.lp");
    let mut configured = options(&[]);
    let mut baseline = Vec::new();
    let first = run_bundle_with_diagnostics(
        SourceBundle::load(&path, BundleLimits::default()).unwrap(),
        &configured,
        &mut baseline,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap();
    configured.stats = true;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_bundle_with_diagnostics(
        SourceBundle::load(&path, BundleLimits::default()).unwrap(),
        &configured,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    assert_eq!(output, baseline);
    assert_eq!(report.models, first.models);
    assert_eq!(report.completion, first.completion);
    let text = String::from_utf8(diagnostics).unwrap();
    assert_eq!(text.matches("Statistics: zetesis").count(), 1);
    assert!(text.contains("excludes source loading and statistics"));
}

#[test]
fn statistics_writer_failure_is_a_typed_error_with_the_exact_written_prefix() {
    let options = options(&["--stats"]);
    let (_, expected_output, text) = solve("p.", &options);
    let start = text.find("Statistics: zetesis").unwrap();
    for capacity in [start, start + "Statistics: zetesis".len()] {
        let mut diagnostics = BoundedWriter::new(capacity);
        let mut output = Vec::new();
        let error = run_with_diagnostics(
            "p.".into(),
            &options,
            &mut output,
            &mut diagnostics,
            &Control::default(),
        )
        .unwrap_err();
        let RunError::Output(error) = error else {
            panic!("expected statistics write failure: {error}");
        };
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(diagnostics.bytes(), &text.as_bytes()[..capacity]);
        assert_eq!(
            output, expected_output,
            "completed answers remain byte-identical even if later statistics cannot be written"
        );
    }
}

#[test]
fn statistics_print_expansion_usage_beside_its_ceilings() {
    let (report, _, text) = solve("p(1..3). q(X) :- p(X).", &options(&["--stats"]));
    let usage = report.expansion.unwrap();
    assert!(usage.term_work > 0);
    assert!(text.contains(&format!(
        "expansion used: term work={} of 1048576; templates={} of 100000; values={} of 1000000;",
        usage.term_work, usage.templates, usage.values
    )), "{text}");
    // The formula route admits through its own budgets and reports no usage.
    let (formula, _, text) = solve("a | b.", &options(&["--stats"]));
    assert!(formula.expansion.is_none());
    assert!(!text.contains("expansion used:"), "{text}");
}

#[test]
fn an_inconsistent_closure_reservation_is_refused_before_any_work() {
    let mut configured = options(&["--stats"]);
    configured.workers = std::num::NonZeroUsize::new(5).unwrap();
    configured.max_closure_bytes = Some(134_217_728);
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let error = run_with_diagnostics(
        "{a}.".into(),
        &configured,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap_err();
    let text = error.to_string();
    assert!(text.contains("--workers 5"), "{text}");
    assert!(text.contains("--max-closure-bytes 134217728"), "{text}");
    assert!(
        text.contains("--max-closure-batch-bytes 536870912"),
        "{text}"
    );
    assert!(text.contains("671088640"), "{text}");
    assert!(output.is_empty());
}

#[test]
fn the_closure_limits_line_states_the_derived_allowance() {
    let mut eight = options(&["--stats"]);
    eight.workers = std::num::NonZeroUsize::new(8).unwrap();
    let (_, _, text) = solve("{a}.", &eight);
    assert!(
        text.contains("independent CPU closure limits: named bytes/owner=67108864 (collective share of 8 workers)"),
        "{text}"
    );
    let (_, _, explicit) = solve(
        "{a}.",
        &options(&["--stats", "--max-closure-bytes", "4096"]),
    );
    assert!(explicit.contains("named bytes/owner=4096;"), "{explicit}");
}
