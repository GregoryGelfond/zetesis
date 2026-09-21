//! Human views retain the distinctions in typed workload identities.
use crate::test_writer::BoundedWriter;
use std::{io, num::NonZeroUsize};
use zetesis_presentation::{ColorMode, Layout};
use zetesis_validation::{
    examples,
    performance::{matrix, series},
    selected::NativeExecution,
};

#[path = "benchmark_reports.rs"]
mod reports;

fn layout(color: ColorMode) -> Layout {
    Layout::new(NonZeroUsize::new(1024).unwrap(), color)
}

fn assert_row(output: &str, expected: &str) {
    assert!(
        output
            .lines()
            .any(|line| line.split_whitespace().collect::<Vec<_>>().join(" ") == expected),
        "missing row {expected:?}:\n{output}"
    );
}

fn summary<'a>(cases: &'a [String], profiles: &'a [NativeExecution]) -> matrix::Summary<'a> {
    matrix::Summary {
        schema: 1,
        format: "zetesis_benchmark_summary",
        passed: false,
        accounted: true,
        cases,
        workloads: None,
        profiles,
        reference_policy: matrix::ReferencePolicy::AllPhases,
        before: &[],
        cells: vec![
            matrix::CellSummary {
                case: 0,
                producer: matrix::Producer::Reference,
                decisions: vec![matrix::DecisionCount {
                    decision: matrix::Decision::Pass,
                    positions: 5,
                }],
                timing: Some(series::Timing {
                    samples: 3,
                    minimum_ns: 4_999_999,
                    median_ns: 5_125_999,
                    maximum_ns: 6_250_999,
                }),
                peak_rss_bytes: Some(3_670_016),
            },
            matrix::CellSummary {
                case: 0,
                producer: matrix::Producer::Native { profile: 0 },
                decisions: vec![matrix::DecisionCount {
                    decision: matrix::Decision::Pass,
                    positions: 5,
                }],
                timing: Some(series::Timing {
                    samples: 3,
                    minimum_ns: 0,
                    median_ns: 0,
                    maximum_ns: 0,
                }),
                peak_rss_bytes: Some(0),
            },
            matrix::CellSummary {
                case: 0,
                producer: matrix::Producer::Native { profile: 1 },
                decisions: vec![
                    matrix::DecisionCount {
                        decision: matrix::Decision::Pass,
                        positions: 1,
                    },
                    matrix::DecisionCount {
                        decision: matrix::Decision::Timeout,
                        positions: 1,
                    },
                    matrix::DecisionCount {
                        decision: matrix::Decision::NotAttempted,
                        positions: 3,
                    },
                ],
                timing: None,
                peak_rss_bytes: None,
            },
        ],
    }
}

fn comparison() -> series::Comparison {
    let [before, after] = reports::reports();
    series::compare(&[
        series::Labelled {
            label: "before",
            report: &before,
        },
        series::Labelled {
            label: "after",
            report: &after,
        },
    ])
    .unwrap()
}

#[test]
fn summary_distinguishes_missing_and_zero_measurements() {
    let cases = [reports::CASES[0].to_owned()];
    let profiles = reports::profiles();
    let summary = summary(&cases, &profiles);
    let mut output = Vec::new();
    super::corpus(&summary, false, layout(ColorMode::Auto), &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains('\u{1b}'));
    assert_row(
        &text,
        "Profile Device Grounder Oracle Threads Completion Batch",
    );
    assert_row(&text, "1 Cpu Auto Auto 1 1 64");
    assert_row(&text, "2 Cpu Auto Auto 4 1 64");
    assert_row(
        &text,
        "Workload Producer Median ms Range ms RSS MiB All positions",
    );
    assert_row(
        &text,
        "1: generated/choice-2.lp clingo 5.125 4.999–6.250 3.500 Pass: 5",
    );
    assert_row(
        &text,
        "1: generated/choice-2.lp zetesis profile 1 0.000 0.000–0.000 0.000 Pass: 5",
    );
    assert_row(
        &text,
        "1: generated/choice-2.lp zetesis profile 2 — — — Pass: 1, Timeout: 1, NotAttempted: 3",
    );
    assert_row(&text, "All passed Accounted");
    assert_row(&text, "false true");
}

