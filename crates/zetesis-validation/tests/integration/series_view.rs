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
    // The JSON publishes the exact medians and no ratio of them.
    assert!(chain["profiles"][0].get("ratios").is_none());
    assert!(chain["profiles"][0].get("reference_ratios").is_none());
    assert_eq!(encoded["labels"], json!(["before", "after"]));
    // The tables divide them: the later report over the earlier, in label
    // order, then each report's native median over the reference solver's on
    // the same cell, the standing of the build against clingo, per step.
    let markdown = comparison.markdown();
    assert!(markdown.contains("| after/before | before/reference | after/reference |"));
    assert!(markdown.contains("| 0.375 | 0.250 | 0.093 |"));
}

#[test]
fn three_reports_divide_in_order_and_last_over_first() {
    // Each report over its predecessor, then the last over the first; a
    // column stands for every pair, even where the first cell cannot fill it.
    let cases = &[
        "generated/chain-1000.lp",
        "standalone/send-money/send-money.lp",
    ];
    let first = report(
        cases,
        &[&[4_000_000], &[1_000_000]],
        &[1_000_000, 1_000_000],
        Some(0),
    );
    let second = report(
        cases,
        &[&[2_000_000], &[2_000_000]],
        &[1_000_000, 1_000_000],
        None,
    );
    let third = report(
        cases,
        &[&[1_000_000], &[3_000_000]],
        &[1_000_000, 1_000_000],
        None,
    );
    let comparison = compare(&[
        Labelled {
            label: "first",
            report: &first,
        },
        Labelled {
            label: "second",
            report: &second,
        },
        Labelled {
            label: "third",
            report: &third,
        },
    ])
    .unwrap();
    let markdown = comparison.markdown();
    assert!(markdown.contains("| second/first | third/second | third/first |"));
    // The chain's first report was refused, so no ratio over it exists.
    assert!(markdown.contains("| n/a | 0.500 | n/a |"));
    assert!(markdown.contains("| 2.000 | 1.500 | 3.000 |"));
}

