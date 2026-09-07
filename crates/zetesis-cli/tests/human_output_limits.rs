//! Every human Answer is prepared within one complete-record byte ceiling.

use std::io::{self, Write};

use clap::Parser;
use zetesis_cli::{Options, Report, RunError, RunFailure, run_detailed_with_diagnostics};
use zetesis_cpu::Control;

fn options(maximum: usize) -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--models",
        "0",
    ])
    .unwrap();
    options.max_observation_bytes = maximum;
    options
}

fn solve(source: &str, options: &Options, output: &mut impl Write) -> Result<Report, RunFailure> {
    run_detailed_with_diagnostics(
        source.into(),
        options,
        output,
        &mut io::sink(),
        &Control::default(),
    )
}

fn records() -> [(&'static str, &'static str); 6] {
    [
        ("a.", "Answer: 1\na\n"),
        ("", "Answer: 1\n\n"),
        ("a. #show.", "Answer: 1\n\n"),
        (r#"p("x\ny")."#, "Answer: 1\np(\"x\\ny\")\n"),
        ("p(f(1,(2,))).", "Answer: 1\np(f(1,(2,)))\n"),
        (
            "a. #minimize{-3@1,k:a}.",
            "Answer: 1\na\nOptimization: -3\n",
        ),
    ]
}

#[test]
fn plain_record_ceiling_is_inclusive() {
    for (source, record) in records() {
        for maximum in [record.len(), record.len() + 1] {
            let mut output = Vec::new();
            let report = solve(source, &options(maximum), &mut output).unwrap();
            assert_eq!(report.models, 1, "{source}");
            assert!(output.starts_with(record.as_bytes()), "{source}");
        }
    }
}

#[test]
fn oversized_plain_records_publish_no_prefix() {
    for (source, record) in records() {
        for maximum in [0, record.len() - 1] {
            let mut output = Vec::new();
            let failure = solve(source, &options(maximum), &mut output).unwrap_err();
            assert!(
                matches!(*failure.cause, RunError::ObservationOutputLimit { observed, limit }
                    if limit == maximum && observed > maximum as u128),
                "{source}"
            );
            let progress = failure.partial_report.unwrap();
            assert_eq!(progress.published_models, 0);
            assert_eq!(progress.verified_models, 1);
            assert!(output.is_empty(), "{source}");
        }
    }
}

struct PrefixWriter {
    maximum: usize,
    bytes: Vec<u8>,
}
impl Write for PrefixWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len() == self.maximum {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let count = bytes.len().min(self.maximum - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn partial_plain_records_do_not_count_as_published() {
    for (source, record) in records() {
        for maximum in 0..record.len() {
            let mut output = PrefixWriter {
                maximum,
                bytes: Vec::new(),
            };
            let failure = solve(source, &options(record.len()), &mut output).unwrap_err();
            assert!(matches!(*failure.cause, RunError::Output(_)));
            let progress = failure.partial_report.unwrap();
            assert_eq!(progress.verified_models, 1);
            assert_eq!(progress.published_models, 0);
            assert_eq!(output.bytes, record.as_bytes()[..maximum]);
        }
    }
}
