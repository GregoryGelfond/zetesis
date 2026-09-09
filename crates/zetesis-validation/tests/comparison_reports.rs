//! Independent strict completed-report contracts transferred to shared decoders.
use serde_json::{Value, json};
use zetesis_validation::answers::{self, Limits};

fn optimal() -> String {
    "Answer: 1\na\nOptimization: -2 7\nAnswer: 2\na\nOptimization: -2 7\nOPTIMUM FOUND\nModels: 2\nCoverage: exhausted\n".into()
}

#[test]
fn native_final_optimum_cannot_include_worse_witnesses() {
    let source = optimal().replacen("Optimization: -2 7", "Optimization: -1 7", 1);
    assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
}

#[test]
fn optimized_native_reports_require_optimality_evidence() {
    let source = optimal().replace("OPTIMUM FOUND", "SATISFIABLE");
    assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
}

#[test]
fn multiline_displays_cannot_supply_finish_metadata() {
    let symbol = "p(\"one\nAnswer: 42\nModels: 999\nCoverage: partial\nUNSATISFIABLE\ntwo\")";
    let native = format!(
        "Answer: 1\n{symbol}\nOptimization: -2 7\nOPTIMUM FOUND\nModels: 1\nCoverage: exhausted\n"
    );
    let reference = json!({"Result":"OPTIMUM FOUND", "Models":{"More":"no","Number":2,"Optimal":1,"Optimum":"yes","Costs":[-2,7]}, "Call":[{"Witnesses":[{"Value":[symbol],"Costs":[-2,7]},{"Value":[symbol],"Costs":[-2,7]}]}]});
    let native = answers::native_text(native.as_bytes(), true, Limits::default()).unwrap();
    let reference =
        answers::clingo_json(&serde_json::to_vec(&reference).unwrap(), Limits::default()).unwrap();
    assert!(answers::same_displays(&reference, &native));
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
    for source in [
        optimal().replacen("Optimization: -2 7\n", "", 1),
        optimal().replacen(
            "Optimization: -2 7",
            "Optimization: -2 7\nOptimization: -2 7",
            1,
        ),
    ] {
        assert!(answers::native_text(source.as_bytes(), true, Limits::default()).is_err());
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
        serde_json::from_str(include_str!("fixtures/objective-free-protocol.json")).unwrap();
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
