//! Common-loop failure evidence with real native membership and an injected checker.
//! The adapter label explicitly identifies a test double, never physical GPU evidence.

use std::io::{self, Write};
use std::num::NonZeroUsize;

use clap::Parser;
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Theory};
use zetesis_sat::{BatchVerdict, StableModels};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

use crate::formula_execution::{Failure, FormulaExecutionStatistics, MembershipExecution};
use crate::formula_queue::BatchQueue;
use crate::phase_timing::Recorder;
use crate::test_writer::BoundedWriter;
use crate::{Options, Report, RunError, RunFailure};

#[derive(Default)]
struct Injected {
    queue: BatchQueue,
    calls: usize,
    fail_on: usize,
    shape_on: usize,
}
impl Injected {
    fn check(&mut self, candidates: &[Interpretation]) -> Result<Vec<BatchVerdict>, Failure> {
        self.calls += 1;
        if self.calls == self.fail_on {
            return Err(Failure::Run(RunError::Output(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "original checker transport failure",
            ))));
        }
        let count = candidates.len() - usize::from(self.calls == self.shape_on);
        Ok(vec![BatchVerdict::Residual; count])
    }
}
impl MembershipExecution for Injected {
    fn next(
        &mut self,
        models: &mut StableModels,
        options: &crate::SolveConfig,
        control: &Control,
        _: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        let mut queue = std::mem::take(&mut self.queue);
        let result = queue.next(models, options, control, |_: &Theory, candidates| {
            self.check(candidates)
        });
        self.queue = queue;
        result
    }
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics> {
        let batch = models.batch_statistics();
        Some(FormulaExecutionStatistics {
            adapter: "injected native checker; no physical device".into(),
            gpu_batches: 0,
            gpu_candidates: 0,
            gpu_work: 0,
            gpu_rounds: 0,
            gpu_decided: 0,
            cpu_residuals: batch.residuals,
            pending_candidates: batch.pending,
            queued_models: self.queue.len(),
            peak_accounted_bytes: 0,
            completion: self.queue.accounting(),
        })
    }
}

fn options() -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--models",
        "0",
    ])
    .unwrap();
    options.batch_size = NonZeroUsize::new(2).unwrap();
    options.max_objective_bound_work = 0;
    options
}

fn run(
    source: &str,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    execution: &mut Injected,
) -> Result<Report, RunFailure> {
    let admitted = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let control = Control::default();
    let phases = Recorder::new(options.stats);
    let result = super::FormulaRun {
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
            control: &control,
        },
    }
    .solve(output, diagnostics, execution, &phases);
    crate::driver::report_statistics(result, diagnostics, options, &phases)
}

#[test]
fn a_late_checker_failure_preserves_prior_answers_and_uncommitted_candidates() {
    let mut output = Vec::new();
    let failure = run(
        "{a;b}.",
        &options(),
        &mut output,
        &mut Vec::new(),
        &mut Injected {
            fail_on: 2,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(&*failure.cause, RunError::Output(error)
        if error.kind() == io::ErrorKind::ConnectionReset && error.to_string() == "original checker transport failure"));
    let partial = failure.partial_report.unwrap();
    assert_eq!(
        (
            partial.published_models,
            partial.verified_models,
            partial.checked
        ),
        (2, 2, 4)
    );
    assert_eq!(partial.completion, None);
    assert!(!partial.summary_published);
    let execution = partial.formula_execution.unwrap();
    assert_eq!(
        (
            execution.pending_candidates,
            execution.queued_models,
            execution.cpu_residuals
        ),
        (2, 0, 2)
    );
    let text = std::str::from_utf8(&output).unwrap();
    assert_eq!(text.matches("Answer:").count(), 2);
    assert!(!text.contains("Coverage:"));
}

#[test]
fn checker_failure_does_not_flush_retained_incumbents_or_lose_tie_metadata() {
    let mut output = Vec::new();
    let failure = run(
        "{a;b}. #minimize{0:a;0:b}. #show.",
        &options(),
        &mut output,
        &mut Vec::new(),
        &mut Injected {
            fail_on: 2,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(output.is_empty());
    let partial = failure.partial_report.unwrap();
    assert_eq!((partial.published_models, partial.verified_models), (0, 2));
    assert_eq!(partial.formula_execution.unwrap().pending_candidates, 2);
    let incumbent = partial.optimization.unwrap();
    assert_eq!((incumbent.tied_models, incumbent.scored_models), (2, 2));
    assert_eq!(incumbent.score.costs(), &[(0, 0)]);
    assert_eq!(partial.completion, None);
}

#[test]
fn protocol_shape_and_secondary_reporting_failures_retain_the_primary_and_pending_set() {
    let mut options = options();
    options.stats = true;
    let mut initial_diagnostics = Vec::new();
    run(
        "{a;b}.",
        &options,
        &mut Vec::new(),
        &mut initial_diagnostics,
        &mut Injected {
            shape_on: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    let prefix = std::str::from_utf8(&initial_diagnostics)
        .unwrap()
        .find("Statistics:")
        .unwrap();
    let failure = run(
        "{a;b}.",
        &options,
        &mut Vec::new(),
        &mut BoundedWriter::new(prefix),
        &mut Injected {
            shape_on: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        *failure.cause,
        RunError::FormulaBatchShape {
            expected: 2,
            actual: 1
        }
    ));
    assert_eq!(
        failure.secondary_output.unwrap().kind(),
        io::ErrorKind::BrokenPipe
    );
    let partial = failure.partial_report.unwrap();
    assert_eq!(
        (
            partial.verified_models,
            partial.published_models,
            partial.checked
        ),
        (0, 0, 2)
    );
    assert_eq!(partial.formula_execution.unwrap().pending_candidates, 2);
    assert!(failure.phase_timings.is_some());
}

#[test]
fn partial_answer_retains_verified_queue_separately_from_the_published_count() {
    let mut options = options();
    options.batch_size = NonZeroUsize::new(3).unwrap();
    let mut output = BoundedWriter::new(4);
    let failure = run(
        "{a;b}.",
        &options,
        &mut output,
        &mut Vec::new(),
        &mut Injected::default(),
    )
    .unwrap_err();
    let partial = failure.partial_report.unwrap();
    assert_eq!(output.bytes(), b"Answ");
    assert_eq!((partial.published_models, partial.verified_models), (0, 3));
    let execution = partial.formula_execution.unwrap();
    assert_eq!(
        (execution.pending_candidates, execution.queued_models),
        (0, 2)
    );
    assert_eq!(partial.countermodel_statistics.unwrap().stable_models, 3);
    assert_eq!(partial.completion, None);
}

#[test]
fn failed_device_shaped_checks_do_not_recount_previous_parallel_completion() {
    for workers in [1, 2, 4] {
        for shape in [false, true] {
            let mut options = options();
            options.completion_workers = NonZeroUsize::new(workers).unwrap();
            let mut execution = Injected {
                queue: BatchQueue::new(&(&options).into()).unwrap(),
                fail_on: if shape { 0 } else { 2 },
                shape_on: if shape { 2 } else { 0 },
                ..Default::default()
            };
            let failure = run(
                "{a;b}.",
                &options,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut execution,
            )
            .unwrap_err();
            let partial = failure.partial_report.unwrap();
            let statistics = partial.formula_execution.unwrap();
            assert_eq!(statistics.completion.entered, 2);
            assert_eq!(statistics.completion.residual_completed, 2);
            assert_eq!(statistics.completion.failed, 0);
            assert_eq!(statistics.pending_candidates, 2);
            assert_eq!((partial.published_models, partial.verified_models), (2, 2));
        }
    }
}
