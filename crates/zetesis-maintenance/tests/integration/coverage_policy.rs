//! Pure policy tests with independently authored protocol observations.
use std::{fmt::Write as _, path::Path};
use zetesis_maintenance::coverage::{self, Floor, Metadata, Mode, Observation, Tool};

const TABLE: &str = include_str!("../support/physical-selection.txt");
const VULKAN_TABLE: &str = include_str!("../support/physical-selection-vulkan.txt");
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
    for group in coverage::selection(TABLE).unwrap().groups {
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
fn integration_selections_admit_other_packages_empty_binaries() {
    // `--test integration` across the workspace runs every package's
    // integration binary; only the owning package's reports matches.
    let groups = coverage::selection(TABLE).unwrap().groups;
    let group = groups
        .iter()
        .find(|group| group.target == "integration")
        .unwrap();
    coverage::physical_result(&output(&group.tests, true), group).unwrap();
    let twice = format!(
        "{}{}",
        output(&group.tests, true),
        output(&group.tests, false)
    );
    assert!(coverage::physical_result(&twice, group).is_err());
}
#[test]
fn physical_selection_is_a_fixed_contract() {
    for table in [
        TABLE.replacen("lazy|integration|4|", "altered|integration|4|", 1),
        TABLE.replacen("solve-context|lib|3|", "cli-context|lib|3|", 1),
        TABLE.replacen(
            "language-consumers|integration|2|",
            "language-consumers|formula_gpu|2|",
            1,
        ),
        TABLE.replacen("static|integration|2|", "static|hardware_formula|2|", 1),
        TABLE.replace(
            "metal_static_oracle_matches_independent_closures",
            "exact_static_oracle_matches_independent_cpu_closures",
        ),
        TABLE
            .replace("cli-formula|formula_gpu|3|", "cli-formula|formula_gpu|2|")
            .replace(
                " physical::ordinary_metal_table_joins_preserve_complete_answers",
                "",
            ),
        TABLE
            .replace(
                "session-resources|integration|9|",
                "session-resources|integration|8|",
            )
            .replace(
                " session_resources_gpu::metal_terminal_sessions_preserve_complete_families",
                "",
            ),
        TABLE.lines().take(15).collect::<Vec<_>>().join("\n"),
        TABLE.replace("metal_support_matches_exact_reduct_semantics", "unknown"),
        TABLE.lines().skip(1).collect::<Vec<_>>().join("\n"),
    ] {
        assert!(coverage::selection(&table).is_err());
    }
}
#[test]
fn library_groups_require_one_positive_summary() {
    for group in coverage::selection(TABLE)
        .unwrap()
        .groups
        .into_iter()
        .filter(|group| group.target_kind == "lib")
    {
        let mut observed = output(&group.tests, true);
        observed.push_str("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n");
        assert!(coverage::physical_result(&observed, &group).is_err());
    }
}
#[test]
fn library_results_cannot_qualify_another_group() {
    let groups = coverage::selection(TABLE).unwrap().groups;
    let libraries: Vec<_> = groups
        .iter()
        .filter(|group| group.target_kind == "lib")
        .collect();
    assert_eq!(libraries.len(), 2);
    for (expected, observed) in [(libraries[0], libraries[1]), (libraries[1], libraries[0])] {
        assert!(coverage::physical_result(&output(&observed.tests, true), expected).is_err());
    }
}
#[test]
fn the_vulkan_selection_is_the_metal_selection_on_its_own_backend() {
    let metal = coverage::selection(TABLE).unwrap().groups;
    let vulkan = coverage::selection(VULKAN_TABLE).unwrap().groups;
    assert_eq!(metal.len(), vulkan.len());
    for (metal, vulkan) in metal.iter().zip(&vulkan) {
        assert_eq!(metal.group, vulkan.group);
        assert_eq!(metal.target, vulkan.target);
        assert_eq!(metal.expected_tests, vulkan.expected_tests);
        assert_eq!(metal.tests.len(), vulkan.tests.len());
        // Each backend's tests are its own; none stands in both selections.
        assert!(metal.tests.iter().all(|test| !vulkan.tests.contains(test)));
        assert!(vulkan.tests.iter().all(|test| test.contains("vulkan")));
    }
    // One row of the other backend is neither selection.
    let mixed = VULKAN_TABLE.replacen(
        VULKAN_TABLE.lines().next().unwrap(),
        TABLE.lines().next().unwrap(),
        1,
    );
    assert!(coverage::selection(&mixed).is_err());
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
        (
            "metal_formula_executes_while_relation_columns_remain_prepared",
            "vulkan_formula_executes_while_relation_columns_remain_prepared",
        ),
        (
            "engine::resource_tests::metal_closure_retains_the_supplied_context",
            "engine::resource_tests::vulkan_closure_retains_the_supplied_context",
        ),
        (
            "formula_execution::tests::resource_tests::metal_formula_retains_the_supplied_context",
            "formula_execution::tests::resource_tests::vulkan_formula_retains_the_supplied_context",
        ),
        (
            "metal_resources_preserve_independent_sessions",
            "vulkan_resources_preserve_independent_sessions",
        ),
        (
            "metal_resource_policy_refusal_preserves_reuse",
            "vulkan_resource_policy_refusal_preserves_reuse",
        ),
        (
            "metal_resources_preserve_cpu_policies",
            "vulkan_resources_preserve_cpu_policies",
        ),
        (
            "metal_observer_failure_preserves_resource_reuse",
            "vulkan_observer_failure_preserves_resource_reuse",
        ),
        (
            "formula::device::tests::metal_profile_starts_fresh_formula_oracles",
            "formula::device::tests::vulkan_profile_starts_fresh_formula_oracles",
        ),
        (
            "formula::device::tests::metal_profiles_identify_exact_compilations",
            "formula::device::tests::vulkan_profiles_identify_exact_compilations",
        ),
        (
            "formula::device::tests::metal_profile_reuse_checks_context_lifecycle",
            "formula::device::tests::vulkan_profile_reuse_checks_context_lifecycle",
        ),
        (
            "metal_collection_refuses_a_foreign_context",
            "vulkan_collection_refuses_a_foreign_context",
        ),
        (
            "metal_resources_leave_a_cpu_collection_on_the_cpu",
            "vulkan_resources_leave_a_cpu_collection_on_the_cpu",
        ),
        (
            "formula_execution::tests::resource_tests::metal_formula_sessions_reuse_the_supplied_profile",
            "formula_execution::tests::resource_tests::vulkan_formula_sessions_reuse_the_supplied_profile",
        ),
        (
            "metal_formula_profiles_preserve_independent_sessions",
            "vulkan_formula_profiles_preserve_independent_sessions",
        ),
        (
            "physical::metal_families_retain_scored_observations",
            "physical::vulkan_families_retain_scored_observations",
        ),
        (
            "physical::metal_optimum_ties_retain_full_answers",
            "physical::vulkan_optimum_ties_retain_full_answers",
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
    metadata_with_cargo(version, physical, "cargo-llvm-cov 0.8.7")
}

fn metadata_with_cargo(
    version: &str,
    physical: bool,
    cargo_llvm_cov: &str,
) -> Result<serde_json::Value, zetesis_maintenance::Error> {
    metadata_over(version, physical.then_some(TABLE), cargo_llvm_cov)
}

fn metadata_over(
    version: &str,
    physical_table: Option<&str>,
    cargo_llvm_cov: &str,
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
        physical_table,
        observation: Observation {
            rustc: "rustc 1.97.1\nhost: fixture\nLLVM version: 22.1.6",
            cargo_llvm_cov,
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
    assert_eq!(record["committed_floor"], "91");
    assert_eq!(record["primary"], "workspace --all-features");
    assert_eq!(record["profiles_merged"], false);
    assert_eq!(
        record["supplemental"],
        "--package zetesis-cli --package zetesis-solve --no-default-features"
    );
    assert_eq!(
        record["floor_profiles"],
        serde_json::json!(["workspace", "cli-cpu"])
    );
    assert_eq!(record["expected_physical_tests"], 60);
    assert_eq!(record["physical_test_groups"].as_array().unwrap().len(), 16);
    assert_eq!(
        record["project_added_filename_filters"],
        serde_json::json!([])
    );
}

#[test]
fn physical_metadata_retains_the_reviewed_schedule() {
    let record = metadata("LLVM version 22.1.6", true).unwrap();
    let groups = record["physical_test_groups"].as_array().unwrap();
    let identities: Vec<_> = groups
        .iter()
        .map(|group| {
            serde_json::json!([
                group["group"],
                group["target_kind"],
                group["target"],
                group["expected_tests"]
            ])
        })
        .collect();
    assert_eq!(
        identities,
        serde_json::json!([
            ["wgpu-lib", "lib", "workspace libraries", 14],
            ["tight", "test", "integration", 4],
            ["formula", "test", "integration", 2],
            ["aggregate", "test", "integration", 3],
            ["lazy", "test", "integration", 4],
            ["cli-lazy", "test", "lazy_gpu", 5],
            ["cli-formula", "test", "formula_gpu", 3],
            ["world-views", "test", "integration", 4],
            ["aggregate-measurement", "test", "aggregate_measurement", 1],
            ["relation", "test", "integration", 2],
            ["relation-measurement", "test", "relation_measurement", 1],
            ["context", "test", "integration", 1],
            ["solve-context", "lib", "workspace libraries", 3],
            ["session-resources", "test", "integration", 9],
            ["language-consumers", "test", "integration", 2],
            ["static", "test", "integration", 2]
        ])
        .as_array()
        .unwrap()
        .as_slice()
    );
    let language_tests = serde_json::json!([
        "language_consumers::physical::metal_families_retain_scored_observations",
        "language_consumers::physical::metal_optimum_ties_retain_full_answers"
    ]);
    assert_eq!(groups[14]["tests"], language_tests);
    let tests = record["physical_tests"].as_array().unwrap();
    assert_eq!(tests.len(), 60);
    let formula_tests = serde_json::json!([
        "physical::ordinary_metal_formula_batches_match_complete_cpu_models_costs_and_displays",
        "physical::ordinary_metal_formula_limits_preserve_partial_coverage_and_writer_errors",
        "physical::ordinary_metal_table_joins_preserve_complete_answers"
    ]);
    assert_eq!(groups[6]["tests"], formula_tests);
    let session_tests = serde_json::json!([
        "session_resources_gpu::metal_resources_preserve_independent_sessions",
        "session_resources_gpu::metal_resource_policy_refusal_preserves_reuse",
        "session_resources_gpu::metal_resources_preserve_cpu_policies",
        "session_resources_gpu::metal_observer_failure_preserves_resource_reuse",
        "session_resources_gpu::metal_formula_profiles_preserve_independent_sessions",
        "session_resources_gpu::metal_tight_sessions_preserve_complete_families",
        "session_resources_gpu::metal_general_formulas_keep_device_execution",
        "session_resources_gpu::metal_tight_refusal_preserves_pending_coverage",
        "session_resources_gpu::metal_terminal_sessions_preserve_complete_families"
    ]);
    assert_eq!(groups[13]["tests"], session_tests);
    let static_tests = serde_json::json!([
        "hardware::metal_constructor_executes_resident_batches_without_fallback",
        "hardware::metal_static_oracle_matches_independent_closures"
    ]);
    assert_eq!(groups[15]["tests"], static_tests);
    let grouped_tests: Vec<_> = groups
        .iter()
        .flat_map(|group| group["tests"].as_array().unwrap().iter())
        .collect();
    assert_eq!(tests.iter().collect::<Vec<_>>(), grouped_tests);
    let scope = record["physical_scope"].as_str().unwrap();
    assert!(scope.starts_with("60 exact Metal tests: "));
    assert!(scope.contains("complete tight families, the general device route for non-tight theories, and tight work refusal before dispatch"));
    assert!(scope.contains("completed-support table joins with actual GPU candidates and complete CPU/Metal answer families"));
    assert!(scope.contains("static constructor and complete closure/reference checks"));
    assert!(
        scope.contains(
            "combined language-consumer families with scored observations and optimum ties"
        )
    );
    assert!(scope.ends_with("Unlisted tests and Vulkan are not selected."));
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
    let group = coverage::selection(TABLE).unwrap().groups.remove(0);
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
    let group = coverage::selection(TABLE).unwrap().groups.remove(0);
    let output = output(&group.tests, true).replacen(
        "adapter=fixture\nok",
        "adapter=fixture\ntest malformed boundary\nok",
        1,
    );
    assert!(coverage::physical_result(&output, &group).is_err());
}

#[test]
fn coverage_version_requires_a_current_observation() {
    for observation in [
        "",
        "0.8.7",
        "cargo-llvm-cov 0.9.1",
        "cargo-llvm-cov 0.8.7\nextra",
    ] {
        assert!(metadata_with_cargo("LLVM version 22.1.6", false, observation).is_err());
    }
    let record =
        metadata_with_cargo("LLVM version 22.1.6", false, "cargo-llvm-cov 0.8.7\n").unwrap();
    assert_eq!(record["cargo_llvm_cov"], "0.8.7");
    assert_eq!(record["cargo_llvm_cov_observation"], "cargo-llvm-cov 0.8.7");
}

#[test]
fn a_selection_names_the_backend_of_its_table() {
    assert_eq!(
        coverage::selection(TABLE).unwrap().api,
        zetesis_backend::GpuApi::Metal
    );
    assert_eq!(
        coverage::selection(VULKAN_TABLE).unwrap().api,
        zetesis_backend::GpuApi::Vulkan
    );
}

#[test]
fn coverage_metadata_refuses_the_vulkan_selection() {
    // The recorded coverage scope is the Metal qualification; a Vulkan
    // table would be recorded as Metal tests.
    assert!(
        metadata_over(
            "LLVM version 22.1.6",
            Some(VULKAN_TABLE),
            "cargo-llvm-cov 0.8.7"
        )
        .is_err()
    );
    assert!(metadata_over("LLVM version 22.1.6", Some(TABLE), "cargo-llvm-cov 0.8.7").is_ok());
}
