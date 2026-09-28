//! Every interrupted statistics write must preserve exactly its accepted prefix.

use std::io;
use std::time::Duration;

use clap::Parser;

use crate::test_writer::BoundedWriter;
use crate::{
    Backend, Completion, Options, PublicationFailure, Report, RunError,
    run_finalized_with_diagnostics,
};
use zetesis_cpu::Cancellation;

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
        assert_eq!(
            error.to_string(),
            "diagnostic sink closed",
            "cut {capacity}"
        );
        assert_eq!(sink.bytes(), &reference[..capacity], "cut {capacity}");
    }
    for capacity in [reference.len(), reference.len() + 1] {
        let mut sink = BoundedWriter::new(capacity);
        super::write_detailed(&mut sink, options, outcome.as_ref(), elapsed).unwrap();
        assert_eq!(sink.bytes(), reference, "no extra bytes after a full write");
    }
    text.to_owned()
}

#[test]
fn every_completed_cpu_statistics_prefix_is_fallible_without_losing_bytes() {
    for (source, arguments, formula, optimum) in [
        ("p.", vec!["--grounder", "lazy"], false, false),
        ("p.", vec!["--grounder", "eager"], false, false),
        ("a | b.", vec![], true, false),
        ("1 {a;b} 1. #minimize{1,a:a;2,b:b}.", vec![], true, true),
    ] {
        let options = options(&arguments);
        let outcome = actual(source, &options, &Cancellation::default());
        let report = outcome.as_ref().unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.countermodel_statistics.is_some(), formula);
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
    assert!(text.contains("backend=cpu; oracle=closure; grounder=eager"));
}

#[test]
fn shared_cpu_statistics_preserve_every_writer_prefix() {
    for selection in ["union", "worlds"] {
        for stop in [None, Some("--max-work"), Some("--max-source-work")] {
            let mut arguments = vec!["--source-batching", selection];
            if let Some(limit) = stop {
                arguments.extend([limit, "0"]);
            }
            let options = options(&arguments);
            let outcome = actual("a.", &options, &Cancellation::default());
            let report = outcome.as_ref().unwrap();
            let stats = report.shared_execution.as_ref().unwrap();
            assert_eq!(stats.submitted_candidates, 1);
            assert_eq!(stats.completed_candidates, u64::from(stop.is_none()));
            assert_eq!(stats.stopped_candidates, u64::from(stop.is_some()));
            assert_eq!(report.models, usize::from(stop.is_none()));
            assert_eq!(
                report.completion,
                if stop.is_some() {
                    Completion::Interrupted
                } else {
                    Completion::Exhausted
                }
            );
            let text = every_prefix(&options, &outcome);
            assert!(text.contains("  shared CPU: source="));
            assert!(text.contains("  shared source: rounds="));
            assert!(text.contains("  CPU world evaluation: record visits plus antecedent tests="));
            assert!(text.contains("closure result/control records examined=1"));
            assert_eq!(
                text.contains("  shared batch interruption:"),
                stop.is_some()
            );
        }
    }
}

#[test]
fn lazy_metadata_rendering_preserves_every_writer_failure() {
    let options = options(&["--grounder", "lazy"]);
    let mut report = actual("p.", &options, &Cancellation::default()).unwrap();
    report.lazy_execution = Some(crate::output::fixtures::lazy_statistics());
    let text = every_prefix(&options, &Ok(report));
    assert!(text.contains("requested=metal; observed=Metal"));
    assert!(text.contains("submitted=7; completed=4; stopped=3; queued results=2"));
    assert!(text.contains("kernel time unmeasured"));
    assert!(text.contains("chunks allocating buffers=2; complete-set reuses=3; peak requested device bytes=1024 (RSS unmeasured)"));
    assert!(text.contains("buffer requests (allocated/reused): uniform=1/4; offsets=1/4; records=2/3; snapshots=1/4; seeds=1/4; output=2/3; readback=2/3; slack releases: budget=0; accounting overflow=0"));
    assert!(text.contains("FORMAT FIXTURE: no physical execution"));
}

#[test]
fn lazy_gpu_statistics_name_the_observed_grounder() {
    // This tests the view of an explicit device report, not physical execution.
    let mut options = options(&["--grounder", "lazy"]);
    let mut report = actual("p.", &options, &Cancellation::default()).unwrap();
    options.backend = Backend::Gpu(Some(zetesis_backend::GpuApi::Metal));
    report.lazy_execution = Some(crate::output::fixtures::lazy_statistics());
    let text = every_prefix(&options, &Ok(report));
    assert!(text.contains(
        "effective execution: oracle=closure; backend=requested GPU policy; grounder=lazy; see backend diagnostics for actual adapter"
    ));
    assert!(!text.contains("grounder=eager"));
}

#[test]
fn static_gpu_statistics_retain_eager_grounding() {
    // No physical device is invoked by this formatting control.
    let mut options = options(&["--grounder", "eager"]);
    let report = actual("p.", &options, &Cancellation::default()).unwrap();
    options.backend = Backend::Gpu(Some(zetesis_backend::GpuApi::Metal));
    let text = every_prefix(&options, &Ok(report));
    assert!(text.contains(
        "effective execution: oracle=closure; backend=requested GPU policy; grounder=eager; see backend diagnostics for actual adapter"
    ));
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
        let mut bounded = options(&["--oracle", "countermodel"]);
        bounded.max_candidates = candidates;
        bounded.max_objective_bound_work = 0;
        let outcome = actual(
            "1 {a;b} 1. #minimize{1,a:a;2,b:b}.",
            &bounded,
            &Cancellation::default(),
        );
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
    assert!(text.contains("oracle work: unavailable"));
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
        report.formula_execution = Some(crate::FormulaExecutionStatistics {
            completion: crate::CompletionAccounting::default(),
            gpu_residuals: None,
            tight_work_per_candidate: None,
            gpu_scheduled_work: None,
            gpu_limits: Some(crate::FormulaDeviceLimits {
                work_per_candidate: 789,
                rounds_per_candidate: 17,
            }),
            gpu_submitted_batches: 3,
            gpu_submitted_candidates: 10,
            adapter: "FORMAT FIXTURE: no physical execution".to_owned(),
            gpu_batches: 2,
            gpu_candidates: 7,
            gpu_work: 123,
            gpu_rounds: 4,
            cpu_residuals: 2,
            gpu_decided: 2,
            pending_candidates: pending,
            queued_models: queued,
            peak_accounted_bytes: 456,
        });
        if !completed {
            report.completion = Completion::Interrupted;
            report.interruption = Some(crate::Interruption::Countermodel(
                zetesis_sat::Incomplete::WorkLimit,
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
        assert!(text.contains("GPU kernel timing=unavailable"));
        assert!(text.contains("propagation sweeps/candidate=17; propagation work/candidate=789"));
        assert!(text.contains("submitted: batches=3; candidates=10"));
        assert!(!text.contains("Backend: gpu"));
        if !completed {
            assert!(!text.contains("completion: exhausted"));
        }
    }
}

#[test]
fn failed_statistics_preserve_missing_completion() {
    let options = options(&["--grounder", "lazy"]);
    let failure = run_finalized_with_diagnostics(
        "a.".into(),
        &options,
        &mut BoundedWriter::new(0),
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
