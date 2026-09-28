use super::Error;
use std::io;
use zetesis_presentation::{Alignment, Column, Layout, Row, Table};
use zetesis_validation::performance::series::{Comparison, Native};

#[cfg(test)]
#[path = "../tests/support/benchmark_views.rs"]
mod tests;

pub(super) fn comparison(
    comparison: &Comparison,
    json: bool,
    layout: Layout,
    output: &mut impl io::Write,
) -> Result<(), Error> {
    if json {
        serde_json::to_writer_pretty(&mut *output, comparison).map_err(Error::Json)?;
        return writeln!(output).map_err(Error::Io);
    }
    let rows = comparison_rows(comparison);
    table(
        "Corpus benchmark — timed medians",
        &[
            "Report",
            "Workload",
            "Profile",
            "zetesis ms",
            "clingo ms",
            "zetesis MiB",
            "clingo MiB",
            "Outcome",
            "Detail",
        ],
        rows,
    )?
    .write(output, layout)
    .map_err(Error::Io)?;
    comparison_failures(comparison, layout, output)?;
    let rows = comparison
        .labels
        .iter()
        .map(|label| {
            let provenance = &comparison.provenance[label];
            Row::new([
                label.clone(),
                provenance.passed.to_string(),
                provenance.accounted.to_string(),
                provenance.native_sha256.clone(),
                provenance.reference_sha256.clone(),
            ])
            .conclusion()
        })
        .collect();
    table(
        "Recorded campaign identity",
        &[
            "Report",
            "All passed",
            "Accounted",
            "zetesis SHA-256",
            "clingo SHA-256",
        ],
        rows,
    )?
    .write(output, layout)
    .map_err(Error::Io)
}

fn comparison_rows(comparison: &Comparison) -> Vec<Row> {
    comparison
        .cells
        .iter()
        .flat_map(|cell| {
            cell.profiles
                .iter()
                .enumerate()
                .flat_map(move |(profile_index, profile)| {
                    comparison.labels.iter().map(move |label| {
                        let (native, memory, outcome, detail) = match profile.reports.get(label) {
                            Some(Native::Passed(record)) => (
                                milliseconds(record.timing.median_ns),
                                bytes(record.peak_rss_bytes),
                                "pass".to_owned(),
                                String::new(),
                            ),
                            Some(Native::NotPassed { decisions, reasons }) => (
                                "—".into(),
                                "—".into(),
                                decisions
                                    .iter()
                                    .map(|(decision, count)| format!("{decision}: {count}"))
                                    .collect::<Vec<_>>()
                                    .join(", "),
                                reasons
                                    .iter()
                                    .map(|(reason, count)| format!("{reason} ×{count}"))
                                    .collect::<Vec<_>>()
                                    .join("; "),
                            ),
                            None => ("—".into(), "—".into(), "unavailable".into(), String::new()),
                        };
                        let reference = cell.reference.get(label);
                        Row::new([
                            label.clone(),
                            cell.label.clone(),
                            format!("{}: {}", profile_index + 1, profile_name(&profile.profile)),
                            native,
                            reference.map_or_else(
                                || "—".into(),
                                |record| milliseconds(record.timing.median_ns),
                            ),
                            memory,
                            bytes(reference.and_then(|record| record.peak_rss_bytes)),
                            outcome,
                            detail,
                        ])
                    })
                })
        })
        .collect()
}

fn comparison_failures(
    comparison: &Comparison,
    layout: Layout,
    output: &mut impl io::Write,
) -> Result<(), Error> {
    let mut rows = Vec::new();
    for cell in &comparison.cells {
        for label in &comparison.labels {
            if let Some(reasons) = cell.failure_reasons.get(label) {
                for (reason, count) in reasons {
                    rows.push(Row::new([
                        label.clone(),
                        cell.label.clone(),
                        reason.clone(),
                        count.to_string(),
                    ]));
                }
            }
        }
    }
    if rows.is_empty() {
        return Ok(());
    }
    table(
        "Recorded failure reasons — all scheduled phases",
        &[
            "Report",
            "Workload",
            "Producer, phase and reason",
            "Positions",
        ],
        rows,
    )?
    .write(output, layout)
    .map_err(Error::Io)
}

