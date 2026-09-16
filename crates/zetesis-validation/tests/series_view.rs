//! The series view derives per-cell medians, ratios and counters from reports.
use serde_json::{Value, json};
use zetesis_validation::performance::series::{Labelled, ViewError, compare};

/// A minimal published matrix report with one profile, timed rounds whose
/// native elapsed values are `native[case][round]`, reference values of
/// `reference[case]`, and one optional non-pass decision for a cell.
fn report(cases: &[&str], native: &[&[u64]], reference: &[u64], refused: Option<usize>) -> Value {
    let mut samples = Vec::new();
    for (case, rounds) in native.iter().enumerate() {
        for (round, elapsed) in rounds.iter().enumerate() {
            let decision = if refused == Some(case) {
                "refused"
            } else {
                "pass"
            };
            let stdout = json!({
                "outcome": {"published_models": 92},
                "statistics": {"search": {"work": 1000 + round, "candidates": 92}}
            })
            .to_string();
            let stderr = format!(
                "  results: displayed models=92; candidates examined={}\n",
                92 + round
            );
            samples.push(json!({
                "slot": {"case": case, "phase": "timed", "round": round,
                         "producer": {"solver": "native", "profile": 0}},
                "decision": decision,
                "capture": {"elapsed_ns": elapsed,
                            "stdout": {"encoding": "utf8", "data": stdout},
                            "stderr": {"encoding": "utf8", "data": stderr}},
                "observation": {"timing": {"driver_elapsed_ns": elapsed.saturating_sub(100_000),
                    "phases": {"candidate_generation": {"calls": 3, "elapsed_ns": 40_000}}}}
            }));
            samples.push(json!({
                "slot": {"case": case, "phase": "timed", "round": round,
                         "producer": {"solver": "reference"}},
                "decision": "pass",
                "capture": {"elapsed_ns": reference[case],
                            "stdout": {"encoding": "utf8", "data": "{}"},
                            "stderr": {"encoding": "utf8", "data": ""}}
            }));
        }
        samples.push(json!({
            "slot": {"case": case, "phase": "qualification", "round": 0,
                     "producer": {"solver": "native", "profile": 0}},
            "decision": "pass",
            "capture": {"elapsed_ns": 1, "stdout": {"encoding": "utf8", "data": "{}"},
                        "stderr": {"encoding": "utf8", "data": ""}}
        }));
    }
    json!({"passed": refused.is_none(), "accounted": true, "report": {
        "cases": cases,
        "plan": {"profiles": [{"backend": "cpu", "oracle": "auto", "grounder": "auto",
                               "workers": 4, "completion_workers": 4, "batch_size": 64,
                               "max_completion_scratch_bytes": 268_435_456}]},
        "before": [{"requested": "/bin/native", "sha256": "ab".repeat(32), "bytes": 1}],
        "samples": samples
    }})
}

#[test]
fn medians_are_taken_per_cell_profile_and_report() {
    let before = report(
        &[
            "generated/chain-1000.lp",
            "standalone/send-money/send-money.lp",
        ],
        &[
            &[3_000_000, 1_000_000, 2_000_000],
            &[50_000_000, 48_000_000, 49_000_000],
        ],
        &[8_000_000, 15_000_000],
        None,
    );
    let after = report(
        &[
            "generated/chain-1000.lp",
            "standalone/send-money/send-money.lp",
        ],
        &[
            &[1_000_000, 500_000, 750_000],
            &[49_000_000, 49_000_000, 49_000_000],
        ],
        &[8_100_000, 15_100_000],
        None,
    );
    let comparison = compare(&[
        Labelled {
            label: "before",
            report: &before,
        },
        Labelled {
            label: "after",
            report: &after,
        },
    ])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let chain = &encoded["cells"][0];
    assert_eq!(chain["entry"], "generated/chain-1000.lp");
    let native = &chain["profiles"][0]["reports"];
    assert_eq!(native["before"]["median_ns"], 2_000_000);
    assert_eq!(native["before"]["minimum_ns"], 1_000_000);
    assert_eq!(native["before"]["maximum_ns"], 3_000_000);
    assert_eq!(native["before"]["samples"], 3);
    assert_eq!(native["after"]["median_ns"], 750_000);
    assert_eq!(chain["reference"]["after"]["median_ns"], 8_100_000);
    // Ratios are of medians, later report over earlier, in label order.
    assert_eq!(chain["profiles"][0]["ratios"]["after/before"], 0.375);
    assert_eq!(encoded["labels"], json!(["before", "after"]));
}

#[test]
fn counters_and_driver_time_come_from_the_retained_records() {
    let only = report(&["generated/chain-1000.lp"], &[&[2_000_000]], &[1], None);
    let comparison = compare(&[Labelled {
        label: "only",
        report: &only,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let native = &encoded["cells"][0]["profiles"][0]["reports"]["only"];
    assert_eq!(native["published_models"], 92);
    assert_eq!(native["candidates_examined"], 92);
    assert_eq!(native["search_work"], 1000);
    assert_eq!(native["driver_median_ns"], 1_900_000);
    assert_eq!(
        native["phases"]["candidate_generation"]["median_ns"],
        40_000
    );
}

#[test]
fn non_pass_cells_are_reported_by_decision_not_averaged() {
    let refused = report(
        &["generated/producer-chain-700.lp"],
        &[&[5_000, 6_000]],
        &[1],
        Some(0),
    );
    let comparison = compare(&[Labelled {
        label: "main",
        report: &refused,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let cell = &encoded["cells"][0]["profiles"][0]["reports"]["main"];
    assert!(cell.get("median_ns").is_none());
    assert_eq!(cell["decisions"], json!({"refused": 2}));
    let markdown = comparison.markdown();
    assert!(markdown.contains("producer-chain-700"));
    assert!(markdown.contains("refused"));
}

#[test]
fn reports_must_share_their_cells_and_profiles() {
    let one = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    let other = report(&["generated/chain-2000.lp"], &[&[1]], &[1], None);
    assert!(matches!(
        compare(&[
            Labelled {
                label: "a",
                report: &one
            },
            Labelled {
                label: "b",
                report: &other
            }
        ]),
        Err(ViewError::Cells { .. })
    ));
    assert!(matches!(compare(&[]), Err(ViewError::Empty)));
    let duplicate = compare(&[
        Labelled {
            label: "a",
            report: &one,
        },
        Labelled {
            label: "a",
            report: &one,
        },
    ]);
    assert!(matches!(duplicate, Err(ViewError::Label { .. })));
}

#[test]
fn provenance_names_each_report_native_seal() {
    let one = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    let comparison = compare(&[Labelled {
        label: "a",
        report: &one,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    assert_eq!(encoded["provenance"]["a"]["native_sha256"], "ab".repeat(32));
    assert_eq!(encoded["provenance"]["a"]["passed"], true);
}
