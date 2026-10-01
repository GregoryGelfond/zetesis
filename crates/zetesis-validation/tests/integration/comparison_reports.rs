//! Independent strict completed-report contracts transferred to shared decoders.
use serde_json::{Value, json};
use zetesis_validation::answers::{self, Limits};

fn optimal() -> String {
    "Answer: 1\na\nOptimization: -2 7\nAnswer: 2\na\nOptimization: -2 7\nOPTIMUM FOUND\nModels: 2\nCoverage: exhausted\n".into()
}

fn compact_optimal() -> String {
    format!(
        "zetesis 0.1.6\nCopyright (c) 2026 Gregory Gelfond\n\n\
         Backend: CPU · 1 thread · eager grounding\n\n{}\n\
         Time: grounding 0.100 ms · solving 0.200 ms\n",
        optimal().replace("Coverage: exhausted\n", "")
    )
}

#[test]
fn compact_optima_preserve_display_multiplicity() {
    let source = compact_optimal().replace("\na\n", "\na a\n");
    let parsed = answers::native_text(source.as_bytes(), true, Limits::default()).unwrap();
    assert_eq!(parsed.cost(), Some([-2, 7].as_slice()));
    assert_eq!(parsed.model_count(), 2);
    assert_eq!(
        parsed.displays(),
        &[(vec!["a".to_owned(), "a".to_owned()], 2)]
    );
}

#[test]
fn compact_terminal_status_matches_witnesses() {
    for (source, satisfiable) in [
        ("Answer: 1\na\nSATISFIABLE\nModels: 1\n", true),
        ("UNSATISFIABLE\nModels: 0\n", false),
    ] {
        let parsed = answers::native_text(source.as_bytes(), false, Limits::default()).unwrap();
        assert_eq!(parsed.satisfiable(), satisfiable);
    }
    for source in [
        "Answer: 1\na\nUNSATISFIABLE\nModels: 1\n",
        "SATISFIABLE\nModels: 0\n",
    ] {
        assert!(answers::native_text(source.as_bytes(), false, Limits::default()).is_err());
    }
}

#[test]
fn compact_counts_must_be_unqualified_decimal_integers() {
    for count in [
        "",
        "+2",
        "-2",
        "2.0",
        "2; candidates examined: 2",
        "2 (answer limit reached)",
        "2 (search incomplete)",
        "18446744073709551616",
    ] {
        let source = compact_optimal().replace("Models: 2", &format!("Models: {count}"));
        assert!(
            answers::native_text(source.as_bytes(), true, Limits::default()).is_err(),
            "{count}"
        );
    }
}

#[test]
fn compact_summary_counts_must_match_witnesses() {
    for count in [0, 1, 3] {
        let source = compact_optimal().replace("Models: 2", &format!("Models: {count}"));
        assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
    }
}

#[test]
fn compact_finish_records_must_be_present_and_unique() {
    for record in ["OPTIMUM FOUND\n", "Models: 2\n"] {
        let missing = compact_optimal().replace(record, "");
        let duplicate = compact_optimal() + record;
        for source in [missing, duplicate] {
            assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
        }
    }
}

#[test]
fn compact_reports_cannot_override_incomplete_evidence() {
    for suffix in [
        "SATISFIABLE\n",
        "UNSATISFIABLE\n",
        "INCOMPLETE\n",
        "INCOMPLETE: timeout\n",
        "Coverage: partial\n",
        "Coverage: exhausted\nCoverage: exhausted\n",
    ] {
        let source = compact_optimal() + suffix;
        assert!(
            answers::native_text(source.as_bytes(), true, Limits::default()).is_err(),
            "{suffix}"
        );
    }
}

#[test]
fn native_final_optimum_cannot_include_worse_witnesses() {
    for source in [optimal(), compact_optimal()] {
        let source = source.replacen("Optimization: -2 7", "Optimization: -1 7", 1);
        assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
    }
}

#[test]
fn optimized_native_reports_require_optimality_evidence() {
    for source in [optimal(), compact_optimal()] {
        let source = source.replace("OPTIMUM FOUND", "SATISFIABLE");
        assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
    }
}

#[test]
fn multiline_displays_cannot_supply_finish_metadata() {
    let symbol = "p(\"one\nAnswer: 42\nModels: 999\nCoverage: partial\nUNSATISFIABLE\ntwo\")";
    let native = format!(
        "Answer: 1\n{symbol}\nOptimization: -2 7\nOPTIMUM FOUND\nModels: 1\nCoverage: exhausted\n"
    );
    let reference = json!({"Result":"OPTIMUM FOUND", "Models":{"More":"no","Number":2,"Optimal":1,"Optimum":"yes","Costs":[-2,7]}, "Call":[{"Witnesses":[{"Value":[symbol],"Costs":[-2,7]},{"Value":[symbol],"Costs":[-2,7]}]}]});
    let reference =
        answers::clingo_json(&serde_json::to_vec(&reference).unwrap(), Limits::default()).unwrap();
    let compact = native.replace("Coverage: exhausted\n", "");
    for native in [native, compact] {
        let native = answers::native_text(native.as_bytes(), true, Limits::default()).unwrap();
        assert!(answers::same_displays(&reference, &native));
    }
}