#[test]
fn qualification_evidence_cannot_supply_a_performance_comparison() {
    let only = report(&["case.lp"], &[&[]], &[1], None);
    assert!(
        matches!(compare(&[Labelled { label: "qualification", report: &only }]),
        Err(ViewError::NoTimedPopulation { label }) if label == "qualification")
    );
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
fn failure_appendix_preserves_causes_outside_timed_samples() {
    let mut only = report(&["case.lp"], &[&[1_000_000]], &[2_000_000], None);
    only["passed"] = json!(false);
    let samples = only["report"]["samples"].as_array_mut().unwrap();
    let detail = "native reported incomplete kind=model_construction code=bytes_limit: model construction requires 129 bytes, allowance is 128";
    for round in 0..2 {
        samples.push(json!({
            "slot": {"case": 0, "phase": "memory", "round": round,
                     "producer": {"solver": "native", "profile": 0}},
            "decision": "incomplete", "detail": detail
        }));
    }
    samples.push(json!({
        "slot": {"case": 0, "phase": "diagnostics", "round": 0,
                 "producer": {"solver": "reference"}},
        "decision": "timeout"
    }));
    let comparison = compare(&[Labelled {
        label: "only",
        report: &only,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let cell = &encoded["cells"][0];
    assert_eq!(
        cell["profiles"][0]["reports"]["only"]["median_ns"],
        1_000_000
    );
    assert_eq!(cell["reference"]["only"]["median_ns"], 2_000_000);
    assert!(cell["profiles"][0].get("reference_ratios").is_none());
    let native = format!("native profile index 0: memory: incomplete: {detail}");
    let reference = "reference: diagnostics: timeout: reason unavailable in retained sample";
    assert_eq!(
        cell["failure_reasons"]["only"],
        json!({(native): 2, (reference): 1})
    );
    let markdown = comparison.markdown();
    assert!(markdown.contains("| case | 1.000 [1.000, 1.000] | 0.500 |"));
    assert!(markdown.contains("Recorded failure reasons across all scheduled phases."));
    assert!(markdown.contains("memory: incomplete: native reported incomplete kind=model\\_construction code=bytes\\_limit: model construction requires 129 bytes, allowance is 128"));
    assert!(markdown.contains(&format!("| only | case | {reference} | 1 |")));
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
    // Fastest ratio first; each verdict carries the medians its ratio
    // derives from, and no ratio.
    assert_eq!(verdicts[0]["cell"], "send-money/send-money");
    assert_eq!(verdicts[0]["native_ns"], 500_000);
    assert_eq!(verdicts[0]["reference_ns"], 1_000_000);
    assert_eq!(verdicts[1]["cell"], "generated/queens-11");
    assert_eq!(verdicts[2]["cell"], "generated/chain-1000");
    assert!(
        verdicts
            .iter()
            .all(|verdict| verdict.get("ratio").is_none())
    );
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
fn device_bytes_come_from_the_native_execution_record() {
    let mut only = report(&["generated/chain-1000.lp"], &[&[2_000_000]], &[1], None);
    let stdout = json!({
        "outcome": {"published_models": 1},
        "statistics": {"execution": {"peak_accounted_bytes": 8192}}
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
    assert_eq!(
        encoded["cells"][0]["profiles"][0]["reports"]["only"]["device_bytes"],
        8192
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
    let mut refused = report(
        &["generated/producer-chain-700.lp"],
        &[&[5_000, 6_000]],
        &[1],
        Some(0),
    );
    for sample in refused["report"]["samples"].as_array_mut().unwrap() {
        if sample["decision"] == "refused" {
            let needed = 129 + sample["slot"]["round"].as_u64().unwrap();
            sample["detail"] = json!(format!("formula support bytes limit 128; needed {needed}"));
        }
    }
    let comparison = compare(&[Labelled {
        label: "main",
        report: &refused,
    }])
    .unwrap();
    let encoded = serde_json::to_value(&comparison).unwrap();
    let cell = &encoded["cells"][0]["profiles"][0]["reports"]["main"];
    assert!(cell.get("median_ns").is_none());
    assert_eq!(cell["decisions"], json!({"refused": 2}));
    assert_eq!(
        cell["reasons"],
        json!({
            "timed: refused: formula support bytes limit 128; needed 129": 1,
            "timed: refused: formula support bytes limit 128; needed 130": 1,
        })
    );
    assert!(
        encoded["cells"][0]["profiles"][0]
            .get("reference_ratios")
            .is_none()
    );
    let markdown = comparison.markdown();
    // A refused native cell has no ratio against the reference.
    assert!(markdown.contains("needed 130 ×1 | n/a |"));
    assert!(markdown.contains("producer-chain-700"));
    assert!(markdown.contains("refused"));
    assert!(markdown.contains("support bytes limit 128; needed 129"));
    assert!(markdown.contains("support bytes limit 128; needed 130"));
}

#[test]
fn reports_must_share_their_cells() {
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
}

#[test]
fn reports_must_share_workload_content_at_each_position() {
    let entry = "standalone/n-queens/variant-01.lp";
    let mut one = report(&[entry], &[&[1]], &[1], None);
    one["report"]["workloads"] = json!([
        {"entry": entry, "identity": "ab".repeat(32), "amended": true,
         "sources": [{"edits": [{"before": "8", "after": "10"}]}]}
    ]);
    let mut other = one.clone();
    other["report"]["workloads"][0]["identity"] = json!("cd".repeat(32));
    other["report"]["workloads"][0]["sources"][0]["edits"][0]["after"] = json!("11");
    assert!(matches!(
        compare(&[
            Labelled {
                label: "ten",
                report: &one
            },
            Labelled {
                label: "eleven",
                report: &other
            }
        ]),
        Err(ViewError::Cells { .. })
    ));
}

#[test]
fn reordered_workloads_with_one_entry_are_different_cells() {
    let entry = "standalone/n-queens/variant-01.lp";
    let mut one = report(&[entry, entry], &[&[1], &[2]], &[1, 2], None);
    one["report"]["workloads"] = json!([
        {"entry": entry, "identity": "ab".repeat(32)},
        {"entry": entry, "identity": "cd".repeat(32)}
    ]);
    let mut other = one.clone();
    other["report"]["workloads"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(matches!(
        compare(&[
            Labelled {
                label: "forward",
                report: &one
            },
            Labelled {
                label: "reverse",
                report: &other
            }
        ]),
        Err(ViewError::Cells { .. })
    ));
}

fn derived_report() -> Value {
    let entry = "generated/chain-1000.lp";
    let mut value = report(&[entry], &[&[1]], &[1], None);
    value["report"]["schema"] = json!(2);
    value["report"]["workloads"] = json!([
        {"entry": entry, "identity": "ab".repeat(32)}
    ]);
    value
}

#[test]
fn equal_workload_identities_compare() {
    let one = derived_report();
    let other = one.clone();
    assert!(
        compare(&[
            Labelled {
                label: "a",
                report: &one
            },
            Labelled {
                label: "b",
                report: &other
            },
        ])
        .is_ok()
    );
}

#[test]
fn a_derived_report_requires_workloads() {
    let mut one = derived_report();
    one["report"].as_object_mut().unwrap().remove("workloads");
    assert!(matches!(
        compare(&[Labelled {
            label: "a",
            report: &one
        }]),
        Err(ViewError::Malformed { .. })
    ));
}

#[test]
fn each_workload_requires_a_content_identity() {
    let mut one = derived_report();
    one["report"]["workloads"][0]
        .as_object_mut()
        .unwrap()
        .remove("identity");
    assert!(matches!(
        compare(&[Labelled {
            label: "a",
            report: &one
        }]),
        Err(ViewError::Malformed { .. })
    ));
}

#[test]
fn malformed_workload_identities_are_refused() {
    for identity in [
        json!(null),
        json!(17),
        json!(""),
        json!("ab"),
        json!("g".repeat(64)),
    ] {
        let mut one = derived_report();
        one["report"]["workloads"][0]["identity"] = identity;
        assert!(matches!(
            compare(&[Labelled {
                label: "a",
                report: &one
            }]),
            Err(ViewError::Malformed { .. })
        ));
    }
}

#[test]
fn workload_positions_must_match_the_case_population() {
    for workloads in [
        json!(null),
        json!("invalid"),
        json!([]),
        json!([
            {"entry": "generated/chain-2000.lp", "identity": "ab".repeat(32)}
        ]),
    ] {
        let mut one = derived_report();
        one["report"]["workloads"] = workloads;
        assert!(matches!(
            compare(&[Labelled {
                label: "a",
                report: &one
            }]),
            Err(ViewError::Malformed { .. })
        ));
    }
}

#[test]
fn corpus_reports_must_share_the_sealed_manifest() {
    let one = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    let mut other = one.clone();
    other["report"]["before"][2]["sha256"] = json!("aa".repeat(32));
    assert!(matches!(
        compare(&[
            Labelled {
                label: "a",
                report: &one
            },
            Labelled {
                label: "b",
                report: &other
            },
        ]),
        Err(ViewError::Cells { .. })
    ));
}

#[test]
fn a_corpus_path_does_not_replace_a_workload_identity() {
    let one = derived_report();
    let other = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    assert!(matches!(
        compare(&[
            Labelled {
                label: "a",
                report: &one
            },
            Labelled {
                label: "b",
                report: &other
            },
        ]),
        Err(ViewError::Cells { .. })
    ));
}

#[test]
fn reports_must_share_their_profiles() {
    let one = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    let mut other = one.clone();
    other["report"]["plan"]["profiles"][0]["workers"] = json!(2);
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
        Err(ViewError::Profiles { .. })
    ));
}

#[test]
fn an_empty_comparison_is_refused() {
    assert!(matches!(compare(&[]), Err(ViewError::Empty)));
}

#[test]
fn a_repeated_label_is_refused() {
    let one = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
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
        {"entry": "generated/chain-1000.lp", "identity": "11".repeat(32),
         "generated": {"family": "chain", "size": 1000}},
        {"entry": "standalone/n-queens/variant-01.lp", "amended": true,
         "identity": "22".repeat(32),
         "sources": [{"edits": [{"before": "8", "after": "10"}]}]},
        {"entry": "standalone/n-queens/variant-01.lp", "amended": true,
         "identity": "33".repeat(32),
         "sources": [{"edits": [{"before": "8", "after": "11"}]}]},
        {"entry": "standalone/send-money/send-money.lp", "identity": "44".repeat(32),
         "amended": false, "sources": []}
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
fn workload_labels_retain_every_recorded_edit() {
    let entry = "standalone/n-queens/variant-01.lp";
    let mut one = report(&[entry], &[&[1]], &[1], None);
    one["report"]["workloads"] = json!([{
        "entry": entry, "amended": true, "identity": "22".repeat(32),
        "sources": [
            {"edits": [{"before": "8", "after": "10"}]},
            {"edits": [{"before": "2", "after": "4"}]}
        ]
    }]);
    let comparison = compare(&[Labelled {
        label: "one",
        report: &one,
    }])
    .unwrap();
    assert_eq!(comparison.cells[0].label, "n-queens/variant-01 8→10 2→4");
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
    samples[blocker]["detail"] = json!("process deadline | 5 seconds\n<b>stopped</b>");
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
    let reason = format!(
        "timed: not_attempted: reason unavailable in retained sample; blocked by sample {blocker} (qualification, timeout): process deadline | 5 seconds\n<b>stopped</b>"
    );
    assert_eq!(
        encoded["cells"][0]["profiles"][0]["reports"]["a"]["reasons"],
        json!({(reason): 2})
    );
    let markdown = comparison.markdown();
    assert!(markdown.contains(
        "(qualification, timeout): process deadline \\| 5 seconds\\n&lt;b&gt;stopped&lt;/b&gt;"
    ));
    assert!(!markdown.contains("5 seconds\n<b>"));
}

#[test]
fn reports_may_differ_in_the_search_method_alone() {
    let mut regions = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    regions["report"]["plan"]["profiles"][0]["search"] = json!("regions");
    let mut clauses = regions.clone();
    clauses["report"]["plan"]["profiles"][0]["search"] = json!("clauses");
    let comparison = compare(&[
        Labelled {
            label: "regions",
            report: &regions,
        },
        Labelled {
            label: "clauses",
            report: &clauses,
        },
    ])
    .unwrap();
    assert_eq!(comparison.methods["regions"], "regions");
    assert_eq!(comparison.methods["clauses"], "clauses");
}

#[test]
fn a_report_before_the_search_field_spells_its_method_as_candidates() {
    // Reports written before the `search` field carried the method under
    // the name `candidates`; it is read as the method and left out of the
    // profile comparison.
    let mut current = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    current["report"]["plan"]["profiles"][0]["search"] = json!("clauses");
    let mut older = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    older["report"]["plan"]["profiles"][0]["candidates"] = json!("clauses");
    let comparison = compare(&[
        Labelled {
            label: "current",
            report: &current,
        },
        Labelled {
            label: "older",
            report: &older,
        },
    ])
    .unwrap();
    assert_eq!(comparison.methods["older"], "clauses");
}

#[test]
fn a_report_naming_region_workers_separately_is_read_with_them() {
    // One campaign's reports named the region workers beside the method;
    // they are read into the method and left out of the profile comparison.
    let mut current = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    current["report"]["plan"]["profiles"][0]["search"] = json!("regions");
    let mut workers = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    workers["report"]["plan"]["profiles"][0]["search"] = json!("regions");
    workers["report"]["plan"]["profiles"][0]["region_workers"] = json!(4);
    let comparison = compare(&[
        Labelled {
            label: "current",
            report: &current,
        },
        Labelled {
            label: "workers",
            report: &workers,
        },
    ])
    .unwrap();
    assert_eq!(comparison.methods["workers"], "regions with 4 workers");
}

#[test]
fn a_report_whose_profiles_differ_in_method_is_refused() {
    // A scoreboard is one method's standing against the reference, so the
    // profiles of a report must agree on the method.
    let mut mixed = report(&["generated/chain-1000.lp"], &[&[1]], &[1], None);
    let profiles = mixed["report"]["plan"]["profiles"].as_array_mut().unwrap();
    let mut clauses = profiles[0].clone();
    profiles[0]["search"] = json!("regions");
    clauses["search"] = json!("clauses");
    profiles.push(clauses);
    assert!(matches!(
        compare(&[Labelled {
            label: "mixed",
            report: &mixed
        }]),
        Err(ViewError::Methods { label }) if label == "mixed"
    ));
}

#[test]
fn loaded_reports_use_the_same_comparison_contract() {
    use zetesis_validation::performance::series::{ReportSource, read_compare};
    let document = report(&["generated/chain-1000.lp"], &[&[9, 3, 6]], &[12], None);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    let bytes = serde_json::to_vec(&document).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let loaded = read_compare(
        &[ReportSource {
            label: "run",
            path: &path,
        }],
        u64::try_from(bytes.len()).unwrap(),
    )
    .unwrap();
    let direct = compare(&[Labelled {
        label: "run",
        report: &document,
    }])
    .unwrap();
    assert_eq!(
        serde_json::to_value(loaded).unwrap(),
        serde_json::to_value(direct).unwrap()
    );
}

#[test]
fn report_source_ceiling_refuses_before_decoding() {
    use zetesis_validation::performance::series::{ReadError, ReportSource, read_compare};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("oversized.json");
    std::fs::write(&path, b"not JSON").unwrap();
    assert!(
        matches!(read_compare(&[ReportSource { label: "large", path: &path }], 3), Err(ReadError::Bytes { label, limit: 3 }) if label == "large")
    );
}

#[test]
fn invalid_json_retains_its_typed_failure() {
    use zetesis_validation::performance::series::{ReadError, ReportSource, read_compare};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("malformed.json");
    std::fs::write(&path, b"not JSON").unwrap();
    assert!(matches!(
        read_compare(
            &[ReportSource {
                label: "bad",
                path: &path
            }],
            100
        ),
        Err(ReadError::Json(_))
    ));
}

/// The same report as a clingo-free campaign writes it: no reference samples,
/// no clingo seal, and the policy recorded as `clingo_free`.
fn clingo_free(mut report: Value) -> Value {
    report["report"]["plan"]["reference_policy"] = json!("clingo_free");
    report["report"]["before"].as_array_mut().unwrap().remove(1);
    report["report"]["samples"]
        .as_array_mut()
        .unwrap()
        .retain(|sample| sample["slot"]["producer"]["solver"] != "reference");
    report
}

/// A report with clingo and the same cells as a clingo-free one, compared in that order.
fn mixed() -> (Value, Value) {
    let cases = [
        "generated/chain-1000.lp",
        "standalone/send-money/send-money.lp",
    ];
    let native: &[&[u64]] = &[
        &[3_000_000, 1_000_000, 2_000_000],
        &[4_000_000, 4_000_000, 4_000_000],
    ];
    (
        report(&cases, native, &[8_000_000, 15_000_000], None),
        clingo_free(report(&cases, native, &[8_000_000, 15_000_000], None)),
    )
}

fn compare_mixed(
    with_clingo: &Value,
    without: &Value,
) -> zetesis_validation::performance::series::Comparison {
    compare(&[
        Labelled {
            label: "clingo",
            report: with_clingo,
        },
        Labelled {
            label: "free",
            report: without,
        },
    ])
    .unwrap()
}

#[test]
fn each_cell_records_what_qualified_it_in_each_report() {
    let (with_clingo, without) = mixed();
    let encoded = serde_json::to_value(compare_mixed(&with_clingo, &without)).unwrap();
    for cell in encoded["cells"].as_array().unwrap() {
        assert_eq!(
            cell["qualification"],
            json!({"clingo": "clingo", "free": "contract"})
        );
    }
}

#[test]
fn a_cell_needing_clingo_records_that_it_does() {
    let (with_clingo, mut without) = mixed();
    // Without a recorded contract a clingo-free campaign cannot qualify the
    // second case: its census needs clingo and its later positions are blocked.
    for sample in without["report"]["samples"].as_array_mut().unwrap() {
        if sample["slot"]["case"] == 1 {
            sample["decision"] = json!(if sample["slot"]["phase"] == "qualification" {
                "needs_clingo"
            } else {
                "not_attempted"
            });
        }
    }
    let encoded = serde_json::to_value(compare_mixed(&with_clingo, &without)).unwrap();
    assert_eq!(encoded["cells"][0]["qualification"]["free"], "contract");
    assert_eq!(encoded["cells"][1]["qualification"]["free"], "needs_clingo");
}

#[test]
fn a_clingo_free_report_is_sealed_without_clingo() {
    let (with_clingo, without) = mixed();
    let comparison = compare_mixed(&with_clingo, &without);
    assert_eq!(comparison.provenance["free"].reference_sha256, None);
    // The manifest follows the native seal directly, and matches the other report's.
    assert_eq!(
        comparison.provenance["free"].manifest_sha256,
        "ef".repeat(32)
    );
}

#[test]
fn only_reports_with_clingo_have_scoreboards() {
    let (with_clingo, without) = mixed();
    let comparison = compare_mixed(&with_clingo, &without);
    assert!(!comparison.scoreboards.is_empty());
    assert!(
        comparison
            .scoreboards
            .iter()
            .all(|board| board.report == "clingo")
    );
}

#[test]
fn markdown_says_clingo_did_not_run_for_a_clingo_free_report() {
    let (with_clingo, without) = mixed();
    let markdown = compare_mixed(&with_clingo, &without).markdown();
    // The reference table: clingo's time with clingo, "not run" without it.
    assert!(
        markdown.contains("| generated/chain-1000 | 8.000 [8.000, 8.000] | not run |"),
        "{markdown}"
    );
    // The native-over-reference ratio columns likewise.
    assert!(markdown.contains(" | 0.250 | not run |"), "{markdown}");
}
