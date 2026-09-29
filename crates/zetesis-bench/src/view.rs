use super::Error;
use std::io;
use zetesis_presentation::{Alignment, Column, Layout, Row, Table};
use zetesis_validation::performance::{
    matrix::{Producer, Qualification, Summary},
    series::{Comparison, Native, Scoreboard},
};

#[cfg(test)]
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
                // A clingo-free campaign sealed no clingo.
                provenance
                    .reference_sha256
                    .clone()
                    .unwrap_or_else(|| "not run".into()),
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
    .map_err(Error::Io)?;
    for scoreboard in &comparison.scoreboards {
        scoreboard_tables(comparison, scoreboard, layout, output)?;
    }
    Ok(())
}

/// One report's standing against clingo on one profile: every cell where both
/// passed, fastest ratio first, and their peak memory when it was measured.
fn scoreboard_tables(
    comparison: &Comparison,
    scoreboard: &Scoreboard,
    layout: Layout,
    output: &mut impl io::Write,
) -> Result<(), Error> {
    let profile = comparison
        .cells
        .first()
        .map(|cell| profile_name(&cell.profiles[scoreboard.profile].profile))
        .unwrap_or_default();
    let standing = if scoreboard.compared == 0 {
        "no cell where both passed".to_owned()
    } else {
        format!(
            "faster on {} of {} cells where both passed",
            scoreboard.wins, scoreboard.compared
        )
    };
    let rows = scoreboard
        .verdicts
        .iter()
        .map(|verdict| {
            Row::new([
                verdict.cell.clone(),
                milliseconds(verdict.native_ns),
                milliseconds(verdict.reference_ns),
                ratio(verdict.native_ns, verdict.reference_ns),
                optional_milliseconds(verdict.native.grounding),
                optional_milliseconds(verdict.native.proposal),
                optional_milliseconds(verdict.native.membership),
                optional_milliseconds(verdict.reference_grounding_ns),
                optional_milliseconds(verdict.reference_solving_ns),
            ])
        })
        .collect();
    table(
        &format!(
            "Against clingo — report {}, profile {}: {profile}, search {}; {standing}",
            scoreboard.report,
            scoreboard.profile + 1,
            scoreboard.method
        ),
        &[
            "Cell",
            "zetesis ms",
            "clingo ms",
            "zetesis/clingo",
            "grounding ms",
            "proposal ms",
            "membership ms",
            "clingo grounding ms",
            "clingo solving ms",
        ],
        rows,
    )?
    .write(output, layout)
    .map_err(Error::Io)?;
    if !scoreboard.verdicts.iter().any(|verdict| {
        verdict.native_peak_rss_bytes.is_some()
            || verdict.reference_peak_rss_bytes.is_some()
            || verdict.device_bytes.is_some()
    }) {
        return Ok(());
    }
    let rows = scoreboard
        .verdicts
        .iter()
        .map(|verdict| {
            Row::new([
                verdict.cell.clone(),
                bytes(verdict.native_peak_rss_bytes),
                bytes(verdict.reference_peak_rss_bytes),
                bytes(verdict.device_bytes),
            ])
        })
        .collect();
    table(
        &format!(
            "Peak memory against clingo — report {}, profile {}",
            scoreboard.report,
            scoreboard.profile + 1
        ),
        &["Cell", "zetesis MiB", "clingo MiB", "device MiB"],
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
                        // Clingo's columns say "not run" where the campaign ran
                        // without it, apart from a missing or failed value's dash.
                        let clingo_ran = comparison
                            .provenance
                            .get(label)
                            .is_some_and(|provenance| provenance.reference_sha256.is_some());
                        let (clingo_ms, clingo_memory) = match cell.reference.get(label) {
                            Some(record) => (
                                milliseconds(record.timing.median_ns),
                                bytes(record.peak_rss_bytes),
                            ),
                            None if clingo_ran => ("—".into(), "—".into()),
                            None => ("not run".into(), "not run".into()),
                        };
                        Row::new([
                            label.clone(),
                            cell.label.clone(),
                            format!("{}: {}", profile_index + 1, profile_name(&profile.profile)),
                            native,
                            clingo_ms,
                            memory,
                            clingo_memory,
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
                    if label.ends_with(" ms")
                        || label.ends_with(" MiB")
                        || *label == "Checked"
                        || *label == "zetesis/clingo"
                    {
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
fn optional_milliseconds(nanos: Option<u64>) -> String {
    nanos.map_or_else(|| "—".into(), milliseconds)
}
/// `native / reference` to three decimals, rounded half up, by exact integer
/// arithmetic; a reference of zero reads as one nanosecond.
fn ratio(native: u64, reference: u64) -> String {
    let reference = u128::from(reference.max(1));
    let thousandths = (u128::from(native) * 1000 + reference / 2) / reference;
    format!("{}.{:03}", thousandths / 1000, thousandths % 1000)
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

pub(super) fn run(
    summary: &Summary<'_>,
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
    let rows = run_rows(summary);
    table(
        "Corpus benchmark — successful timed populations",
        &[
            "Workload",
            "Producer",
            "Qualified by",
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

fn run_rows(summary: &Summary<'_>) -> Vec<Row> {
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
                qualification(cell.qualification).to_owned(),
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

/// What qualified a cell's answer family, as the tables name it.
const fn qualification(qualification: Qualification) -> &'static str {
    match qualification {
        Qualification::Clingo => "clingo",
        Qualification::Contract => "recorded contract",
        Qualification::NeedsClingo => "needs clingo",
    }
}
