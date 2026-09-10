//! Pure policy tests with independently authored protocol observations.
use std::{fmt::Write as _, path::Path};
use zetesis_maintenance::coverage::{self, Floor, Metadata, Mode, Observation, Tool};

const TABLE: &str = include_str!("support/physical-selection.txt");
fn output(tests: &[String], library: bool) -> String {
    let mut value = String::new();
    for test in tests {
        writeln!(value, "test {test} ... adapter=fixture\nok").unwrap();
    }
    if library {
        value.push_str("test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n");
    }
    writeln!(value, "test result: ok. {} passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s", tests.len()).unwrap();
    value
}
#[test]
fn floors_admit_only_integer_percentages() {
    for value in ["", "NaN", "-1", "+1", "1.5", "101", "999", "９１"] {
        assert!(Floor::parse(value).is_err(), "{value}");
    }
    for value in ["0", "91", "100", "UNMEASURED"] {
        assert_eq!(Floor::parse(value).unwrap().label(), value);
    }
}
#[test]
fn unmeasured_floor_cannot_gate() {
    let floor = Floor::parse("UNMEASURED").unwrap();
    assert!(floor.admit(Mode::Gate).is_err());
    assert!(floor.admit(Mode::Baseline).is_ok());
}
#[test]
fn measured_floors_cannot_decrease() {
    for (old, new, expected) in [
        ("UNMEASURED", "91", true),
        ("91", "91", true),
        ("91", "92", true),
        ("91", "90", false),
        ("91", "UNMEASURED", false),
    ] {
        assert_eq!(
            coverage::ratchet(Floor::parse(old).unwrap(), Floor::parse(new).unwrap()).is_ok(),
            expected
        );
    }
}
#[test]
fn previous_identity_obeys_event_precedence() {
    let first = "a".repeat(40);
    let second = "b".repeat(40);
    for event in [
        serde_json::json!({"before":first}),
        serde_json::json!({"pull_request":{"base":{"sha":first}},"before":second}),
        serde_json::json!({"pull_request":{"base":{"sha":""}},"before":first}),
    ] {
        assert_eq!(
            coverage::previous_revision(&serde_json::to_vec(&event).unwrap()).unwrap(),
            Some(first.clone())
        );
    }
}
#[test]
fn absent_history_does_not_invent_a_commit() {
    for event in [
        b"{}".to_vec(),
        serde_json::to_vec(&serde_json::json!({"before":"0".repeat(40)})).unwrap(),
    ] {
        assert_eq!(coverage::previous_revision(&event).unwrap(), None);
    }
}
#[test]
fn malformed_history_is_refused() {
    for event in [
        r#"{"before":true}"#.to_owned(),
        r#"{"before":"bad"}"#.into(),
        r#"{"before":null,"before":""}"#.into(),
        format!(r#"{{"before":"{}"}}"#, "A".repeat(40)),
    ] {
        assert!(coverage::previous_revision(event.as_bytes()).is_err());
    }
}
#[test]
fn every_physical_target_requires_complete_individual_results() {
    for group in coverage::selection(TABLE).unwrap() {
        coverage::physical_result(&output(&group.tests, group.target_kind == "lib"), &group)
            .unwrap();
        for output in [
            String::new(),
            output(&group.tests[1..], group.target_kind == "lib"),
            output(&group.tests, group.target_kind == "lib").replace("\nok\n", "\nFAILED\n"),
            output(&group.tests, group.target_kind == "lib").replace("0 ignored", "1 ignored"),
        ] {
            assert!(coverage::physical_result(&output, &group).is_err());
        }
    }
}
#[test]
fn physical_selection_is_a_fixed_contract() {
    for table in [
        TABLE.replacen("lazy|hardware_lazy|4|", "altered|hardware_lazy|4|", 1),
        TABLE.replace("metal_support_matches_exact_reduct_semantics", "unknown"),
        TABLE.lines().skip(1).collect::<Vec<_>>().join("\n"),
    ] {
        assert!(coverage::selection(&table).is_err());
    }
}
#[test]
fn metal_selection_refuses_vulkan_substitution() {
    for (metal, vulkan) in [
        (
            "metal_relation_masks_match_typed_rows",
            "vulkan_relation_masks_match_typed_rows",
        ),
        (
            "metal_relation_refusals_preserve_prepared_view",
            "vulkan_relation_refusals_preserve_prepared_view",
        ),
        (
            "metal_relation_measurement_keeps_complete_masks",
            "vulkan_relation_measurement_keeps_complete_masks",
        ),
    ] {
        let changed = TABLE.replacen(metal, vulkan, 1);
        assert_ne!(changed, TABLE);
        assert!(coverage::selection(&changed).is_err());
    }
}

fn metadata(
    version: &str,
    physical: bool,
) -> Result<serde_json::Value, zetesis_maintenance::Error> {
    let digest = "a".repeat(64);
    let tool = Tool {
        path: Path::new("/fixture/llvm"),
        sha256: &digest,
        version,
    };
    coverage::metadata(Metadata {
        mode: Mode::Gate,
        floor: "91",
        physical_table: physical.then_some(TABLE),
        observation: Observation {
            rustc: "rustc 1.97.1\nhost: fixture\nLLVM version: 22.1.6",
            llvm_cov: tool,
            llvm_profdata: tool,
        },
    })
}
#[test]
fn llvm_version_requires_the_pinned_build() {
    for version in ["22.1.6", "22.1.6-rust-1.97.1-stable"] {
        metadata(&format!("LLVM version {version}"), false).unwrap();
    }
    for version in [
        "22.1.5",
        "22.1.5-rust-1.97.1-stable",
        "22.1.6-rust-1.96.0-stable",
        "22.1.6-rust-1.97.1-nightly",
        "22.1.6-custom",
        "22.1.6-rust-1.97.1-stable-extra",
        "22.1.6 trailing text",
    ] {
        assert!(metadata(&format!("LLVM version {version}"), false).is_err());
    }
}
#[test]
fn physical_metadata_keeps_floor_populations_separate() {
    let record = metadata("LLVM version 22.1.6", true).unwrap();
    assert_eq!(record["profiles_merged"], false);
    assert_eq!(
        record["floor_profiles"],
        serde_json::json!(["workspace", "cli-cpu"])
    );
    assert_eq!(record["expected_physical_tests"], 30);
    assert_eq!(record["physical_test_groups"].as_array().unwrap().len(), 11);
    assert_eq!(
        record["project_added_filename_filters"],
        serde_json::json!([])
    );
}
#[test]
fn portable_metadata_makes_no_physical_claim() {
    let record = metadata("LLVM version 22.1.6", false).unwrap();
    assert_eq!(record["expected_physical_tests"], 0);
    assert!(record["physical_scope"].is_null());
}

#[test]
fn coverage_inputs_enforce_their_byte_ceiling() {
    let oversized = " ".repeat(coverage::MAX_INPUT_BYTES + 1);
    assert!(matches!(
        coverage::previous_revision(oversized.as_bytes()),
        Err(zetesis_maintenance::Error::Limit { .. })
    ));
    let group = coverage::selection(TABLE).unwrap().remove(0);
    assert!(matches!(
        coverage::physical_result(&oversized, &group),
        Err(zetesis_maintenance::Error::Limit { .. })
    ));
    assert!(matches!(
        metadata(&oversized, false),
        Err(zetesis_maintenance::Error::Limit { .. })
    ));
}

#[test]
fn unknown_test_boundaries_cannot_hide_missing_outcomes() {
    let group = coverage::selection(TABLE).unwrap().remove(0);
    let output = output(&group.tests, true).replacen(
        "adapter=fixture\nok",
        "adapter=fixture\ntest malformed boundary\nok",
        1,
    );
    assert!(coverage::physical_result(&output, &group).is_err());
}