pub(super) fn table(title: &str, columns: &[&str], rows: Vec<Row>) -> Result<Table, Error> {
    Table::new(
        title,
        columns
            .iter()
            .map(|label| {
                Column::new(
                    label,
                    if label.ends_with(" ms") || label.ends_with(" MiB") || *label == "Checked" {
                        Alignment::Right
                    } else {
                        Alignment::Left
                    },
                )
            })
            .collect(),
        rows,
    )
    .map_err(Error::Table)
}

fn profile_name(profile: &serde_json::Value) -> String {
    format!(
        "{}/{} ({} threads)",
        profile["backend"].as_str().unwrap_or("unavailable"),
        profile["grounder"].as_str().unwrap_or("unavailable"),
        profile["workers"]
            .as_u64()
            .map_or_else(|| "?".into(), |workers| workers.to_string())
    )
}
fn milliseconds(nanos: u64) -> String {
    format!("{}.{:03}", nanos / 1_000_000, nanos % 1_000_000 / 1000)
}
fn bytes(bytes: Option<u64>) -> String {
    bytes.map_or_else(
        || "—".into(),
        |value| {
            format!(
                "{}.{:03}",
                value / 1_048_576,
                value % 1_048_576 * 1000 / 1_048_576
            )
        },
    )
}

pub(super) fn corpus(
    summary: &zetesis_validation::performance::matrix::Summary<'_>,
    json: bool,
    layout: Layout,
    output: &mut impl io::Write,
) -> Result<(), Error> {
    if json {
        serde_json::to_writer_pretty(&mut *output, summary).map_err(Error::Json)?;
        return writeln!(output).map_err(Error::Io);
    }
    let profiles = summary
        .profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            Row::new([
                (index + 1).to_string(),
                format!("{:?}", profile.backend),
                format!("{:?}", profile.grounder),
                format!("{:?}", profile.oracle),
                profile.workers.to_string(),
                profile.completion_workers.to_string(),
                profile.batch_size.to_string(),
            ])
        })
        .collect();
    table(
        "Requested native profiles",
        &[
            "Profile",
            "Device",
            "Grounder",
            "Oracle",
            "Threads",
            "Completion",
            "Batch",
        ],
        profiles,
    )?
    .write(output, layout)
    .map_err(Error::Io)?;
    let rows = corpus_rows(summary);
    table(
        "Corpus benchmark — successful timed populations",
        &[
            "Workload",
            "Producer",
            "Median ms",
            "Range ms",
            "RSS MiB",
            "All positions",
            "Detail",
        ],
        rows,
    )?
    .write(output, layout)
    .map_err(Error::Io)?;
    table(
        "Campaign outcome",
        &["All passed", "Accounted"],
        vec![Row::new([summary.passed.to_string(), summary.accounted.to_string()]).conclusion()],
    )?
    .write(output, layout)
    .map_err(Error::Io)
}

fn corpus_rows(summary: &zetesis_validation::performance::matrix::Summary<'_>) -> Vec<Row> {
    use zetesis_validation::performance::matrix::Producer;
    summary
        .cells
        .iter()
        .map(|cell| {
            let producer = match cell.producer {
                Producer::Reference => "clingo".to_owned(),
                Producer::Native { profile } => format!("zetesis profile {}", profile + 1),
            };
            let decisions = cell
                .decisions
                .iter()
                .map(|count| format!("{:?}: {}", count.decision, count.positions))
                .collect::<Vec<_>>()
                .join(", ");
            Row::new([
                format!(
                    "{}: {}",
                    cell.case + 1,
                    summary.workloads.map_or_else(
                        || summary.cases[cell.case].clone(),
                        |workloads| workloads[cell.case].label(),
                    )
                ),
                producer,
                cell.timing
                    .as_ref()
                    .map_or_else(|| "—".into(), |timing| milliseconds(timing.median_ns)),
                cell.timing.as_ref().map_or_else(
                    || "—".into(),
                    |timing| {
                        format!(
                            "{}–{}",
                            milliseconds(timing.minimum_ns),
                            milliseconds(timing.maximum_ns)
                        )
                    },
                ),
                bytes(cell.peak_rss_bytes),
                decisions,
                cell.reasons
                    .iter()
                    .map(|(reason, count)| format!("{reason} ×{count}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            ])
        })
        .collect()
}
