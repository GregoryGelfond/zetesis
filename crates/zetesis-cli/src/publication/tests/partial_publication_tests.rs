//! Publication failures retain semantic evidence from real CPU completion batches.

use crate::{Completion, Options, PublicationFailure, RunError, run_finalized_with_diagnostics};
use clap::Parser;
use std::{
    io::{self, Write},
    num::NonZeroUsize,
};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::BoundedWriter;

fn options() -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        // The batched completion protocol is the clause method's.
        "--search",
        "clauses",
        "--models",
        "0",
    ])
    .unwrap();
    options.batch_size = NonZeroUsize::new(3).unwrap();
    options.completion_workers = NonZeroUsize::new(4).unwrap();
    options
}

struct FailAnswer {
    limit: usize,
    accepted: usize,
    bytes: Vec<u8>,
}
impl FailAnswer {
    fn after(limit: usize) -> Self {
        Self {
            limit,
            accepted: 0,
            bytes: Vec::new(),
        }
    }
}
impl Write for FailAnswer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.starts_with(b"Answer:") {
            if self.accepted == self.limit {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionReset,
                    "answer sink closed",
                ));
            }
            self.accepted += 1;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn run(
    source: &str,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> PublicationFailure {
    run_finalized_with_diagnostics(
        source.into(),
        options,
        output,
        diagnostics,
        &Cancellation::default(),
    )
    .unwrap_err()
}

fn require_cpu_batches(failure: &PublicationFailure) {
    let execution = failure.semantic().unwrap().formula_execution().unwrap();
    assert_eq!(execution.completion.requested_workers, 4);
    assert_eq!(execution.gpu_batches, 0);
    assert_eq!(execution.gpu_work, 0);
}

fn require_answer_error(failure: &PublicationFailure) {
    assert!(matches!(failure.cause.as_ref(), RunError::Output(error)
        if error.kind() == io::ErrorKind::ConnectionReset && error.to_string() == "answer sink closed"));
}

#[test]
fn late_output_failure_retains_published_prefix() {
    let mut output = FailAnswer::after(2);
    let failure = run("{a;b;c}.", &options(), &mut output, &mut Vec::new());
    require_answer_error(&failure);
    require_cpu_batches(&failure);
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!(
        (
            partial.published_models,
            partial.verified_models,
            partial.checked
        ),
        (2, 3, 3)
    );
    assert_eq!(partial.completion, None);
    assert!(!partial.summary_published);
    assert_eq!(failure.publication().unwrap().models(), 2);
    let text = std::str::from_utf8(&output.bytes).unwrap();
    assert_eq!(text.matches("Answer:").count(), 2);
    assert!(!text.contains("Coverage:"));
    let execution = failure.semantic().unwrap().formula_execution().unwrap();
    assert_eq!(
        (
            execution.pending_candidates,
            execution.queued_models,
            execution.cpu_residuals
        ),
        (0, 0, 3)
    );
}

#[derive(Default)]
struct RefuseBound {
    bytes: Vec<u8>,
    refusals: usize,
}
impl Write for RefuseBound {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.starts_with(b"Objective pruning:") {
            self.refusals += 1;
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "bound diagnostic closed",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn failed_bound_diagnostics_leave_incumbents_unpublished() {
    let mut output = Vec::new();
    let mut diagnostics = RefuseBound::default();
    let failure = run(
        "1 {a;b;c} 1. #minimize{1,a:a;1,b:b;2,c:c}. #show.",
        &options(),
        &mut output,
        &mut diagnostics,
    );
    assert!(matches!(failure.cause.as_ref(), RunError::Output(error)
        if error.kind() == io::ErrorKind::BrokenPipe && error.to_string() == "bound diagnostic closed"));
    assert_eq!(diagnostics.refusals, 1);
    require_cpu_batches(&failure);
    assert!(output.is_empty());
    let semantic = failure.semantic().unwrap();
    assert_eq!(semantic.verified_models(), 3);
    assert_eq!(semantic.retained_models(), 1);
    assert_eq!(semantic.incumbent().unwrap().tied_models, 1);
    assert_eq!(semantic.incumbent().unwrap().scored_models, 1);
    assert_eq!(semantic.incumbent().unwrap().score.costs(), [(0, 1)]);
    assert_eq!(semantic.completion(), None);
    assert!(!semantic.optimum_proved());
    assert_eq!(failure.publication().unwrap().models(), 0);
    assert!(!failure.publication().unwrap().summary());
    let execution = semantic.formula_execution().unwrap();
    assert_eq!(
        (execution.pending_candidates, execution.queued_models),
        (0, 2)
    );
}

#[test]
fn secondary_reporting_failure_preserves_primary_cause() {
    let mut options = options();
    options.stats = true;
    let mut reference = Vec::new();
    let initial = run(
        "{a;b}.",
        &options,
        &mut FailAnswer::after(0),
        &mut reference,
    );
    require_answer_error(&initial);
    let prefix = std::str::from_utf8(&reference)
        .unwrap()
        .find("Statistics:")
        .unwrap();
    let mut diagnostics = BoundedWriter::new(prefix);
    let failure = run(
        "{a;b}.",
        &options,
        &mut FailAnswer::after(0),
        &mut diagnostics,
    );
    require_answer_error(&failure);
    require_cpu_batches(&failure);
    assert_eq!(
        failure.secondary_output.as_ref().unwrap().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        failure.diagnostics_failure().unwrap().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(diagnostics.bytes(), &reference[..prefix]);
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!(
        (
            partial.verified_models,
            partial.published_models,
            partial.checked
        ),
        (3, 0, 3)
    );
    assert_eq!(partial.formula_execution.as_ref().unwrap().queued_models, 2);
    assert_eq!(partial.completion, None);
    assert!(failure.phase_timings.is_some());
}

#[test]
fn partial_answer_retains_unpublished_membership() {
    let mut output = BoundedWriter::new(4);
    let failure = run("{a;b}.", &options(), &mut output, &mut Vec::new());
    require_cpu_batches(&failure);
    assert!(
        matches!(failure.cause.as_ref(), RunError::Output(error) if error.kind() == io::ErrorKind::BrokenPipe)
    );
    assert_eq!(output.bytes(), b"Answ");
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!((partial.published_models, partial.verified_models), (0, 3));
    let execution = partial.formula_execution.as_ref().unwrap();
    assert_eq!(
        (execution.pending_candidates, execution.queued_models),
        (0, 2)
    );
    assert_eq!(partial.countermodel_statistics.unwrap().stable_models, 3);
    assert_eq!(partial.completion, None);
    assert_eq!(failure.publication().unwrap().models(), 0);
    assert!(!failure.publication().unwrap().summary());
}

#[test]
fn failed_answer_output_preserves_complete_optimum() {
    let mut options = options();
    options.max_objective_bound_work = 0;
    let failure = run(
        "{a;b}. #minimize{0:a;0:b}. #show.",
        &options,
        &mut FailAnswer::after(0),
        &mut Vec::new(),
    );
    require_answer_error(&failure);
    require_cpu_batches(&failure);
    let semantic = failure.semantic().unwrap();
    assert_eq!(semantic.completion(), Some(Completion::Exhausted));
    assert!(semantic.optimum_proved());
    assert_eq!(semantic.verified_models(), 4);
    assert_eq!(semantic.retained_models(), 4);
    assert_eq!(semantic.incumbent().unwrap().tied_models, 4);
    assert_eq!(semantic.incumbent().unwrap().score.costs(), [(0, 0)]);
    assert_eq!(failure.publication().unwrap().models(), 0);
    assert!(!failure.publication().unwrap().summary());
}