#[test]
fn native_answer_labels_require_numeric_identifiers() {
    let source = optimal().replacen("Answer: 1", "Answer: fabricated", 1);
    assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
}

#[test]
fn native_finish_records_must_be_unique() {
    for suffix in ["Coverage: exhausted\n", "SATISFIABLE\n", "Models: 2\n"] {
        assert!(
            answers::native_text((optimal() + suffix).as_bytes(), true, Limits::default()).is_err(),
            "{suffix}"
        );
    }
}

#[test]
fn incomplete_markers_prevent_completed_evidence() {
    for suffix in ["INCOMPLETE\n", "INCOMPLETE: timeout\n"] {
        assert!(
            answers::native_text((optimal() + suffix).as_bytes(), true, Limits::default()).is_err()
        );
    }
}

#[test]
fn every_optimal_native_witness_requires_one_cost_vector() {
    for source in [optimal(), compact_optimal()] {
        for source in [
            source.replacen("Optimization: -2 7\n", "", 1),
            source.replacen(
                "Optimization: -2 7",
                "Optimization: -2 7\nOptimization: -2 7",
                1,
            ),
        ] {
            assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
        }
    }
}

#[test]
fn clingo_object_positions_reject_nonobjects() {
    for replacement in [
        Value::Null,
        json!([]),
        json!(1),
        json!(true),
        json!("object"),
    ] {
        for pointer in ["", "/Models", "/Call/0", "/Call/0/Witnesses/0"] {
            let mut report = json!({"Result":"SATISFIABLE", "Models":{"More":"no","Number":1},"Call":[{"Witnesses":[{"Value":["a"]}]}]});
            *report.pointer_mut(pointer).unwrap() = replacement.clone();
            assert!(
                answers::clingo_json(&serde_json::to_vec(&report).unwrap(), Limits::default())
                    .is_err()
            );
        }
    }
}

#[test]
fn optn_requires_the_incumbent_to_reappear() {
    let report = json!({"Result":"OPTIMUM FOUND", "Models":{"More":"no","Number":2,"Optimal":1,"Optimum":"yes","Costs":[1]},"Call":[{"Witnesses":[{"Value":["unreplayed"],"Costs":[1]},{"Value":["a"],"Costs":[1]}]}]});
    assert!(
        answers::clingo_json(&serde_json::to_vec(&report).unwrap(), Limits::default()).is_err()
    );
}

#[test]
fn repeated_empty_displays_preserve_optimal_multiplicity() {
    let report = json!({"Result":"OPTIMUM FOUND", "Models":{"More":"no","Number":3,"Optimal":2,"Optimum":"yes","Costs":[0]},"Call":[{"Witnesses":[{"Value":[],"Costs":[0]},{"Value":[],"Costs":[0]},{"Value":[],"Costs":[0]}]}]});
    let native = "Answer: 1\n\nOptimization: 0\nAnswer: 2\n\nOptimization: 0\nOPTIMUM FOUND\nModels: 2\nCoverage: exhausted\n";
    let native = answers::native_text(native.as_bytes(), true, Limits::default()).unwrap();
    let reference =
        answers::clingo_json(&serde_json::to_vec(&report).unwrap(), Limits::default()).unwrap();
    assert!(answers::same_displays(&reference, &native));
    assert_eq!(native.displays(), &[(Vec::new(), 2)]);
}

#[test]
fn frozen_production_reports_preserve_display_identity() {
    let document: Value =
        serde_json::from_str(include_str!("../fixtures/objective-free-protocol.json")).unwrap();
    let models: Vec<Vec<String>> = serde_json::from_value(document["models"].clone()).unwrap();
    for run in document["runs"].as_array().unwrap() {
        let source = run["stdout"].as_str().unwrap().as_bytes();
        let reported = match run["solver"].as_str().unwrap() {
            "zetesis" => answers::native_text(source, false, Limits::default()),
            "clingo" => answers::clingo_json(source, Limits::default()),
            other => panic!("unrecognized fixture producer: {other}"),
        }
        .unwrap();
        assert_eq!(reported.displays(), &[(models[0].clone(), 1)]);
    }
}

#[test]
fn frozen_optimal_reports_preserve_complete_cost_vectors() {
    let document: Value =
        serde_json::from_str(include_str!("../fixtures/optimal-protocol.json")).unwrap();
    for case in document["cases"].as_array().unwrap() {
        let costs: Vec<i64> = serde_json::from_value(case["expected_cost"].clone()).unwrap();
        let reports: Vec<_> = case["runs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|run| {
                let source = run["stdout"].as_str().unwrap().as_bytes();
                match run["solver"].as_str().unwrap() {
                    "zetesis" => answers::native_text(source, true, Limits::default()),
                    "clingo" => answers::clingo_json(source, Limits::default()),
                    other => panic!("unrecognized fixture producer: {other}"),
                }
                .unwrap()
            })
            .collect();
        assert_eq!(reports[0].cost(), Some(costs.as_slice()));
        assert!(answers::same_displays(&reports[0], &reports[1]));
    }
}
