//! Actual solve outcomes remain unchanged by optional attempted host timing.

use crate::support::options::serial_with_statistics;
use zetesis_cli::{
    Completion, Options, PhaseTimings, Report, RunError, SolvePhase, StatisticsView,
    run_with_diagnostics,
};
use zetesis_cpu::Cancellation;

fn options(arguments: &[&str], statistics: bool) -> Options {
    let mut options = serial_with_statistics(arguments, statistics);
    options.statistics_view = StatisticsView::Records;
    options
}

fn before_timing(output: &str) -> &str {
    output.split_once("\nTime:").expect("human elapsed time").0
}

fn solve(source: &str, options: &Options) -> (Report, String, String) {
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
    (
        report,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn entered(timing: &PhaseTimings, phase: SolvePhase) {
    let measurement = timing
        .get(phase)
        .unwrap_or_else(|| panic!("{}", phase.label()));
    assert!(measurement.calls > 0);
    assert!(!measurement.overflowed);
    // Elapsed zero remains valid for a clock with coarse resolution. No timing
    // threshold, sleep, or sum-to-total property is part of the solver contract.
}

#[test]
fn optional_timing_preserves_complete_closure_and_formula_results() {
    for (source, arguments, formula, objective) in [
        (
            "p:-not q. q:-not p.",
            vec!["--grounder", "lazy"],
            false,
            false,
        ),
        (
            "p:-not q. q:-not p.",
            vec!["--grounder", "eager"],
            false,
            false,
        ),
        (
            "a|b. a:-b. b:-a.",
            vec!["--oracle", "countermodel"],
            true,
            false,
        ),
        (
            "1{p(1);p(2)}1. {hidden}. #minimize{X:p(X)}. #show. #show seen(X):p(X).",
            vec!["--oracle", "countermodel"],
            true,
            true,
        ),
    ] {
        let (mut plain, plain_output, plain_diagnostics) =
            solve(source, &options(&arguments, false));
        let (mut measured, output, diagnostics) = solve(source, &options(&arguments, true));
        assert_eq!(before_timing(&output), before_timing(&plain_output));
        assert_eq!(measured.completion, Completion::Exhausted);
        assert_eq!(measured.models, plain.models);
        assert_eq!(measured.checked, plain.checked);
        assert_eq!(measured.interruption, plain.interruption);
        assert_eq!(measured.discovered_gate_atoms, plain.discovered_gate_atoms);
        assert!(measured.formula_execution.is_none());
        assert!(plain.formula_execution.is_none());
        assert_eq!(
            format!("{:?}", measured.optimization),
            format!("{:?}", plain.optimization)
        );
        assert!(diagnostics.starts_with(&plain_diagnostics));
        assert!(!plain_diagnostics.contains("Phase timings:"));
        let coarse = plain.phase_timings.as_ref().unwrap();
        for phase in SolvePhase::ALL {
            assert!(coarse.get(phase).is_none());
        }
        let timing = measured.phase_timings.unwrap();
        for phase in [
            SolvePhase::AdmissionMaterialization,
            SolvePhase::ExecutionSetup,
            SolvePhase::CandidateSetup,
            SolvePhase::CandidateGeneration,
            SolvePhase::ObservationOutput,
        ] {
            entered(&timing, phase);
        }
        assert!(timing.get(SolvePhase::GpuHostOracle).is_none());
        for phase in [
            SolvePhase::OriginalValidation,
            SolvePhase::ExactReductMembership,
        ] {
            assert_eq!(timing.get(phase).is_some(), formula);
        }
        assert_eq!(
            timing.get(SolvePhase::ClosureMembership).is_some(),
            !formula
        );
        assert_eq!(
            timing.get(SolvePhase::ObjectiveScoringRetention).is_some(),
            objective
        );
        assert_eq!(
            timing.get(SolvePhase::ObjectiveFeedback).is_some(),
            objective
        );
        if let Some(statistics) = measured.countermodel_statistics.as_mut() {
            assert!(statistics.phase_timings.take().is_some());
        }
        assert_eq!(
            measured.countermodel_statistics,
            plain.countermodel_statistics.take()
        );
        assert_eq!(diagnostics.matches("Phase timings:").count(), 1);
        assert!(diagnostics.contains("  phase gpu_host_oracle: unmeasured\n"));
        assert!(diagnostics.ends_with("kernel_time=unmeasured\n"));
    }
}

#[test]
fn disabled_objective_feedback_is_unmeasured_while_scoring_remains_measured() {
    let (report, output, _) = crate::support::prepared::formula_run(
        "1{a;b}1. #minimize{1,a:a;2,b:b}.",
        &["--stats"],
        |config| config.solve.max_objective_bound_work = 0,
    );
    let report = report.unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    assert!(output.contains("Optimization: 1\n"));
    let timing = report.phase_timings.unwrap();
    entered(&timing, SolvePhase::ObjectiveScoringRetention);
    assert!(timing.get(SolvePhase::ObjectiveFeedback).is_none());
}

#[test]
fn admission_and_candidate_setup_failures_retain_attempts_not_false_work() {
    let mut diagnostics = Vec::new();
    assert!(
        run_with_diagnostics(
            "a(.".into(),
            &options(&[], true),
            &mut Vec::new(),
            &mut diagnostics,
            &Cancellation::default(),
        )
        .is_err()
    );
    let text = String::from_utf8(diagnostics).unwrap();
    assert!(text.contains("phase admission_materialization: calls="));
    assert!(text.contains("phase candidate_setup: unmeasured"));
    assert!(text.contains("phase exact_reduct_membership: unmeasured"));
    let (report, output, _) =
        crate::support::prepared::formula_run("a|b.", &["--stats"], |config| {
            config.solve.max_search_work = 0;
        });
    let report = report.unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(!output.contains("SATISFIABLE"));
    assert!(output.contains("Models: 0 (search incomplete)"));
    let timing = report.phase_timings.unwrap();
    entered(&timing, SolvePhase::CandidateSetup);
    assert!(timing.get(SolvePhase::CandidateGeneration).is_none());
}

#[test]
fn retention_stop_and_failed_answer_write_never_claim_complete_output() {
    let (report, output, _) =
        crate::support::prepared::formula_run("p. #minimize{1:p}.", &["--stats"], |config| {
            config.solve.max_optimal_bytes = 0;
        });
    let report = report.unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(!output.contains("OPTIMUM FOUND"));
    entered(
        &report.phase_timings.unwrap(),
        SolvePhase::ObjectiveScoringRetention,
    );
    let mut diagnostics = Vec::new();
    let error = run_with_diagnostics(
        "a|b.".into(),
        &options(&[], true),
        &mut zetesis_test_support::io::FailAt::new(b"Answer:"),
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::Output(_)));
    let text = String::from_utf8(diagnostics).unwrap();
    let output_calls: u64 = text
        .lines()
        .find_map(|line| {
            line.trim_start()
                .strip_prefix("phase observation_output: calls=")
        })
        .unwrap()
        .split_once(';')
        .unwrap()
        .0
        .parse()
        .unwrap();
    // Configuration delivery and the failed answer are separate output attempts.
    assert!(output_calls >= 2);
    assert!(text.contains("phase exact_reduct_membership: calls="));
    assert!(text.contains("failed_attempts=included"));
    // Failed Reports still do not recover their local semantic ledger. The
    // external recorder supplies attempted time only, never invented counts.
    assert!(text.contains("counters=unavailable"));
}

#[test]
fn already_loaded_original_bundle_receives_driver_phases_on_both_oracles() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/correctness/excerpts/shortest-path-reachable.lp");
    let source = std::fs::read_to_string(&path).unwrap();
    for oracle in ["closure", "countermodel"] {
        let options = options(&["--oracle", oracle], true);
        // Loading is deliberately outside the driver API and its timer.
        let bundle = zetesis_themelios::SourceBundle::load(
            &path,
            zetesis_themelios::BundleLimits::default(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let report = zetesis_cli::run_bundle_with_diagnostics(
            bundle,
            &options,
            &mut output,
            &mut diagnostics,
            &Cancellation::default(),
        )
        .unwrap();
        let (direct, direct_output, _) = solve(&source, &options);
        assert_eq!(
            before_timing(std::str::from_utf8(&output).unwrap()),
            before_timing(&direct_output)
        );
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.models, 1);
        assert_eq!(report.checked, direct.checked);
        entered(
            &report.phase_timings.unwrap(),
            SolvePhase::AdmissionMaterialization,
        );
        let text = String::from_utf8(diagnostics).unwrap();
        assert!(text.contains("phase admission_materialization: calls=1;"));
        assert!(text.contains("source_loading=excluded"));
    }
}
