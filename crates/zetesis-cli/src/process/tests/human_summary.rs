//! Human terminal flushing preserves cross-stream order and independent evidence.

use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};

use clap::Parser;

use super::{Sink, buffered_output, finish_output};
use crate::presentation::Diagnostics;
use crate::{
    ColorMode, Completion, Options, RunError, StatisticsView, run_finalized_with_diagnostics,
};
use zetesis_cpu::Cancellation;

fn options(stats: bool) -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--models",
        "0",
        "--grounder",
        "eager",
    ])
    .unwrap();
    options.stats = stats;
    options.statistics_view = StatisticsView::Human;
    options
}

#[derive(Clone, Default)]
struct SharedBytes(Rc<RefCell<Vec<u8>>>);

impl Write for SharedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn buffered_human_summary_precedes_statistics() {
    // Both handles reach the same destination, as with merged redirected
    // stdout/stderr. Only stdout has the process's bounded staging buffer.
    let sink = SharedBytes::default();
    let mut output = buffered_output(sink.clone(), false);
    let mut diagnostics = sink.clone();
    let outcome = run_finalized_with_diagnostics(
        "p.".into(),
        &options(true),
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(outcome.semantic().completion(), Some(Completion::Exhausted));
    assert!(output.buffer().is_empty());
    let text = String::from_utf8(sink.0.borrow().clone()).unwrap();
    let answer = text.find("Answer: 1\np\n").unwrap();
    let timing = text.find("SATISFIABLE\nModels: 1\n\nTime: ").unwrap();
    let statistics = text.find("\nStatistics\n").unwrap();
    assert!(answer < timing && timing < statistics, "{text}");
}

#[test]
fn summary_flush_failure_retains_optimality() {
    let mut sink = Sink {
        fail_flush: true,
        ..Sink::default()
    };
    let mut output = buffered_output(&mut sink, false);
    let failure = run_finalized_with_diagnostics(
        "p. #minimize{0:p}.".into(),
        &options(false),
        &mut output,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause.as_ref(), RunError::Output(error)
        if error.to_string() == "sink flush failed"));
    let semantic = failure.semantic().unwrap();
    assert_eq!(semantic.completion(), Some(Completion::Exhausted));
    assert!(semantic.optimum_proved());
    let publication = failure.publication().unwrap();
    assert_eq!(publication.models, 1);
    assert!(!publication.summary);
    // The sink accepted the bytes before its flush failed. Acceptance does not
    // justify acknowledging the terminal callback that returned the error.
    let (_, pending) = output.into_parts();
    assert!(pending.unwrap().is_empty());
    assert_eq!(sink.flushes, 1);
    assert!(
        std::str::from_utf8(&sink.bytes)
            .unwrap()
            .contains("OPTIMUM FOUND")
    );
}

#[test]
fn successful_process_flush_preserves_publication_failure() {
    let mut sink = Sink {
        fail_write: true,
        ..Sink::default()
    };
    let mut output = buffered_output(&mut sink, false);
    let failure = run_finalized_with_diagnostics(
        "p.".into(),
        &options(false),
        &mut output,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause.as_ref(), RunError::Output(error)
        if error.to_string() == "sink write failed"));
    assert_eq!(
        failure.semantic().unwrap().completion(),
        Some(Completion::Exhausted)
    );
    assert!(!failure.publication().unwrap().summary);
    assert!(!output.buffer().is_empty());
    // The existing final process flush may retry still-pending bytes. Even if
    // that explicit attempt succeeds, the earlier publication failure survives.
    output.get_mut().fail_write = false;
    let mut diagnostics = Vec::new();
    let status = finish_output(
        output,
        Err(failure.into_legacy()),
        &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
    );
    assert_eq!(status, std::process::ExitCode::from(2));
    assert_eq!(sink.writes, 2);
    assert_eq!(sink.flushes, 1);
    assert!(
        String::from_utf8(diagnostics)
            .unwrap()
            .contains("sink write failed")
    );
    assert_eq!(
        std::str::from_utf8(&sink.bytes)
            .unwrap()
            .matches("Answer: 1")
            .count(),
        1
    );
}
