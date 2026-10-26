//! Every interrupted statistics write must preserve exactly its accepted prefix.

use std::io;
use std::time::Duration;

use clap::Parser;

use crate::{
    Completion, Options, PublicationFailure, Report, RunError, run_finalized_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::{BoundedWriter, FULL, FailAt};

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

fn actual(
    source: &str,
    options: &Options,
    cancellation: &Cancellation,
) -> Result<Report, PublicationFailure> {
    assert!(
        !options.stats,
        "obtain the outcome independently of its statistics rendering"
    );
    run_finalized_with_diagnostics(
        source.into(),
        options,
        &mut io::sink(),
        &mut io::sink(),
        cancellation,
    )
    .and_then(crate::PublicationOutcome::into_legacy)
    .map(crate::PublicationReport::into_report)
}

fn every_prefix(options: &Options, outcome: &Result<Report, PublicationFailure>) -> String {
    let elapsed = Duration::from_micros(1_234);
    let mut reference = Vec::new();
    super::write_detailed(&mut reference, options, outcome.as_ref(), elapsed).unwrap();
    let text = std::str::from_utf8(&reference).unwrap();
    assert!(text.starts_with("Statistics: zetesis "));
    assert!(text.contains("driver wall time: 1.234 ms"));
    assert!(text.ends_with('\n'));
    for capacity in 0..reference.len() {
        let mut sink = BoundedWriter::new(capacity);
        let error =
            super::write_detailed(&mut sink, options, outcome.as_ref(), elapsed).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe, "cut {capacity}");
        assert_eq!(error.to_string(), FULL, "cut {capacity}");
        assert_eq!(sink.bytes(), &reference[..capacity], "cut {capacity}");
    }
    for capacity in [reference.len(), reference.len() + 1] {
        let mut sink = BoundedWriter::new(capacity);
        super::write_detailed(&mut sink, options, outcome.as_ref(), elapsed).unwrap();
        assert_eq!(sink.bytes(), reference, "no extra bytes after a full write");
    }
    text.to_owned()
}

fn bounded_statistics(
    source: &str,
    case: &crate::publication_fixture::FormulaCase,
) -> (Report, Vec<u8>, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = case
        .run(
            source,
            &mut output,
            &mut diagnostics,
            &Cancellation::default(),
        )
        .unwrap()
        .into_legacy()
        .unwrap()
        .into_report();
    (report, output, String::from_utf8(diagnostics).unwrap())
}

#[test]
fn every_completed_cpu_statistics_prefix_is_fallible_without_losing_bytes() {
    for (source, arguments, optimum) in [
        ("p.", vec!["--grounder", "lazy"], false),
        ("p.", vec!["--grounder", "eager"], false),
        ("a | b.", vec![], false),
        ("1 {a;b} 1. #minimize{1,a:a;2,b:b}.", vec![], true),
    ] {
        let options = options(&arguments);
        let outcome = actual(source, &options, &Cancellation::default());
        let report = outcome.as_ref().unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert!(report.countermodel_statistics.is_some());
        assert_eq!(report.optimization.is_some(), optimum);
        let text = every_prefix(&options, &outcome);
        assert!(text.contains("completion: exhausted"));
    }
    let options = options(&["--grounder", "eager"]);
    let outcome = actual("p.", &options, &Cancellation::default());
    let report = outcome.as_ref().unwrap();
    assert_eq!((report.checked, report.models), (1, 1));
    assert_eq!(report.completion, Completion::Exhausted);
    let text = every_prefix(&options, &outcome);
    assert!(text.contains("grounder=eager"));
    assert!(text.contains("backend=cpu; oracle=positive-consequences; grounder=eager"));
}

