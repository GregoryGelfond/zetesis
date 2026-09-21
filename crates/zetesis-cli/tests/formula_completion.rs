//! Explicit ordinary CPU completion pools preserve semantic and output contracts.

use std::num::NonZeroUsize;

use clap::Parser;
use zetesis_cli::{
    Completion, Interruption, Options, Report, RunError, run_detailed_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_sat::Incomplete;

#[path = "support/bounded_writer.rs"]
mod bounded_writer;
use bounded_writer::BoundedWriter;

/// The batched completion and its pool belong to the clause search; the
/// region walk decides its leaves in its workers.
fn options(workers: usize, batch: usize) -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--search",
        "clauses",
        "--oracle",
        "countermodel",
        "--models",
        "0",
        "--stats",
    ])
    .unwrap();
    options.completion_workers = NonZeroUsize::new(workers).unwrap();
    options.batch_size = NonZeroUsize::new(batch).unwrap();
    options
}

fn solve(source: &str, options: &Options) -> (Report, String, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = run_detailed_with_diagnostics(
        source.into(),
        options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    (
        result,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn answers(output: &str) -> Vec<&str> {
    let mut lines = output.lines();
    let mut result = Vec::new();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            result.push(lines.next().unwrap());
        }
    }
    result
}

#[test]
fn ordinary_cpu_pools_preserve_complete_ordered_answers_and_optimum_ties() {
    for source in [
        "{a;b;c}.",
        "a | b. a :- b. b :- a.",
        "{a;b}. c :- a. d :- b. :- c,d.",
        "{a;b}. #minimize { 0,a:a; 0,b:b }. #show.",
        "1 {a;b;c} 2. #minimize { 2,a:a; 1,b:b; 1,c:c }.",
        "a :- b. b :- a.",
        ":-.",
    ] {
        let reference = solve(source, &options(1, 3));
        assert_eq!(reference.0.completion, Completion::Exhausted);
        assert!(reference.0.formula_execution.is_none());
        for workers in [2, 4] {
            for batch in [1, 3, 7] {
                let actual = solve(source, &options(workers, batch));
                assert_eq!(actual.0.completion, Completion::Exhausted, "{source}");
                assert_eq!(answers(&actual.1), answers(&reference.1), "{source}");
                assert_eq!(actual.0.models, reference.0.models);
                assert_eq!(
                    actual
                        .0
                        .optimization
                        .as_ref()
                        .map(|o| (o.score.costs(), o.tied_models)),
                    reference
                        .0
                        .optimization
                        .as_ref()
                        .map(|o| (o.score.costs(), o.tied_models))
                );
                let execution = actual.0.formula_execution.unwrap();
                assert!(execution.adapter.is_empty());
                assert_eq!(
                    (
                        execution.gpu_batches,
                        execution.gpu_candidates,
                        execution.gpu_decided
                    ),
                    (0, 0, 0)
                );
                assert_eq!(
                    (execution.pending_candidates, execution.queued_models),
                    (0, 0)
                );
                assert_eq!(execution.cpu_residuals, actual.0.checked);
                assert_eq!(execution.completion.residuals, actual.0.checked);
                assert_eq!(execution.completion.residual_completed, actual.0.checked);
                assert_eq!(execution.completion.failed, 0);
                assert!(execution.completion.effective_workers <= workers.min(batch));
                assert!(
                    execution.completion.peak_scratch_bytes
                        <= zetesis_cli::SolveConfig::from(&options(workers, batch))
                            .max_completion_scratch_bytes
                );
                assert!(actual.2.contains("backend=cpu batched exact completion"));
                assert!(!actual.2.contains("backend=hybrid"));
                if actual.0.checked > 0 {
                    assert!(execution.completion.wall.unwrap().calls > 0);
                    assert_eq!(
                        execution.completion.worker_reduct.unwrap().calls,
                        actual.0.checked
                    );
                }
            }
        }
    }
}

#[test]
fn scratch_refusal_and_model_limits_report_pending_and_queued_coverage() {
    for workers in [2, 4] {
        let mut options = options(workers, 3);
        options.max_completion_scratch_bytes = Some(0);
        let limited = solve("{a;b}.", &options);
        assert_eq!(limited.0.completion, Completion::Interrupted);
        assert_eq!(
            limited.0.interruption,
            Some(Interruption::Countermodel(Incomplete::CompletionScratch))
        );
        assert_eq!(limited.0.models, 0);
        let execution = limited.0.formula_execution.unwrap();
        assert_eq!(execution.pending_candidates, 3);
        assert_eq!(execution.completion.entered, 0);
        assert_eq!(execution.completion.peak_scratch_bytes, 0);
        assert!(!limited.1.contains("UNSATISFIABLE"));
        options.max_completion_scratch_bytes = Some(256 * 1024 * 1024);
        options.models = 1;
        let limited = solve("{a;b}.", &options);
        assert_eq!(limited.0.completion, Completion::RequestedModels);
        assert_eq!(limited.0.models, 1);
        let execution = limited.0.formula_execution.unwrap();
        assert_eq!(
            (execution.pending_candidates, execution.queued_models),
            (0, 2)
        );
        assert_eq!(execution.completion.residual_completed, 3);
        assert!(execution.completion.requested_scratch_bytes > 0);
        assert!(execution.completion.peak_scratch_bytes > 0);
    }
}

#[test]
fn output_failure_preserves_verified_queued_models_after_join() {
    for workers in [2, 4] {
        let mut writer = BoundedWriter::new(0);
        let failure = run_detailed_with_diagnostics(
            "{a;b}.".into(),
            &options(workers, 3),
            &mut writer,
            &mut Vec::new(),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(matches!(*failure.cause, RunError::Output(_)));
        assert!(writer.bytes().is_empty());
        let partial = failure.partial_report.unwrap();
        assert_eq!(
            (
                partial.published_models,
                partial.verified_models,
                partial.checked
            ),
            (0, 3, 3)
        );
        let execution = partial.formula_execution.unwrap();
        assert_eq!(
            (execution.pending_candidates, execution.queued_models),
            (0, 2)
        );
        assert_eq!(execution.completion.residual_completed, 3);
    }
}

#[test]
fn default_scalar_cursor_keeps_its_existing_storage_contract() {
    let mut options = options(1, 3);
    options.max_completion_scratch_bytes = Some(0);
    let actual = solve("{a;b}.", &options);
    assert_eq!(actual.0.completion, Completion::Exhausted);
    assert_eq!(actual.0.models, 4);
    assert!(actual.0.formula_execution.is_none());
    assert!(
        actual
            .2
            .contains("batch-completion scratch limit=inapplicable")
    );
}

#[test]
fn native_parallel_statistics_do_not_claim_scalar_execution() {
    for workers in [4, 14] {
        let mut configured = options(1, 3);
        configured.search = zetesis_cli::SearchMethod::Regions;
        configured.workers = NonZeroUsize::new(workers).unwrap();
        configured.max_completion_scratch_bytes = Some(0);
        let actual = solve("{a;b}.", &configured);
        assert_eq!(actual.0.completion, Completion::Exhausted);
        let mut models = answers(&actual.1);
        models.sort_unstable();
        assert_eq!(models, ["", "a", "a b", "b"]);
        assert!(actual.0.formula_execution.is_none());
        assert!(
            actual
                .2
                .contains(&format!("Parallel regions: {workers} workers"))
        );
        assert!(
            actual
                .2
                .contains("native CPU search; batch-completion scratch limit=inapplicable")
        );
        assert!(!actual.2.contains("search workers=1"));
        assert!(!actual.2.contains("scalar cursor"));
    }
}

#[test]
fn negative_head_formulas_compose_with_bounded_parallel_completion() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../zetesis-themelios/tests/fixtures/negative-heads.json"
    ))
    .unwrap();
    for case in cases {
        let mut expected: Vec<Vec<String>> =
            serde_json::from_value(case["models"].clone()).unwrap();
        for model in &mut expected {
            model.sort();
        }
        expected.sort();
        for workers in [2, 4] {
            for batch in [1, 3] {
                let (report, output, _) =
                    solve(case["source"].as_str().unwrap(), &options(workers, batch));
                assert_eq!(report.completion, Completion::Exhausted);
                let mut actual: Vec<Vec<_>> = answers(&output)
                    .iter()
                    .map(|answer| {
                        let mut atoms: Vec<_> = answer.split_ascii_whitespace().collect();
                        atoms.sort_unstable();
                        atoms
                    })
                    .collect();
                actual.sort();
                assert_eq!(
                    actual, expected,
                    "{}: workers={workers}, batch={batch}",
                    case["name"]
                );
                assert_eq!(report.models, expected.len());
                let execution = report.formula_execution.unwrap();
                assert_eq!(execution.pending_candidates, 0);
                assert_eq!(execution.queued_models, 0);
                assert_eq!(execution.completion.residual_completed, report.checked);
                assert_eq!(execution.completion.failed, 0);
                assert_eq!(execution.gpu_candidates, 0);
            }
        }
    }
}
