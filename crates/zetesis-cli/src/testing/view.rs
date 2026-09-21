use super::{Error, ViewOptions};
use std::io;
use zetesis_presentation::{Alignment, Column, Layout, Row, Table};
use zetesis_validation::{backend_check, corpus_comparison};

pub(super) fn json(value: &serde_json::Value, output: &mut impl io::Write) -> Result<(), Error> {
    serde_json::to_writer_pretty(&mut *output, value).map_err(Error::Json)?;
    writeln!(output).map_err(Error::Io)
}

pub(super) fn corpus(
    report: &corpus_comparison::Report,
    options: &ViewOptions,
    layout: Layout,
    output: &mut impl io::Write,
) -> Result<(), Error> {
    if options.json {
        return json(&report.to_json().map_err(Error::Json)?, output);
    }
    let rows = report
        .cases()
        .iter()
        .map(|case| {
            Row::new([
                case.path().to_owned(),
                case.decision().label().to_owned(),
                case.native_answers().map_or_else(
                    || "unavailable".into(),
                    |answer| answer.model_count().to_string(),
                ),
                case.decision().detail().unwrap_or("").to_owned(),
            ])
        })
        .collect();
    table(
        "Corpus conformance — selected displays and costs",
        &["Source", "Outcome", "Native models", "Detail"],
        rows,
    )?
    .write(output, layout)
    .map_err(Error::Io)?;
    if options.stats {
        let rows = report
            .cases()
            .iter()
            .map(|case| {
                Row::new([
                    case.path().to_owned(),
                    elapsed(case.native_elapsed_ms()),
                    elapsed(case.reference_elapsed_ms()),
                ])
            })
            .collect();
        table(
            "Captured process elapsed time — not a benchmark",
            &["Source", "zetesis ms", "clingo ms"],
            rows,
        )?
        .write(output, layout)
        .map_err(Error::Io)?;
    }
    table(
        "Corpus result",
        &["Cases recorded", "Cases required", "All passed"],
        vec![
            Row::new([
                report.cases().len().to_string(),
                report.required_cases().to_string(),
                report.passed().to_string(),
            ])
            .conclusion(),
        ],
    )?
    .write(output, layout)
    .map_err(Error::Io)
}

pub(super) fn backend(
    report: &backend_check::Report,
    options: &ViewOptions,
    layout: Layout,
    output: &mut impl io::Write,
) -> Result<(), Error> {
    if options.json {
        return backend_json(report, output);
    }
    let rows = report
        .cases()
        .iter()
        .map(|case| {
            let attempt = case.selected().unwrap_or_else(|| case.reference());
            let observed = attempt.observation();
            Row::new([
                case.name().to_owned(),
                case.decision().label().to_owned(),
                observed.map_or_else(
                    || "unavailable".into(),
                    |value| {
                        format!(
                            "{:?} / {:?}",
                            value.execution.backend, value.execution.procedure
                        )
                    },
                ),
                attempt.detail().unwrap_or("").to_owned(),
            ])
        })
        .collect();
    table(
        "Small backend conformance checks — mandatory route evidence",
        &["Fixture", "Outcome", "Observed route", "Detail"],
        rows,
    )?
    .write(output, layout)
    .map_err(Error::Io)?;
    if options.stats {
        let rows = report
            .cases()
            .iter()
            .map(|case| {
                Row::new([
                    case.name().to_owned(),
                    elapsed(
                        case.selected()
                            .and_then(backend_check::Attempt::capture)
                            .map(|capture| capture.elapsed().as_millis()),
                    ),
                ])
            })
            .collect();
        table(
            "Captured process elapsed time — not a benchmark",
            &["Fixture", "Selected ms"],
            rows,
        )?
        .write(output, layout)
        .map_err(Error::Io)?;
    }
    table(
        "Backend result — not full physical qualification",
        &["Cases recorded", "Cases required", "All passed"],
        vec![
            Row::new([
                report.cases().len().to_string(),
                report.required_cases().to_string(),
                report.passed().to_string(),
            ])
            .conclusion(),
        ],
    )?
    .write(output, layout)
    .map_err(Error::Io)
}

pub(super) fn backend_json(
    report: &backend_check::Report,
    output: &mut impl io::Write,
) -> Result<(), Error> {
    serde_json::to_writer_pretty(&mut *output, &report.json()).map_err(Error::Json)?;
    writeln!(output).map_err(Error::Io)
}

fn elapsed(value: Option<u128>) -> String {
    value.map_or_else(|| "unavailable".into(), |value| value.to_string())
}

fn table(title: &str, columns: &[&str], rows: Vec<Row>) -> Result<Table, Error> {
    Table::new(
        title,
        columns
            .iter()
            .map(|name| {
                Column::new(
                    name,
                    if name.ends_with(" ms") {
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
