//! CPU completion batches reach the same publication contracts as scalar sessions.

use crate::test_writer::BoundedWriter;
use crate::{
    Completion, FormulaExecutionStatistics, Options, PublicationFailure, PublicationReport,
    RunError, run_finalized_with_diagnostics,
};
use clap::Parser;
use std::{
    io::{self, Write},
    num::NonZeroUsize,
};
use zetesis_cpu::Cancellation;

fn options() -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--workers",
        "1",
        "--models",
        "0",
    ])
    .unwrap();
    options.batch_size = NonZeroUsize::new(3).unwrap();
    options.completion_workers = NonZeroUsize::new(4).unwrap();
    options
}

fn run(
    source: &str,
    options: &Options,
    cancellation: &Cancellation,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> Result<PublicationReport, PublicationFailure> {
    run_finalized_with_diagnostics(source.into(), options, output, diagnostics, cancellation)
        .and_then(crate::PublicationOutcome::into_legacy)
}

fn require_cpu_batches(statistics: &FormulaExecutionStatistics) {
    assert_eq!(statistics.completion.requested_workers, 4);
    assert_eq!(statistics.gpu_batches, 0);
    assert_eq!(statistics.gpu_candidates, 0);
    assert_eq!(statistics.gpu_work, 0);
}

fn records(output: &[u8]) -> Vec<Vec<String>> {
    let text = std::str::from_utf8(output).unwrap();
    let lines: Vec<_> = text.lines().collect();
    let mut records: Vec<_> = lines
        .windows(2)
        .filter(|pair| pair[0].starts_with("Answer:"))
        .map(|pair| {
            // Named fixture terms contain no embedded whitespace; duplicates remain.
            let mut atoms: Vec<_> = pair[1].split_whitespace().map(str::to_owned).collect();
            atoms.sort();
            atoms
        })
        .collect();
    records.sort();
    records
}

fn costs(output: &[u8]) -> Vec<Vec<i64>> {
    std::str::from_utf8(output)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("Optimization:"))
        .map(|line| {
            line.split_whitespace()
                .map(|cost| cost.parse().unwrap())
                .collect()
        })
        .collect()
}

