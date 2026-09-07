//! The production queue/common loop tested with actual exact native membership.
//! No device is constructed, and these tests make no GPU execution/parity claim.

use std::io::{self, Write};
use std::num::NonZeroUsize;

use clap::Parser;
use zetesis_cpu::Control;
use zetesis_ferraris::Interpretation;
use zetesis_sat::{BatchStatistics, BatchVerdict, Incomplete, StableModels, Statistics};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

use crate::formula_execution::{Failure, FormulaExecutionStatistics, MembershipExecution};
use crate::formula_queue::BatchQueue;
use crate::test_writer::BoundedWriter;
use crate::{Completion, Options, Report, RunError, run_with_diagnostics};

#[derive(Default)]
struct NativeBatch {
    queue: BatchQueue,
    snapshots: Vec<(BatchStatistics, Statistics, usize)>,
    proposed: Vec<Vec<Vec<usize>>>,
    fail_work: bool,
    omit_verdict: bool,
}

impl MembershipExecution for NativeBatch {
    fn next(
        &mut self,
        models: &mut StableModels,
        options: &crate::SolveConfig,
        control: &Control,
        _: &crate::phase_timing::Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        let result = self
            .queue
            .next(models, options, control, |theory, candidates| {
                self.proposed.push(
                    candidates
                        .iter()
                        .map(|candidate| candidate.atoms().collect())
                        .collect(),
                );
                if self.fail_work {
                    let limits = zetesis_sat::Limits {
                        search: zetesis_sat::SearchLimits {
                            max_work: 0,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    if let zetesis_sat::Check::Inconclusive(error) =
                        zetesis_sat::check(theory, &candidates[0], limits, control)
                    {
                        return Err(Failure::Search(error));
                    }
                    panic!("the selected nonempty source needs native encoding work");
                }
                // Declining partial propagation sends every original candidate to
                // the real exact native reduct checker inside StableModels.
                let mut verdicts = Vec::new();
                verdicts
                    .try_reserve_exact(candidates.len())
                    .map_err(|_| Failure::Search(Incomplete::Allocation))?;
                verdicts.resize(candidates.len(), BatchVerdict::Residual);
                if self.omit_verdict {
                    verdicts.pop();
                }
                Ok(verdicts)
            });
        self.snapshots.push((
            models.batch_statistics(),
            models.statistics(),
            self.queue.len(),
        ));
        result
    }

    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        // Actual native batches have no physical adapter, GPU work or upload counters.
        None
    }
}

fn options() -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--workers",
        "1",
    ])
    .unwrap();
    options.models = 0;
    options.batch_size = NonZeroUsize::new(3).unwrap();
    options
}

