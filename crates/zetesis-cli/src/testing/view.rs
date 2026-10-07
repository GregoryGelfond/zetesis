use super::{Error, ViewOptions};
use std::io;
use zetesis_presentation::{Alignment, Column, Layout, Row, Table};
use zetesis_validation::{
    backend_check, corpus_comparison,
    performance::{Capture, matrix},
};

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

pub(super) fn scalability(
    report: &matrix::Report,
    options: &ViewOptions,
    layout: Layout,
    output: &mut impl io::Write,
) -> Result<(), Error> {
    if options.json {
        let checks = report
            .samples()
            .iter()
            .map(|sample| scalability_check(sample, options.stats))
            .collect::<Vec<_>>();
        return json(
            &serde_json::json!({
                "schema": 1,
                "format": "zetesis_scalability_conformance",
                "passed": report.passed(),
                "accounted": report.accounted(),
                "workloads": report.workloads(),
                "profiles": report.plan().profiles(),
                "before": report.before(),
                "after": report.after(),
                "faults": report.faults(),
                "unresolved_children": report.unresolved_children(),
                "checks": checks,
                "report": options.report,
            }),
            output,
        );
    }
    let mut columns = vec!["Workload", "Producer", "Outcome", "Detail"];
    if options.stats {
        columns.push("Captured ms");
    }
    let rows = report
        .samples()
        .iter()
        .map(|sample| {
            let slot = sample.slot();
            let label = report.workloads().map_or_else(
                || report.cases()[slot.case].clone(),
                |workloads| workloads[slot.case].label(),
            );
            let producer = match slot.producer {
                matrix::Producer::Reference => "clingo".into(),
                matrix::Producer::Native { profile } => format!(
                    "zetesis / {} threads",
                    report.plan().profiles()[profile].threads
                ),
            };
            let mut cells = vec![
                label,
                producer,
                format!("{:?}", sample.decision()),
                sample.detail().unwrap_or("").to_owned(),
            ];
            if options.stats {
                cells.push(elapsed(
                    sample
                        .capture()
                        .and_then(Capture::elapsed_ns)
                        .map(|ns| ns / 1_000_000),
                ));
            }
            Row::new(cells)
        })
        .collect();
    table(
        "Scalability conformance — complete families",
        &columns,
        rows,
    )?
    .write(output, layout)
    .map_err(Error::Io)?;
    table(
        "Qualification outcome — no measurement rounds",
        &["All passed", "Accounted"],
        vec![Row::new([report.passed().to_string(), report.accounted().to_string()]).conclusion()],
    )?
    .write(output, layout)
    .map_err(Error::Io)
}

fn scalability_check(sample: &matrix::Sample, stats: bool) -> serde_json::Value {
    let mut check = serde_json::json!({
        "slot": sample.slot(),
        "decision": sample.decision(),
        "detail": sample.detail(),
        "blocked_by": sample.blocked_by(),
        "selected_models": sample.selected_models(),
        "cost": sample.cost(),
        "observation": sample.observation(),
        "capture": sample.capture().map(|capture| serde_json::json!({
            "stop": capture.stop(),
            "exit": capture.exit(),
            "failure": capture.failure(),
            "cleanup_failure": capture.cleanup_failure(),
        })),
    });
    if stats {
        check["elapsed_ns"] = serde_json::json!(sample.capture().and_then(Capture::elapsed_ns));
    }
    check
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