#[test]
fn statistics_failure_preserves_observation_cause() {
    let source = format!(
        "p(\"{}\"). #show shown:p(X). #minimize{{0@1,k:p(X)}}.",
        "x".repeat(1500)
    );
    let mut presentation = options(&["--json", "--stats"]);
    presentation.statistics_view = crate::StatisticsView::Records;
    let mut case = crate::publication_fixture::FormulaCase::new(presentation);
    case.config.observations.max_work = 0;
    let mut reference = Vec::new();
    case.run(
        &source,
        &mut io::sink(),
        &mut reference,
        &Cancellation::default(),
    )
    .unwrap_err();
    let capacity = std::str::from_utf8(&reference)
        .unwrap()
        .find("Statistics:")
        .unwrap();
    let mut output = Vec::new();
    let failure = case
        .run(
            &source,
            &mut output,
            &mut BoundedWriter::new(capacity),
            &Cancellation::default(),
        )
        .unwrap_err()
        .into_legacy();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let partial = failure.partial_report.unwrap();
    assert!(partial.summary_published);
    assert_eq!(partial.completion, Some(Completion::Exhausted));
    assert_eq!(value["outcome"]["status"], "failed");
    assert_eq!(value["outcome"]["coverage"], "exhausted");
    assert_eq!(value["outcome"]["optimization"]["optimal"], true);
    assert!(value["statistics"]["stage_timings"].is_object());
    assert_eq!(value["outcome"]["error"]["secondary_output_failure"], true);
    assert!(matches!(*failure.cause, RunError::Observation(_)));
    assert!(failure.secondary_output.is_some());
    assert_eq!(value["outcome"]["error"]["kind"], "observation");
    assert_eq!((partial.published_models, partial.verified_models), (0, 1));
}