fn run(
    source: &str,
    options: &Options,
    control: &Control,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    execution: &mut NativeBatch,
) -> Result<Report, RunError> {
    let admitted = admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    super::FormulaRun {
        input: super::Input {
            theory: admitted.theory(),
            atoms: admitted.atoms(),
            gate_atoms: 0,
            objectives: admitted.objectives(),
            observations: admitted.metadata().observations(),
        },
        display: crate::display::Display {
            selection: admitted.metadata().output(),
            observations: admitted.metadata().observations(),
            options,
            control,
        },
    }
    .solve(
        output,
        diagnostics,
        execution,
        &crate::phase_timing::Recorder::new(options.stats),
    )
    .map(|progress| progress.report)
    .map_err(|failure| *failure.cause)
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

#[test]
fn native_batches_share_the_complete_model_and_display_loop() {
    for source in [
        "",
        ":-.",
        "a | b.",
        "{a;b;c}. p:-p.",
        "1 {a;b;c} 1. #show.",
        "{p;-p}. #show x.",
        "{a;b}. #show a:a. #minimize{1,a:a;1,b:b}.",
    ] {
        let options = options();
        let mut scalar = Vec::new();
        let expected = run_with_diagnostics(
            source.into(),
            &options,
            &mut scalar,
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut execution = NativeBatch::default();
        let actual = run(
            source,
            &options,
            &Control::default(),
            &mut output,
            &mut Vec::new(),
            &mut execution,
        )
        .unwrap();
        assert_eq!(actual.completion, Completion::Exhausted);
        assert_eq!(actual.models, expected.models);
        assert_eq!(records(&output), records(&scalar), "{source}");
        let (batch, search, queued) = execution.snapshots.last().unwrap();
        assert_eq!((batch.pending, *queued), (0, 0));
        assert_eq!(batch.committed, search.candidates);
        assert_eq!(batch.residuals, search.candidates);
        assert_eq!(batch.propagated, 0);
        assert!(execution.proposed.iter().all(|batch| batch.len() <= 3));
        assert!(actual.formula_execution.is_none());
    }
}

#[test]
fn requested_output_and_delayed_proposal_limits_preserve_queued_accounting() {
    let mut options = options();
    options.models = 1;
    let mut execution = NativeBatch::default();
    let report = run(
        "{a;b;c}.",
        &options,
        &Control::default(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut execution,
    )
    .unwrap();
    assert_eq!(report.completion, Completion::RequestedModels);
    assert_eq!((report.models, report.checked), (1, 3));
    assert_eq!(execution.queue.len(), 2);
    assert_eq!(execution.snapshots[0].0.committed, 3);
    options.models = 0;
    options.max_candidates = 2;
    let mut output = Vec::new();
    let mut execution = NativeBatch::default();
    let report = run(
        "{a;b;c}.",
        &options,
        &Control::default(),
        &mut output,
        &mut Vec::new(),
        &mut execution,
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!((report.models, report.checked), (2, 2));
    assert_eq!(execution.queue.len(), 0);
    assert!(std::str::from_utf8(&output).unwrap().contains("INCOMPLETE"));
    assert!(
        !std::str::from_utf8(&output)
            .unwrap()
            .contains("coverage=exhausted")
    );
}

struct CancelOnAnswer {
    bytes: Vec<u8>,
    control: Control,
}
impl Write for CancelOnAnswer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        if bytes.starts_with(b"Answer:") {
            self.control.cancel();
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn cancellation_after_a_buffered_answer_does_not_publish_queued_models() {
    let control = Control::default();
    let mut output = CancelOnAnswer {
        bytes: Vec::new(),
        control: control.clone(),
    };
    let mut execution = NativeBatch::default();
    let report = run(
        "{a;b;c}. #show x.",
        &options(),
        &control,
        &mut output,
        &mut Vec::new(),
        &mut execution,
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(report.models, 1);
    assert_eq!(records(&output.bytes).len(), 1);
    assert_eq!(execution.queue.len(), 2);
    assert_eq!(execution.snapshots.last().unwrap().0.pending, 0);
    assert!(
        std::str::from_utf8(&output.bytes)
            .unwrap()
            .contains("INCOMPLETE")
    );
}

#[test]
fn objective_updates_score_already_queued_models_and_keep_all_hidden_optimal_ties() {
    let source = "1 {a;b;c} 1. #minimize{1,a:a;1,b:b;2,c:c}. #show.";
    let mut expected = None;
    for enabled in [false, true] {
        let mut options = options();
        if !enabled {
            options.max_objective_bound_work = 0;
        }
        let mut execution = NativeBatch::default();
        let mut output = Vec::new();
        let report = run(
            source,
            &options,
            &Control::default(),
            &mut output,
            &mut Vec::new(),
            &mut execution,
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        let optimum = report.optimization.unwrap();
        assert_eq!(optimum.tied_models, 2);
        assert_eq!(optimum.score.costs(), &[(0, 1)]);
        assert_eq!(records(&output), vec![Vec::<String>::new(), Vec::new()]);
        assert_eq!(optimum.scored_models, 3);
        if enabled {
            assert!(execution.snapshots.iter().any(|(_, statistics, queued)| {
                statistics.candidate_restrictions > 0 && *queued > 0
            }));
        }
        if let Some(expected) = &expected {
            assert_eq!(records(&output), *expected);
        } else {
            expected = Some(records(&output));
        }
    }
}

#[test]
fn real_scoring_retention_and_native_work_stops_never_claim_optimality() {
    for kind in 0..4 {
        let mut options = options();
        let mut execution = NativeBatch::default();
        match kind {
            0 => options.max_objective_work = 0,
            1 => options.max_optimal_models = 1,
            2 => options.max_batch_bytes = 0,
            _ => execution.fail_work = true,
        }
        let mut output = Vec::new();
        let report = run(
            "1 {a;b;c} 1. #minimize{1,a:a;1,b:b;1,c:c}.",
            &options,
            &Control::default(),
            &mut output,
            &mut Vec::new(),
            &mut execution,
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        let text = std::str::from_utf8(&output).unwrap();
        assert!(!text.contains("OPTIMUM FOUND"));
        assert!(!text.contains("coverage=exhausted"));
        if kind == 1 {
            assert_eq!(report.models, 1);
            assert_eq!(report.optimization.unwrap().tied_models, 2);
        }
        if kind == 3 {
            assert_eq!(execution.snapshots.last().unwrap().0.pending, 3);
            assert_eq!(report.models, 0);
        }
    }
}

#[test]
fn malformed_checker_shape_is_a_real_protocol_error_before_any_answer() {
    let mut execution = NativeBatch {
        omit_verdict: true,
        ..NativeBatch::default()
    };
    let mut output = Vec::new();
    let error = run(
        "{a;b;c}.",
        &options(),
        &Control::default(),
        &mut output,
        &mut Vec::new(),
        &mut execution,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        RunError::FormulaBatchShape {
            expected: 3,
            actual: 2
        }
    ));
    assert!(output.is_empty());
    assert_eq!(execution.snapshots.last().unwrap().0.pending, 3);
}

#[test]
fn every_common_loop_output_and_diagnostic_truncation_keeps_its_exact_prefix() {
    for source in [
        "{a;b}. #show x.",
        "1 {a;b;c} 1. #minimize{1,a:a;1,b:b;2,c:c}.",
    ] {
        let options = options();
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        run(
            source,
            &options,
            &Control::default(),
            &mut output,
            &mut diagnostics,
            &mut NativeBatch::default(),
        )
        .unwrap();
        for (cut_diagnostics, reference) in [(false, &output), (true, &diagnostics)] {
            for capacity in 0..reference.len() {
                let mut broken = BoundedWriter::new(capacity);
                let mut other = Vec::new();
                let result = if cut_diagnostics {
                    run(
                        source,
                        &options,
                        &Control::default(),
                        &mut other,
                        &mut broken,
                        &mut NativeBatch::default(),
                    )
                } else {
                    run(
                        source,
                        &options,
                        &Control::default(),
                        &mut broken,
                        &mut other,
                        &mut NativeBatch::default(),
                    )
                };
                assert!(
                    matches!(result, Err(RunError::Output(ref error)) if error.kind() == io::ErrorKind::BrokenPipe)
                );
                assert_eq!(broken.bytes(), &reference[..capacity]);
            }
        }
    }
}

#[test]
fn all_original_corpus_contracts_hold_through_native_residual_batches() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/kr-domains/complete-models.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 94);
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/corpus/kr-domains");
    for case in cases {
        original_case(&root, case);
    }
}

fn original_case(root: &std::path::Path, case: &serde_json::Value) {
    let path = case["path"].as_str().unwrap();
    let mut options = options();
    options.batch_size = NonZeroUsize::new(64).unwrap();
    let bundle = zetesis_themelios::SourceBundle::load(
        root.join(path),
        zetesis_themelios::BundleLimits {
            max_roots: options.max_source_roots,
            max_files: options.max_source_files,
            max_file_bytes: options.max_source_bytes,
            max_total_bytes: options.max_total_source_bytes,
            max_include_depth: options.max_include_depth,
        },
    )
    .unwrap();
    let admitted = zetesis_themelios::admit_bundle_formula(
        bundle,
        zetesis_themelios::BundleAdmissionOptions::default(),
        crate::admission::expansion_limits(&options),
        crate::admission::formula_limits(&options),
    )
    .unwrap();
    let control = Control::default();
    let context = super::FormulaRun {
        input: super::Input {
            theory: admitted.theory(),
            atoms: admitted.atoms(),
            gate_atoms: 0,
            objectives: admitted.objectives(),
            observations: admitted.metadata().observations(),
        },
        display: crate::display::Display {
            selection: admitted.metadata().output(),
            observations: admitted.metadata().observations(),
            options: &options,
            control: &control,
        },
    };
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let mut execution = NativeBatch::default();
    let report = context
        .solve(
            &mut output,
            &mut diagnostics,
            &mut execution,
            &crate::phase_timing::Recorder::new(options.stats),
        )
        .unwrap()
        .report;
    assert_eq!(
        report.completion,
        Completion::Exhausted,
        "{path}: {}\n{}",
        std::str::from_utf8(&output).unwrap(),
        std::str::from_utf8(&diagnostics).unwrap()
    );
    let expected = &case["answer"];
    assert_eq!(
        u64::try_from(report.models).unwrap(),
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
        report.optimization.map(|score| score
            .score
            .costs()
            .iter()
            .map(|(_, value)| *value)
            .collect::<Vec<_>>()),
        expected_cost,
        "{path}"
    );
    let (batch, statistics, queued) = execution.snapshots.last().unwrap();
    assert_eq!(batch.committed, statistics.candidates, "{path}");
    assert_eq!((batch.pending, *queued), (0, 0), "{path}");
}