#[test]
fn summary_json_preserves_raw_units_without_styling() {
    let cases = [reports::CASES[0].to_owned()];
    let profiles = reports::profiles();
    let mut output = Vec::new();
    super::corpus(
        &summary(&cases, &profiles),
        true,
        layout(ColorMode::Always),
        &mut output,
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["format"], "zetesis_benchmark_summary");
    assert_eq!(value["cells"][0]["timing"]["median_ns"], 5_125_999);
    assert_eq!(value["cells"][0]["peak_rss_bytes"], 3_670_016);
    assert_eq!(value["cells"][1]["peak_rss_bytes"], 0);
    assert!(value["cells"][2]["timing"].is_null());
    assert!(!output.contains(&0x1b));
}

#[test]
fn qualification_only_reference_has_no_measurements_in_either_view() {
    let cases = [reports::CASES[0].to_owned()];
    let profiles = reports::profiles();
    let mut summary = summary(&cases, &profiles);
    summary.reference_policy = matrix::ReferencePolicy::QualificationOnly;
    let reference = &mut summary.cells[0];
    reference.decisions[0].positions = 1;
    reference.timing = None;
    reference.peak_rss_bytes = None;
    let mut human = Vec::new();
    super::corpus(&summary, false, layout(ColorMode::Never), &mut human).unwrap();
    assert_row(
        &String::from_utf8(human).unwrap(),
        "1: generated/choice-2.lp clingo — — — Pass: 1",
    );
    let mut machine = Vec::new();
    super::corpus(&summary, true, layout(ColorMode::Never), &mut machine).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&machine).unwrap();
    assert_eq!(value["reference_policy"], "qualification_only");
    assert_eq!(value["cells"][0]["decisions"][0]["positions"], 1);
    assert!(value["cells"][0]["timing"].is_null());
    assert!(value["cells"][0]["peak_rss_bytes"].is_null());
}

