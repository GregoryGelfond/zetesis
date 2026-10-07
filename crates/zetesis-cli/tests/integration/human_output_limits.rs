//! Every human Answer is prepared within one complete-record byte ceiling.

use std::io::{self, Write};

use zetesis_cli::{PublicationConfig, Report, RunError, RunFailure};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::BoundedWriter;

fn options(maximum: usize) -> PublicationConfig {
    let mut config = crate::support::prepared::config(&[]);
    config.observations.max_output_bytes = maximum;
    config
}

fn solve(
    source: &str,
    options: &PublicationConfig,
    output: &mut impl Write,
) -> Result<Report, RunFailure> {
    crate::support::prepared::human(
        source,
        options,
        output,
        &mut io::sink(),
        &Cancellation::default(),
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
            let text = std::str::from_utf8(&output).unwrap();
            let answer = text.find("Answer:").unwrap();
            assert!(text[answer..].starts_with(record), "{source}");
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
            assert!(
                !std::str::from_utf8(&output).unwrap().contains("Answer:"),
                "{source}"
            );
        }
    }
}

#[test]
fn partial_plain_records_do_not_count_as_published() {
    for (source, record) in records() {
        let mut reference = Vec::new();
        solve(source, &options(record.len()), &mut reference).unwrap();
        let preamble = std::str::from_utf8(&reference)
            .unwrap()
            .find("Answer:")
            .unwrap();
        for maximum in 0..record.len() {
            let mut output = BoundedWriter::new(preamble + maximum);
            let failure = solve(source, &options(record.len()), &mut output).unwrap_err();
            assert!(matches!(*failure.cause, RunError::Output(_)));
            let progress = failure.partial_report.unwrap();
            assert_eq!(progress.verified_models, 1);
            assert_eq!(progress.published_models, 0);
            assert_eq!(output.bytes(), &reference[..preamble + maximum]);
        }
    }
}
