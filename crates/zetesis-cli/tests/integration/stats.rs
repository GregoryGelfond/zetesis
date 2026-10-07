//! Opt-in stderr statistics preserve answer records and truthful completion.

use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;

use crate::support::human::before_timing;
use crate::support::options::serial;
use clap::Parser;
use zetesis_cli::{
    Completion, Options, Report, RunError, run_bundle_with_diagnostics, run_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::BoundedWriter;
use zetesis_themelios::{BundleLimits, SourceBundle};

fn options(arguments: &[&str]) -> Options {
    let mut options = serial(arguments);
    options.statistics_view = zetesis_cli::StatisticsView::Records;
    options
}

fn solve(source: &str, options: &Options) -> (Report, Vec<u8>, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    (report, output, String::from_utf8(diagnostics).unwrap())
}

#[test]
fn statistics_report_memory_derived_storage() {
    for memory in [64 * 1024 * 1024, 128 * 1024 * 1024] {
        let mut configured = options(&["--stats", "--oracle", "countermodel"]);
        configured.memory = memory;
        let resources = zetesis_solve::Resources::new(memory, configured.workers);
        let formula = resources.formula_limits();
        let (_, _, diagnostics) = solve("a.", &configured);
        assert!(
            diagnostics.contains("no cumulative operation budget"),
            "{diagnostics}"
        );
        assert!(
            diagnostics.contains(&format!(
                "support bytes={}; formula nodes={}; formula operands={}",
                formula.max_support_bytes, formula.theory.max_nodes, formula.theory.max_operands,
            )),
            "{diagnostics}"
        );
        assert!(!diagnostics.contains("formula profile ceilings:"));
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
fn statistics_identify_necessary_disjunctive_support_under_clauses() {
    let (_, _, text) = solve(
        "a | b.",
        &options(&["--stats", "--oracle", "countermodel", "--search", "clauses"]),
    );
    assert!(
        text.contains("necessary disjunctive support: status=Applied;"),
        "{text}"
    );
    assert!(text.contains("(included in search work)"), "{text}");
    assert!(!text.contains("candidate regions:"), "{text}");
}

fn regions_statistics() -> String {
    let (_, _, text) = solve(
        "a | b.",
        &options(&["--stats", "--oracle", "countermodel", "--search", "regions"]),
    );
    text
}

#[test]
fn statistics_report_the_regions_the_search_visited() {
    let text = regions_statistics();
    assert!(
        text.contains("oracle=countermodel; grounder=auto; search=regions"),
        "{text}"
    );
    assert!(text.contains("candidate regions: visited="), "{text}");
    assert!(text.contains("; leaves=2; propagations="), "{text}");
}

#[test]
fn statistics_report_the_support_cut_in_place_of_the_disjunctive_certificate() {
    let text = regions_statistics();
    assert!(
        text.contains("support cut=applied; reading work="),
        "{text}"
    );
    assert!(!text.contains("necessary disjunctive support:"), "{text}");
}

#[test]
fn requested_statistics_use_accepted_policy_spelling() {
    for backend in ["cpu"] {
        for oracle in ["auto", "closure", "countermodel"] {
            let mut configured = Options::try_parse_from([
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
            configured.statistics_view = zetesis_cli::StatisticsView::Records;
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
        assert_eq!(
            before_timing(std::str::from_utf8(&output).unwrap()),
            before_timing(std::str::from_utf8(&expected).unwrap())
        );
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
fn cancellation_and_source_refusal_have_unavailable_execution_not_zero_work() {
    let options = options(&["--stats", "--oracle", "countermodel"]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "p(.".into(),
        &options,
        &mut io::sink(),
        &mut diagnostics,
        &cancellation,
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
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::FormulaAdmission(_)));
    let text = String::from_utf8(diagnostics).unwrap();
    assert!(text.contains("status: failed; completion=unavailable"));
    assert!(text.contains("effective execution: unavailable; counters=unavailable"));
    assert!(!text.contains("completion: exhausted"));
    assert!(!std::str::from_utf8(&output).unwrap().contains("Answer:"));
}

#[test]
fn automatic_execution_reports_cpu() {
    let mut options = Options::try_parse_from(["zetesis", "--stats", "--workers", "1"]).unwrap();
    options.statistics_view = zetesis_cli::StatisticsView::Records;
    let (_, _, text) = solve("p.", &options);
    assert!(text.contains("backend=cpu; oracle=closure; grounder=lazy"));
    assert!(!text.contains("backend=untracked"));
}

#[test]
fn bundled_sources_use_the_same_statistics_boundary_without_rewriting_stdout() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/correctness/excerpts/shortest-path-reachable.lp");
    let mut configured = options(&[]);
    let mut baseline = Vec::new();
    let first = run_bundle_with_diagnostics(
        SourceBundle::load(&path, BundleLimits::default()).unwrap(),
        &configured,
        &mut baseline,
        &mut io::sink(),
        &Cancellation::default(),
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
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        before_timing(std::str::from_utf8(&output).unwrap()),
        before_timing(std::str::from_utf8(&baseline).unwrap())
    );
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
            &Cancellation::default(),
        )
        .unwrap_err();
        let RunError::Output(error) = error else {
            panic!("expected statistics write failure: {error}");
        };
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(diagnostics.bytes(), &text.as_bytes()[..capacity]);
        assert_eq!(
            before_timing(std::str::from_utf8(&output).unwrap()),
            before_timing(std::str::from_utf8(&expected_output).unwrap()),
            "completed answers and status remain byte-identical if later statistics fail"
        );
    }
}

#[test]
fn statistics_print_expansion_usage_beside_its_ceilings() {
    let configured = options(&["--stats"]);
    let limits =
        zetesis_solve::Resources::new(configured.memory, configured.workers).expansion_limits();
    let (report, _, text) = solve("p(1..3). q(X) :- p(X).", &configured);
    let usage = report.expansion.unwrap();
    assert!(usage.term_work > 0);
    assert!(
        text.contains(&format!(
            "expansion used: term work={} of {}; templates={} of {}; values={} of {};",
            usage.term_work,
            limits.max_term_work,
            usage.templates,
            limits.max_templates,
            usage.values,
            limits.max_values
        )),
        "{text}"
    );
    // The formula route admits through its own budgets and reports no usage.
    let (formula, _, text) = solve("a | b.", &options(&["--stats"]));
    assert!(formula.expansion.is_none());
    assert!(!text.contains("expansion used:"), "{text}");
}

#[test]
fn the_closure_limits_line_states_the_derived_allowance() {
    let mut eight = options(&["--stats", "--memory", "2147483648"]);
    eight.workers = std::num::NonZeroUsize::new(8).unwrap();
    let (_, _, text) = solve("{a}.", &eight);
    assert!(
        text.contains("closure bytes/worker=67108864; closure bytes/collective=536870912;"),
        "{text}"
    );
    eight.memory /= 2;
    let (_, _, text) = solve("{a}.", &eight);
    assert!(
        text.contains("closure bytes/worker=33554432; closure bytes/collective=268435456;"),
        "{text}"
    );
}

#[test]
fn the_counter_runs_between_the_program_closures() {
    // Eight nodes, one bad, so the gate predicates blocked/1 and reach/1 have
    // sixteen symbolic atoms. The first pass finds blocked(4) and reach(1)
    // necessary and seven blocked atoms underivable; the second, reading
    // those decisions, finds every other reachable node necessary and
    // reach(4) underivable; the third changes nothing. One seed remains of
    // the 65,536 the symbolic carrier offered.
    let mut source = String::from("node(1..8). bad(3). ");
    for node in 1..8 {
        write!(
            source,
            "e({node},{}). next({node},{}). ",
            node + 1,
            node + 1
        )
        .unwrap();
    }
    for node in 1..7 {
        write!(source, "e({node},{}). ", node + 2).unwrap();
    }
    source.push_str(
        "blocked(Y) :- bad(X), next(X,Y). reach(1). reach(Y) :- reach(X), e(X,Y), not blocked(Y). :- not reach(8).",
    );
    let (report, _, diagnostics) = solve(&source, &options(&["--stats"]));
    assert_eq!(report.models, 1);
    assert_eq!(report.checked, 1, "{diagnostics}");
    assert!(
        diagnostics.contains("carrier narrowing: passes=3; cut gate atoms=8; held gate atoms=8"),
        "{diagnostics}"
    );
    // The root is decided, so it is the one region and its one seed.
    assert!(
        diagnostics.contains(
            "carrier regions: visited=1; refuted=0; leaves=1; counted=0; narrowing passes=0"
        ),
        "{diagnostics}"
    );
}

#[test]
fn the_regions_of_independent_pairs_are_the_answers() {
    // Deciding out(i) decides in(i), and two adjacent in atoms are refuted
    // by the edge constraint in the region's lower closure: the regions'
    // leaves are the thirteen independent sets of the path, each checked once.
    let (report, _, diagnostics) = solve(
        "node(1..5). edge(1,2). edge(2,3). edge(3,4). edge(4,5). \
         in(X) :- node(X), not out(X). out(X) :- node(X), not in(X). :- edge(X,Y), in(X), in(Y).",
        &options(&["--stats"]),
    );
    assert_eq!(report.models, 13);
    assert_eq!(report.checked, 13, "{diagnostics}");
    assert!(
        diagnostics.contains("carrier regions: visited=")
            && diagnostics.contains("; leaves=13; counted=0;"),
        "{diagnostics}"
    );
}

#[test]
fn a_root_refuted_program_states_the_constraint_once() {
    // The root's narrowing refutes every seed: the carrier receipt says a
    // definite constraint fired, the execution line says no route ran, and
    // the unavailable oracle work is said once.
    let (report, _, text) = solve("{a}. p. :- p.", &options(&["--stats"]));
    assert_eq!(report.models, 0);
    assert_eq!(text.matches("definite constraint").count(), 1, "{text}");
    assert_eq!(
        text.matches("oracle work: unavailable").count(),
        1,
        "{text}"
    );
    assert!(
        text.contains("effective execution: none needed; the root narrowing refuted every seed\n"),
        "{text}"
    );
}