#[test]
fn cpu_batches_preserve_complete_model_displays() {
    for source in [
        "",
        ":-.",
        "a | b.",
        "{a;b;c}. p:-p.",
        "1 {a;b;c} 1. #show.",
        "{p;-p}. #show x.",
        "{a;b}. #show a:a. #minimize{1,a:a;1,b:b}.",
    ] {
        let mut options = options();
        options.completion_workers = NonZeroUsize::MIN;
        let mut scalar = Vec::new();
        let expected = run(
            source,
            &options,
            &Cancellation::default(),
            &mut scalar,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(expected.semantic().formula_execution().is_none());
        options.completion_workers = NonZeroUsize::new(4).unwrap();
        let mut output = Vec::new();
        let actual = run(
            source,
            &options,
            &Cancellation::default(),
            &mut output,
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(actual.semantic().completion(), Some(Completion::Exhausted));
        assert_eq!(
            actual.publication().models(),
            expected.publication().models()
        );
        assert!(actual.publication().summary());
        assert_eq!(records(&output), records(&scalar), "{source}");
        assert_eq!(costs(&output), costs(&scalar), "{source}");
        let batch = actual.semantic().formula_execution().unwrap();
        require_cpu_batches(batch);
        assert_eq!((batch.pending_candidates, batch.queued_models), (0, 0));
        assert_eq!(batch.cpu_residuals, actual.semantic().candidate_progress());
        assert_eq!(
            batch.completion.entered,
            actual.semantic().candidate_progress()
        );
    }
}

#[test]
fn requested_publication_limit_reports_partial_coverage() {
    let mut options = options();
    options.models = 1;
    let mut output = Vec::new();
    let captured = run(
        "{a;b;c}.",
        &options,
        &Cancellation::default(),
        &mut output,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(captured.report().completion, Completion::RequestedModels);
    assert_eq!(
        (captured.report().models, captured.report().checked),
        (1, 3)
    );
    let execution = captured.semantic().formula_execution().unwrap();
    require_cpu_batches(execution);
    assert_eq!(execution.queued_models, 2);
    assert_eq!(records(&output).len(), 1);
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains("Coverage: partial"));
    assert!(!text.contains("Coverage: exhausted"));
}

#[test]
fn proposal_limit_publishes_an_incomplete_prefix() {
    let mut options = options();
    options.max_candidates = 2;
    let mut output = Vec::new();
    let captured = run(
        "{a;b;c}.",
        &options,
        &Cancellation::default(),
        &mut output,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(captured.report().completion, Completion::Interrupted);
    assert_eq!(
        (captured.report().models, captured.report().checked),
        (2, 2)
    );
    let execution = captured.semantic().formula_execution().unwrap();
    require_cpu_batches(execution);
    assert_eq!(execution.queued_models, 0);
    assert_eq!(records(&output).len(), 2);
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains("INCOMPLETE"));
    assert!(!text.contains("Coverage: exhausted"));
}

struct CancelOnAnswer {
    bytes: Vec<u8>,
    cancellation: Cancellation,
}
impl Write for CancelOnAnswer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        if bytes.starts_with(b"Answer:") {
            self.cancellation.cancel();
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn cancellation_prevents_queued_answer_publication() {
    let cancellation = Cancellation::default();
    let mut output = CancelOnAnswer {
        bytes: Vec::new(),
        cancellation: cancellation.clone(),
    };
    let captured = run(
        "{a;b;c}. #show x.",
        &options(),
        &cancellation,
        &mut output,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(captured.report().completion, Completion::Interrupted);
    assert_eq!(captured.publication().models(), 1);
    assert_eq!(records(&output.bytes), [vec!["x".to_owned()]]);
    let execution = captured.semantic().formula_execution().unwrap();
    require_cpu_batches(execution);
    assert_eq!(
        (execution.queued_models, execution.pending_candidates),
        (2, 0)
    );
    assert!(
        std::str::from_utf8(&output.bytes)
            .unwrap()
            .contains("INCOMPLETE")
    );
}

#[test]
fn hidden_optimal_ties_keep_display_multiplicity() {
    let source = "1 {a;b;c} 1. #minimize{1,a:a;1,b:b;2,c:c}. #show.";
    for enabled in [false, true] {
        let mut options = options();
        if !enabled {
            options.max_objective_bound_work = 0;
        }
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let captured = run(
            source,
            &options,
            &Cancellation::default(),
            &mut output,
            &mut diagnostics,
        )
        .unwrap();
        require_cpu_batches(captured.semantic().formula_execution().unwrap());
        assert_eq!(captured.report().completion, Completion::Exhausted);
        let optimum = captured.semantic().incumbent().unwrap();
        assert_eq!(optimum.tied_models, 2);
        assert_eq!(optimum.score.costs(), [(0, 1)]);
        assert_eq!(optimum.scored_models, 3);
        assert_eq!(records(&output), [Vec::<String>::new(), Vec::new()]);
        assert_eq!(costs(&output), [vec![1], vec![1]]);
        assert!(
            std::str::from_utf8(&output)
                .unwrap()
                .contains("OPTIMUM FOUND")
        );
        assert_eq!(
            std::str::from_utf8(&diagnostics)
                .unwrap()
                .contains("Objective pruning: bound"),
            enabled
        );
    }
}

#[test]
fn bounded_search_never_publishes_optimum_status() {
    for kind in 0..4 {
        let mut options = options();
        match kind {
            0 => options.max_objective_work = 0,
            1 => options.max_optimal_models = 1,
            2 => options.max_batch_bytes = Some(0),
            _ => options.max_search_work = 0,
        }
        let mut output = Vec::new();
        let captured = run(
            "1 {a;b;c} 1. #minimize{1,a:a;1,b:b;1,c:c}.",
            &options,
            &Cancellation::default(),
            &mut output,
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(captured.report().completion, Completion::Interrupted);
        assert!(!captured.semantic().optimum_proved());
        let text = std::str::from_utf8(&output).unwrap();
        assert!(text.contains("INCOMPLETE"));
        assert!(!text.contains("OPTIMUM FOUND"));
        assert!(!text.contains("Coverage: exhausted"));
        if kind == 1 {
            assert_eq!(captured.publication().models(), 1);
            assert_eq!(captured.semantic().incumbent().unwrap().tied_models, 2);
        }
    }
}

#[test]
fn every_publication_truncation_preserves_its_prefix() {
    for source in [
        "{a;b}. #show x.",
        "1 {a;b;c} 1. #minimize{1,a:a;1,b:b;2,c:c}.",
    ] {
        let options = options();
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let captured = run(
            source,
            &options,
            &Cancellation::default(),
            &mut output,
            &mut diagnostics,
        )
        .unwrap();
        require_cpu_batches(captured.semantic().formula_execution().unwrap());
        for (cut_diagnostics, reference) in [(false, &output), (true, &diagnostics)] {
            for capacity in 0..reference.len() {
                let mut broken = BoundedWriter::new(capacity);
                let mut other = Vec::new();
                let result = if cut_diagnostics {
                    run(
                        source,
                        &options,
                        &Cancellation::default(),
                        &mut other,
                        &mut broken,
                    )
                } else {
                    run(
                        source,
                        &options,
                        &Cancellation::default(),
                        &mut broken,
                        &mut other,
                    )
                };
                assert!(
                    matches!(result, Err(ref failure) if matches!(failure.cause.as_ref(), RunError::Output(error) if error.kind() == io::ErrorKind::BrokenPipe))
                );
                assert_eq!(broken.bytes(), &reference[..capacity]);
            }
        }
    }
}

#[test]
fn cpu_batches_preserve_original_corpus_displays() {
    every_original_case(crate::SearchMethod::Clauses, 1);
}

/// Four workers walking the region tree return the recorded answer sets,
/// atom for atom, on every corpus case, in whatever order they arrive.
#[test]
fn parallel_regions_preserve_original_corpus_displays() {
    every_original_case(crate::SearchMethod::Regions, 4);
}

/// The regions proposer returns the recorded answer sets, atom for atom, on
/// every corpus case, through the same batched route.
#[test]
fn cpu_batches_preserve_original_corpus_displays_under_regions() {
    every_original_case(crate::SearchMethod::Regions, 1);
}

fn every_original_case(search: crate::SearchMethod, workers: usize) {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/kr-domains/complete-models.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 94);
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/corpus/kr-domains");
    for case in cases {
        original_case(&root, case, search, workers);
    }
}

fn original_case(
    root: &std::path::Path,
    case: &serde_json::Value,
    search: crate::SearchMethod,
    workers: usize,
) {
    let path = case["path"].as_str().unwrap();
    let mut options = options();
    options.batch_size = NonZeroUsize::new(64).unwrap();
    options.search = search;
    options.workers = NonZeroUsize::new(workers).unwrap();
    let bundle = zetesis_themelios::SourceBundle::load(
        root.join(path),
        zetesis_themelios::BundleLimits::default(),
    )
    .unwrap();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let captured = crate::run_bundle_finalized_with_diagnostics(
        bundle,
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        captured.report().unwrap().completion,
        Completion::Exhausted,
        "{path}"
    );
    // Parallel regions decide their leaves on the workers: no batch runs.
    if workers == 1 {
        let execution = captured.semantic().formula_execution().unwrap();
        require_cpu_batches(execution);
        assert_eq!(
            (execution.pending_candidates, execution.queued_models),
            (0, 0),
            "{path}"
        );
        assert_eq!(
            execution.completion.entered,
            captured.semantic().candidate_progress(),
            "{path}"
        );
    }
    let expected = &case["answer"];
    assert_eq!(
        u64::try_from(captured.publication().models()).unwrap(),
        expected["model_count"].as_u64().unwrap(),
        "{path}"
    );
    let mut expected_models: Vec<Vec<String>> = expected["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let mut atoms: Vec<_> = row
                .as_array()
                .unwrap()
                .iter()
                .map(|atom| atom.as_str().unwrap().to_owned())
                .collect();
            atoms.sort();
            atoms
        })
        .collect();
    expected_models.sort();
    assert_eq!(records(&output), expected_models, "{path}");
    let expected_cost = expected["cost"].as_array().map(|cost| {
        cost.iter()
            .map(|value| value.as_i64().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        captured.semantic().incumbent().map(|score| score
            .score
            .costs()
            .iter()
            .map(|(_, value)| *value)
            .collect::<Vec<_>>()),
        expected_cost,
        "{path}"
    );
    assert_eq!(
        costs(&output),
        expected_cost.map_or_else(Vec::new, |cost| vec![cost; captured.publication().models()]),
        "{path}"
    );
    assert!(captured.publication().summary());
}
