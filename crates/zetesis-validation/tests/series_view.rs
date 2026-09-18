//! The series view derives per-cell medians, ratios and counters from reports.
use serde_json::{Value, json};
use zetesis_validation::performance::series::{Labelled, ViewError, compare};

/// A minimal published matrix report with one profile, timed rounds whose
/// native elapsed values are `native[case][round]`, reference values of
/// `reference[case]`, and one optional non-pass decision for a cell. Every
/// native record measures the same stages and phases; every reference
/// record prints the same times.
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
                    "stages": {"grounding": {"calls": 1, "elapsed_ns": 10_000},
                               "solving": {"calls": 1, "elapsed_ns": 60_000}},
                    "phases": {"candidate_setup": {"calls": 1, "elapsed_ns": 200},
                               "candidate_generation": {"calls": 3, "elapsed_ns": 40_000},
                               "closure_membership": {"calls": 3, "elapsed_ns": 3_000},
                               "certified_membership": null}}}
            }));
            samples.push(json!({
                "slot": {"case": case, "phase": "timed", "round": round,
                         "producer": {"solver": "reference"}},
                "decision": "pass",
                "capture": {"elapsed_ns": reference[case],
                            "stdout": {"encoding": "utf8",
                                       "data": "{\"Time\": {\"Total\": 0.004, \"Solve\": 0.001}}"},
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
        "before": [
            {"requested": "/bin/native", "sha256": "ab".repeat(32), "bytes": 1},
            {"requested": "/bin/clingo", "sha256": "cd".repeat(32), "bytes": 1},
            {"requested": "/corpus/manifest.json", "sha256": "ef".repeat(32), "bytes": 1}
        ],
        "started_unix_ns": 1_000_000_000_000_000_000u64,
        "finished_unix_ns": 1_000_000_000_060_000_000u64,
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
    // Each report's native median over the reference solver's median on the
    // same cell: the standing of the build against clingo, per step.
    assert_eq!(chain["profiles"][0]["reference_ratios"]["before"], 0.25);
    assert_eq!(
        chain["profiles"][0]["reference_ratios"]["after"],
        750_000.0 / 8_100_000.0
    );
    assert_eq!(encoded["labels"], json!(["before", "after"]));
    let markdown = comparison.markdown();
    assert!(markdown.contains("| before/reference | after/reference |"));
    assert!(markdown.contains("| 0.250 | 0.093 |"));
}

/// Add passed memory rounds for one producer on the first case, with the
/// given peak resident sets.
fn with_memory(report: &mut Value, producer: &Value, peaks: &[u64]) {
    let samples = report["report"]["samples"].as_array_mut().unwrap();
    for (round, peak) in peaks.iter().enumerate() {
        samples.push(json!({
            "slot": {"case": 0, "phase": "memory", "round": round, "producer": producer},
            "decision": "pass",
            "capture": {"elapsed_ns": 5, "stdout": {"encoding": "utf8", "data": "{}"},
                        "stderr": {"encoding": "utf8", "data": ""}},
            "memory": {"schema": 1, "child": 7, "exit_code": 0, "signal": null,
                       "raw_max_rss": peak / 1024, "raw_unit": "kibibytes",
                       "peak_rss_bytes": peak}
        }));
    }
}

#[test]
fn scoreboards_count_the_cells_the_native_solver_decided_faster() {
    let only = report(
        &[
            "generated/chain-1000.lp",
            "standalone/send-money/send-money.lp",
            "generated/queens-11.lp",
        ],
        &[&[2_000_000], &[500_000], &[1_000_000]],
        &[1_000_000, 1_000_000, 1_000_000],
        None,
    );
    let comparison = compare(&[Labelled {
        label: "only",
        report: &only,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let board = &encoded["scoreboards"][0];
    assert_eq!(board["report"], "only");
    assert_eq!(board["profile"], 0);
    assert_eq!(board["compared"], 3);
    // An equal median is not a win.
    assert_eq!(board["wins"], 1);
    let verdicts = board["verdicts"].as_array().unwrap();
    assert_eq!(verdicts[0]["cell"], "send-money/send-money");
    assert_eq!(verdicts[0]["ratio"], 0.5);
    assert_eq!(verdicts[1]["cell"], "generated/queens-11");
    assert_eq!(verdicts[2]["cell"], "generated/chain-1000");
    assert_eq!(verdicts[2]["ratio"], 2.0);
    let markdown = comparison.markdown();
    assert!(markdown.contains("faster on 1 of 3 cells where both passed (33.3%)."));
    assert!(markdown.contains("| send-money/send-money | 0.500 | 1.000 | 0.500 |"));
    assert!(markdown.contains("| generated/chain-1000 | 2.000 | 1.000 | 2.000 |"));
    // The campaign ran no memory rounds, so no memory table is printed.
    assert!(!markdown.contains("Peak memory"));
}

#[test]
fn a_cell_the_reference_did_not_pass_is_not_compared() {
    let mut only = report(
        &["generated/chain-1000.lp", "generated/queens-11.lp"],
        &[&[500_000], &[500_000]],
        &[1_000_000, 1_000_000],
        None,
    );
    for sample in only["report"]["samples"].as_array_mut().unwrap() {
        if sample["slot"]["case"] == 1 && sample["slot"]["producer"]["solver"] == "reference" {
            sample["decision"] = json!("timeout");
        }
    }
    let comparison = compare(&[Labelled {
        label: "only",
        report: &only,
    }])
    .unwrap();
    assert_eq!(comparison.scoreboards[0].compared, 1);
    assert_eq!(comparison.scoreboards[0].wins, 1);
    assert_eq!(
        comparison.scoreboards[0].verdicts[0].cell,
        "generated/chain-1000"
    );
}

#[test]
fn breakdowns_sum_the_parts_of_each_record() {
    let only = report(&["generated/chain-1000.lp"], &[&[2_000_000]], &[1], None);
    let comparison = compare(&[Labelled {
        label: "only",
        report: &only,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let breakdown = &encoded["cells"][0]["profiles"][0]["reports"]["only"]["breakdown"];
    assert_eq!(breakdown["grounding"], 10_000);
    assert_eq!(breakdown["proposal"], 40_200);
    // An unmeasured phase contributes nothing; the measured one is the sum.
    assert_eq!(breakdown["membership"], 3_000);
    let verdict = &encoded["scoreboards"][0]["verdicts"][0];
    assert_eq!(verdict["native"]["proposal"], 40_200);
}

#[test]
fn reference_times_split_by_its_own_report() {
    let only = report(
        &["generated/chain-1000.lp"],
        &[&[2_000_000]],
        &[4_000_000],
        None,
    );
    let comparison = compare(&[Labelled {
        label: "only",
        report: &only,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let reference = &encoded["cells"][0]["reference"]["only"];
    assert_eq!(reference["median_ns"], 4_000_000);
    assert_eq!(reference["grounding_ns"], 3_000_000);
    assert_eq!(reference["solving_ns"], 1_000_000);
    let verdict = &encoded["scoreboards"][0]["verdicts"][0];
    assert_eq!(verdict["reference_grounding_ns"], 3_000_000);
    assert_eq!(verdict["reference_solving_ns"], 1_000_000);
    assert!(comparison.markdown().contains(
        "| generated/chain-1000 | 2.000 | 4.000 | 0.500 | 0.010 | 0.040 | 0.003 | 3.000 | 1.000 |"
    ));
}

#[test]
fn memory_rounds_report_the_median_peak_resident_set() {
    let mut only = report(&["generated/chain-1000.lp"], &[&[2_000_000]], &[1], None);
    let mib = 1024 * 1024;
    with_memory(
        &mut only,
        &json!({"solver": "native", "profile": 0}),
        &[300 * mib, 100 * mib, 200 * mib],
    );
    with_memory(
        &mut only,
        &json!({"solver": "reference"}),
        &[50 * mib + mib / 2],
    );
    let comparison = compare(&[Labelled {
        label: "only",
        report: &only,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    assert_eq!(
        encoded["cells"][0]["profiles"][0]["reports"]["only"]["peak_rss_bytes"],
        200 * mib
    );
    assert_eq!(
        encoded["cells"][0]["reference"]["only"]["peak_rss_bytes"],
        50 * mib + mib / 2
    );
    let markdown = comparison.markdown();
    assert!(markdown.contains("Peak memory, MiB"));
    assert!(markdown.contains("| generated/chain-1000 | 200.0 | 50.5 | n/a |"));
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
fn closure_route_work_is_read_from_its_summed_receipt() {
    let mut only = report(&["generated/chain-1000.lp"], &[&[2_000_000]], &[1], None);
    // The closure route reports no formula search; its typed receipt sums
    // the work of every completed check instead.
    let stdout = json!({
        "outcome": {"published_models": 1},
        "statistics": {"search": null,
                       "closure_execution": {"grounder": "lazy", "completed_checks": 1,
                                             "stopped_checks": 0, "work": 4321}}
    })
    .to_string();
    for sample in only["report"]["samples"].as_array_mut().unwrap() {
        if sample["slot"]["producer"]["solver"] == "native" {
            sample["capture"]["stdout"]["data"] = json!(stdout);
        }
    }
    let comparison = compare(&[Labelled {
        label: "only",
        report: &only,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let native = &encoded["cells"][0]["profiles"][0]["reports"]["only"];
    assert_eq!(native["search_work"], 4321);
    assert_eq!(native["published_models"], 1);
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
    assert!(encoded["cells"][0]["profiles"][0]["reference_ratios"]["main"].is_null());
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
    assert_eq!(
        encoded["provenance"]["a"]["reference_sha256"],
        "cd".repeat(32)
    );
    assert_eq!(
        encoded["provenance"]["a"]["manifest_sha256"],
        "ef".repeat(32)
    );
    assert_eq!(
        encoded["provenance"]["a"]["started_unix_ns"],
        1_000_000_000_000_000_000u64
    );
    assert_eq!(
        encoded["provenance"]["a"]["finished_unix_ns"],
        1_000_000_000_060_000_000u64
    );
    assert_eq!(encoded["provenance"]["a"]["passed"], true);
}

#[test]
fn cells_are_labelled_by_family_or_by_amended_entry() {
    let mut one = report(
        &[
            "generated/chain-1000.lp",
            "standalone/n-queens/variant-01.lp",
            "standalone/n-queens/variant-01.lp",
            "standalone/send-money/send-money.lp",
        ],
        &[&[1], &[1], &[1], &[1]],
        &[1, 1, 1, 1],
        None,
    );
    one["report"]["workloads"] = json!([
        {"entry": "generated/chain-1000.lp", "generated": {"family": "chain", "size": 1000}},
        {"entry": "standalone/n-queens/variant-01.lp", "amended": true,
         "sources": [{"edits": [{"before": "8", "after": "10"}]}]},
        {"entry": "standalone/n-queens/variant-01.lp", "amended": true,
         "sources": [{"edits": [{"before": "8", "after": "11"}]}]},
        {"entry": "standalone/send-money/send-money.lp", "amended": false, "sources": []}
    ]);
    let comparison = compare(&[Labelled {
        label: "a",
        report: &one,
    }])
    .unwrap();
    let labels: Vec<_> = comparison
        .cells
        .iter()
        .map(|cell| cell.label.as_str())
        .collect();
    assert_eq!(
        labels,
        [
            "chain-1000",
            "n-queens/variant-01 8→10",
            "n-queens/variant-01 8→11",
            "send-money/send-money"
        ]
    );
    let markdown = comparison.markdown();
    assert!(markdown.contains("| n-queens/variant-01 8→11 |"));
}

#[test]
fn blocked_positions_name_their_blocking_decision() {
    let mut one = report(&["generated/stratified-16.lp"], &[&[1, 1]], &[1], None);
    // The qualification position timed out and the timed positions were
    // never launched; the view reports the blocking decision, not only the
    // count of positions it blocked.
    let samples = one["report"]["samples"].as_array_mut().unwrap();
    let blocker = samples
        .iter()
        .position(|s| s["slot"]["phase"] == "qualification")
        .unwrap();
    samples[blocker]["decision"] = json!("timeout");
    for sample in samples.iter_mut() {
        if sample["slot"]["phase"] == "timed" && sample["slot"]["producer"]["solver"] == "native" {
            sample["decision"] = json!("not_attempted");
            sample["blocked_by"] = json!(blocker);
            sample["capture"] = Value::Null;
        }
    }
    let comparison = compare(&[Labelled {
        label: "a",
        report: &one,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    assert_eq!(
        encoded["cells"][0]["profiles"][0]["reports"]["a"]["decisions"],
        json!({"blocked by timeout": 2})
    );
}