#[test]
fn every_partial_or_cancelled_statistics_prefix_preserves_incomplete_qualification() {
    let mut requested = options(&[]);
    requested.models = 1;
    let outcome = actual("{a}.", &requested, &Cancellation::default());
    assert_eq!(
        outcome.as_ref().unwrap().completion,
        Completion::RequestedModels
    );
    assert!(
        every_prefix(&requested, &outcome).contains("requested models reached (partial coverage)")
    );
    for candidates in [0, 1] {
        let bounded = options(&["--oracle", "countermodel"]);
        let mut config = crate::PublicationConfig::from(&bounded);
        config.solve.max_candidates = candidates;
        config.solve.max_objective_bound_work = 0;
        let outcome = crate::publication_fixture::human(
            "1 {a;b} 1. #minimize{1,a:a;2,b:b}.",
            &config,
            &mut io::sink(),
            &Cancellation::default(),
        )
        .and_then(crate::PublicationOutcome::into_legacy)
        .map(crate::PublicationReport::into_report);
        let report = outcome.as_ref().unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!(report.optimization.is_some(), candidates == 1);
        let text = every_prefix(&bounded, &outcome);
        assert!(text.contains("interrupted (partial coverage)"));
        assert!(!text.contains("objective: optimal"));
    }
    let cancelled = options(&["--oracle", "countermodel"]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let outcome = actual("p(.", &cancelled, &cancellation);
    let report = outcome.as_ref().unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(report.countermodel_statistics.is_none());
    let text = every_prefix(&cancelled, &outcome);
    assert!(text.contains("effective execution: unavailable (stopped before execution counters)"));
}

#[test]
fn every_real_source_refusal_statistics_prefix_propagates_its_writer_failure() {
    let options = options(&["--oracle", "countermodel"]);
    let outcome = actual("p(.", &options, &Cancellation::default());
    assert!(
        matches!(outcome, Err(ref failure) if matches!(*failure.cause, RunError::FormulaAdmission(_)))
    );
    let text = every_prefix(&options, &outcome);
    assert!(text.contains("status: failed; completion=unavailable"));
    assert!(text.contains("failure: source admission:"));
    assert!(!text.contains("completion: exhausted"));
}

#[test]
fn hybrid_statistics_preserve_reported_fields() {
    // Only rendering is exercised here. The adapter spelling explicitly marks
    // these authored values as synthetic; no GPU execution/parity is claimed.
    for (pending, queued, completed) in [(0, 0, true), (3, 2, false)] {
        let options = options(&["--oracle", "countermodel"]);
        let mut report = actual("a | b.", &options, &Cancellation::default()).unwrap();
        let mut execution = crate::FormulaExecutionStatistics::default();
        execution.gpu_limits = Some(crate::FormulaDeviceLimits {
            work_per_candidate: 789,
            rounds_per_candidate: 17,
        });
        execution.gpu_submitted_batches = 3;
        execution.gpu_submitted_candidates = 10;
        "FORMAT FIXTURE: no physical execution".clone_into(&mut execution.adapter);
        execution.gpu_batches = 2;
        execution.gpu_candidates = 7;
        execution.gpu_work = 123;
        execution.gpu_rounds = 4;
        execution.cpu_residuals = 2;
        execution.gpu_decided = 2;
        execution.pending_candidates = pending;
        execution.queued_models = queued;
        execution.peak_accounted_bytes = 456;
        execution.gpu_workgroup_size = Some(128);
        execution.gpu_slates = 9;
        execution.gpu_largest_slate = Some(256);
        execution.gpu_slate_resets = 5;
        execution.gpu_lookahead_slates = 3;
        execution.gpu_lookahead = Some(zetesis_solve::GpuLookahead::Engaged);
        execution.gpu_lookahead_device_ns = 700;
        execution.gpu_lookahead_finish_wait_ns = 200;
        execution.gpu_lookahead_unmeasured = 1;
        execution.first_answer_ns = Some(4_000);
        report.formula_execution = Some(execution);
        if !completed {
            report.completion = Completion::Interrupted;
            report.interruption = Some(crate::Interruption::Countermodel(
                zetesis_stable::Incomplete::WorkLimit,
            ));
        }
        let text = every_prefix(&options, &Ok(report));
        assert!(text.contains("backend=hybrid GPU propagation + exact CPU residual search"));
        assert!(text.contains("adapter=FORMAT FIXTURE: no physical execution"));
        assert!(text.contains("batches=2; candidates=7; primitive work=123; completed sweeps=4"));
        assert!(text.contains(&format!(
            "pending candidates={pending}; queued verified models={queued}"
        )));
        assert!(text.contains("peak authored GPU bytes=456"));
        assert!(text.contains("workgroup size=128"));
        assert!(text.contains(
            "formula GPU lookahead: slates proposed ahead=3; last decision=engaged; overlapped device compute=700ns; finish waits=200ns; unmeasured submissions=1; device time hidden behind production is the compute less the waits, paired over the measured submissions"
        ));
        assert!(text.contains("formula GPU first answer: 4000ns after the device execution began"));
        assert!(text.contains("adaptive slates; initial and reset slate size=64"));
        assert!(text.contains("formula GPU slates: committed=9; largest=256; feedback resets=5"));
        assert!(text.contains("GPU compute passes: status=not_requested; elapsed_ns=unavailable"));
        assert!(text.contains("propagation sweeps/candidate=17; propagation work/candidate=789"));
        assert!(text.contains("submitted: batches=3; candidates=10"));
        assert!(!text.contains("Backend: gpu"));
        if !completed {
            assert!(!text.contains("completion: exhausted"));
        }
    }
}

#[test]
fn tight_statistics_report_routes_and_update_availability() {
    // Only rendering is exercised here, with authored values.
    let options = options(&["--oracle", "countermodel"]);
    let mut report = actual("a | b.", &options, &Cancellation::default()).unwrap();
    let mut execution = crate::FormulaExecutionStatistics::default();
    "FORMAT FIXTURE: no physical execution".clone_into(&mut execution.adapter);
    execution.tight_work_per_candidate = Some(500);
    execution.gpu_scheduled_work = Some(4_000);
    let mut routes = zetesis_solve::TightRouteStatistics::default();
    routes.complete = 4;
    routes.dense_reuse = 3;
    routes.beyond_threshold = 1;
    routes.update = 9;
    routes.nodes_evaluated = 85;
    execution.gpu_tight_routes = Some(routes);
    let mut updates = zetesis_solve::TightUpdateStatistics::default();
    updates.availability = zetesis_solve::TightUpdateAvailability::Bytes;
    updates.guard_declined_batches = 2;
    execution.gpu_tight_updates = Some(updates);
    report.formula_execution = Some(execution);
    let text = every_prefix(&options, &Ok(report));
    assert!(text.contains("tight GPU limits: work/candidate=500"));
    assert!(text.contains("tight GPU scheduled work=4000; performed by decoded batches"));
    assert!(text.contains(
        "tight GPU routes: complete=4; dense reuse=3 (beyond update threshold=1); update=9; nodes evaluated by update=85"
    ));
    assert!(
        text.contains("tight GPU update route: bytes; count width=unavailable; steady state=false; guard declined batches=2")
    );
}

#[test]
fn failed_statistics_preserve_missing_completion() {
    let options = options(&["--grounder", "lazy"]);
    let failure = run_finalized_with_diagnostics(
        "a.".into(),
        &options,
        &mut FailAt::new(b"Answer:"),
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap_err();
    let partial = failure.partial_report.as_deref().unwrap();
    assert_eq!(partial.completion, None);
    assert_eq!(partial.verified_models, 1);
    let view = super::Details::from(partial);
    assert!(!view.optimum_proved);
    assert_eq!(view.interruption, None);
    let text = every_prefix(&options, &Err(failure));
    assert!(text.contains("search completion=None"));
    assert!(!text.contains("  interruption:"));
    assert!(!text.contains("completion: exhausted"));
}

#[test]
fn exhausted_coverage_alone_cannot_label_an_incumbent_optimal() {
    let options = options(&[]);
    let mut report = actual(
        "1 {a;b} 1. #minimize{1,a:a;1,b:b}.",
        &options,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert!(report.optimization.is_some());
    assert!(report.optimum_proved);
    // Detached compatibility records can describe an incumbent without the
    // stronger optimum evidence. The renderer must read that explicit fact.
    report.optimum_proved = false;
    let mut bytes = Vec::new();
    super::write_detailed(&mut bytes, &options, Ok(&report), Duration::ZERO).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("objective: incumbent only"));
    assert!(!text.contains("objective: optimal"));
}

#[test]
fn incomplete_and_requested_model_statistics_do_not_claim_exhaustion_or_optimality() {
    let mut one_model = options(&["--stats"]);
    one_model.models = 1;
    let case = crate::publication_fixture::FormulaCase::new(one_model);
    let (report, _, text) = bounded_statistics("{a}.", &case);
    assert_eq!(report.completion, Completion::RequestedModels);
    assert!(text.contains("requested models reached (partial coverage)"));
    let mut bounded = crate::publication_fixture::FormulaCase::new(options(&[
        "--stats",
        "--oracle",
        "countermodel",
    ]));
    bounded.config.solve.max_candidates = 0;
    let (report, output, text) = bounded_statistics("{a}. #minimize{1:a}.", &bounded);
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(report.optimization.is_none());
    assert!(text.contains("interrupted (partial coverage)"));
    assert!(text.contains("objective: no retained score; evaluation counters=unavailable"));
    assert!(!text.contains("objective: optimal"));
    assert!(!String::from_utf8(output).unwrap().contains("OPTIMUM FOUND"));
    bounded.config.solve.max_candidates = 1;
    bounded.config.solve.max_objective_bound_work = 0;
    let (report, output, text) = bounded_statistics("1 {a;b} 1. #minimize{1,a:a;2,b:b}.", &bounded);
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(report.optimization.is_some());
    assert!(text.contains("objective: incumbent only"));
    assert!(!text.contains("objective: optimal"));
    assert!(!String::from_utf8(output).unwrap().contains("OPTIMUM FOUND"));
}

#[test]
fn frozen_positive_statistics_report_incomplete_attempts() {
    let mut reduct = zetesis_stable::ReductStatistics::default();
    reduct.frozen_positive_attempts = 3;
    reduct.frozen_positive_completed = 2;
    reduct.frozen_positive_preparation_work = 5;
    reduct.frozen_positive_work = 17;
    let statistics = zetesis_stable::Statistics {
        reduct,
        ..zetesis_stable::Statistics::default()
    };
    let mut output = Vec::new();
    super::countermodel(
        &mut output,
        &crate::SolveConfig::from(&options(&[])),
        &statistics,
        "stable models",
    )
    .unwrap();
    assert!(String::from_utf8(output).unwrap().contains(
        "  frozen positive reduct: attempts=3; completed=2; preparation work=5; work=17 (included in search work)\n"
    ));
}

#[test]
fn support_explanation_statistics_include_declined_work() {
    let mut statistics = zetesis_stable::RegionSearchStatistics::default();
    statistics.counts.support_explanations = zetesis_ferraris::SupportExplanationStatistics {
        attempts: 2,
        certified: 1,
        matched: 3,
        cut: 4,
        forced: 6,
        refuted: 1,
        declined: 1,
        work: 19,
        peak_bytes: 43,
    };
    let mut output = Vec::new();
    super::candidate_regions(&mut output, &statistics).unwrap();
    assert!(String::from_utf8(output).unwrap().contains(
        "  support explanations: attempts=2; certified=1; matched=3; cut=4; forced=6; refuted=1; declined=1; work=19 (included in reading work); peak worker bytes=43\n"
    ));
}