#[test]
fn comparison_keeps_each_report_and_profile_separate() {
    let comparison = comparison();
    let mut output = Vec::new();
    super::comparison(&comparison, false, layout(ColorMode::Never), &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert_row(
        &text,
        "Report Workload Profile zetesis ms clingo ms zetesis MiB clingo MiB Outcome",
    );
    assert_row(
        &text,
        "before generated/choice-2 1: cpu/auto (1 threads) 2.000 5.000 3.500 5.000 pass",
    );
    assert_row(
        &text,
        "after generated/choice-2 1: cpu/auto (1 threads) 20.000 8.000 — — pass",
    );
    assert_row(
        &text,
        "before generated/choice-2 2: cpu/auto (4 threads) 3.000 5.000 — 5.000 pass",
    );
    assert_row(
        &text,
        "after generated/choice-2 2: cpu/auto (4 threads) 21.000 8.000 — — pass",
    );
    let position = |label| {
        text.lines()
            .position(|line| {
                line.split_whitespace()
                    .take(2)
                    .eq([label, "generated/choice-2"])
            })
            .unwrap()
    };
    assert!(position("before") < position("after"));
    assert_row(
        &text,
        &format!("before false true {} {}", "11".repeat(32), "33".repeat(32)),
    );
    assert_row(
        &text,
        &format!("after true true {} {}", "22".repeat(32), "33".repeat(32)),
    );
}

#[test]
fn comparison_does_not_time_failed_positions() {
    let comparison = comparison();
    let mut output = Vec::new();
    super::comparison(&comparison, false, layout(ColorMode::Never), &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert_row(
        &text,
        "before generated/cycle-2 1: cpu/auto (1 threads) — 15.000 — — blocked by timeout: 2, timeout: 1",
    );
    assert_row(
        &text,
        "after generated/cycle-2 1: cpu/auto (1 threads) 30.000 18.000 — — pass",
    );
    assert!(
        !text.contains("9999.999"),
        "a timeout is not a measured completion"
    );
}

#[test]
fn missing_comparison_observations_stay_unavailable() {
    let mut comparison = comparison();
    comparison.cells[0].profiles[0].reports.remove("before");
    comparison.cells[0].reference.remove("before");
    let mut output = Vec::new();
    super::comparison(&comparison, false, layout(ColorMode::Never), &mut output).unwrap();
    assert_row(
        &String::from_utf8(output).unwrap(),
        "before generated/choice-2 1: cpu/auto (1 threads) — — — — unavailable",
    );
}

#[test]
fn comparison_styles_only_the_human_view() {
    let comparison = comparison();
    let mut human = Vec::new();
    super::comparison(&comparison, false, layout(ColorMode::Always), &mut human).unwrap();
    let text = String::from_utf8(human).unwrap();
    assert!(text.contains("\u{1b}[34mCorpus benchmark — timed medians\u{1b}[0m"));
    assert!(text.contains("\u{1b}[3;90m2.000\u{1b}[0m"));
    assert!(text.contains("\u{1b}[1;3;90mfalse\u{1b}[0m"));
    let mut machine = Vec::new();
    super::comparison(&comparison, true, layout(ColorMode::Always), &mut machine).unwrap();
    assert!(!machine.contains(&0x1b));
    let decoded: serde_json::Value = serde_json::from_slice(&machine).unwrap();
    assert_eq!(decoded, serde_json::to_value(&comparison).unwrap());
}

#[test]
fn summary_preserves_a_writer_failure_after_rows() {
    let cases = [reports::CASES[0].to_owned()];
    let profiles = reports::profiles();
    let summary = summary(&cases, &profiles);
    let mut complete = Vec::new();
    super::corpus(&summary, false, layout(ColorMode::Never), &mut complete).unwrap();
    let limit = std::str::from_utf8(&complete)
        .unwrap()
        .find("Campaign outcome")
        .unwrap();
    let mut output = BoundedWriter::new(limit);
    let error = super::corpus(&summary, false, layout(ColorMode::Never), &mut output).unwrap_err();
    assert!(
        matches!(error, super::Error::Io(ref error) if error.kind() == io::ErrorKind::BrokenPipe)
    );
    assert_eq!(output.bytes(), &complete[..limit]);
}

#[test]
fn comparison_preserves_a_writer_failure_after_rows() {
    let comparison = comparison();
    let mut complete = Vec::new();
    super::comparison(&comparison, false, layout(ColorMode::Never), &mut complete).unwrap();
    let limit = std::str::from_utf8(&complete)
        .unwrap()
        .find("Recorded campaign identity")
        .unwrap();
    let mut output = BoundedWriter::new(limit);
    let error =
        super::comparison(&comparison, false, layout(ColorMode::Never), &mut output).unwrap_err();
    assert!(
        matches!(error, super::Error::Io(ref error) if error.kind() == io::ErrorKind::BrokenPipe)
    );
    assert_eq!(output.bytes(), &complete[..limit]);
}

#[test]
fn amended_workloads_have_distinct_human_labels() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kr-domains");
    let corpus = examples::load(&root, examples::Limits::default()).unwrap();
    let entry = "standalone/n-queens/variant-01.lp";
    let workloads: Vec<_> = [10, 11]
        .into_iter()
        .map(|replacement| {
            matrix::Workload::amended(
                &corpus,
                entry,
                &[matrix::ConstantAmendment {
                    source_path: entry,
                    name: "n",
                    expected: 8,
                    replacement,
                }],
                matrix::WorkloadLimits::default(),
            )
            .unwrap()
        })
        .collect();
    let cases = vec![entry.to_owned(); 2];
    let summary = matrix::Summary {
        schema: 1,
        format: "zetesis_benchmark_summary",
        passed: false,
        accounted: true,
        cases: &cases,
        workloads: Some(&workloads),
        profiles: &[],
        reference_policy: matrix::ReferencePolicy::AllPhases,
        before: &[],
        cells: (0..2)
            .map(|case| matrix::CellSummary {
                case,
                producer: matrix::Producer::Reference,
                decisions: vec![matrix::DecisionCount {
                    decision: matrix::Decision::NotAttempted,
                    positions: 1,
                }],
                timing: None,
                peak_rss_bytes: None,
            })
            .collect(),
    };
    let mut output = Vec::new();
    super::corpus(
        &summary,
        false,
        zetesis_presentation::Layout::default(),
        &mut output,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("1: n-queens/variant-01 8→10"), "{text}");
    assert!(text.contains("2: n-queens/variant-01 8→11"), "{text}");
}
