//! Independent adversarial records, never invoking Lean or a solver.
#[path = "support/proof_fixture.rs"]
mod fixture;
use fixture::Fixture;
use serde_json::{Value, json};
use std::{fmt::Write as _, fs, process::Command};
use zetesis_maintenance::proofs::{self, Limits, Summary};

#[test]
fn supported_declarations_have_exact_locations() {
    assert_eq!(
        Fixture::new().verify().unwrap(),
        Summary {
            theorems: 2,
            semantic_modules: 2,
            source_files: 7
        }
    );
}
#[test]
fn verification_does_not_write_record_files() {
    let f = Fixture::new();
    let before = f.read("verification.json");
    f.verify().unwrap();
    assert_eq!(f.read("verification.json"), before);
    // Every recorded source/artifact hash is checked again after the read-only operation.
    f.verify().unwrap();
}
#[test]
fn artifact_hashes_detect_staleness() {
    for file in ["README.md", "theorems.json", "axiom-audit.txt"] {
        let f = Fixture::new();
        f.write(file, &(f.read(file) + "\n"));
        f.reject();
    }
}
#[test]
fn source_inventory_is_exact() {
    for mutation in [
        "changed",
        "extra",
        "missing",
        "record_extra",
        "record_missing",
    ] {
        let mut f = Fixture::new();
        match mutation {
            "changed" => f.write("Zetesis/A.lean", "namespace Zetesis\nend Zetesis\n"),
            "extra" => f.write("Zetesis/Extra.lean", "namespace Extra\nend Extra\n"),
            "missing" => fs::remove_file(f.root().join("Zetesis/A.lean")).unwrap(),
            "record_extra" => f.record["source_sha256"]["README.md"] = f.digest("README.md").into(),
            _ => {
                f.record["source_sha256"]
                    .as_object_mut()
                    .unwrap()
                    .remove("Zetesis/A.lean");
            }
        }
        f.save();
        f.reject();
    }
}
#[test]
fn reported_counts_equal_derived_counts() {
    for key in [
        "theorems_audited",
        "semantic_modules",
        "source_theorem_declarations",
        "indexed_theorems",
        "printed_axiom_entries",
        "unique_qualified_names",
    ] {
        for value in [json!(1), json!(true)] {
            let mut f = Fixture::new();
            if matches!(key, "theorems_audited" | "semantic_modules") {
                f.record[key] = value;
            } else {
                f.record["audit_consistency"][key] = value;
            }
            f.save();
            f.reject();
        }
    }
}
#[test]
fn optional_module_count_is_derived() {
    let mut f = Fixture::new();
    fs::rename(
        f.root().join("Zetesis/Sub/B.lean"),
        f.root().join("Zetesis/BatchAccounting.lean"),
    )
    .unwrap();
    let old = f.record["source_sha256"]
        .as_object_mut()
        .unwrap()
        .remove("Zetesis/Sub/B.lean")
        .unwrap();
    f.record["source_sha256"]["Zetesis/BatchAccounting.lean"] = old;
    f.entries[1]["file"] = "Zetesis/BatchAccounting.lean".into();
    f.write("theorems.json", &f.entries.to_string());
    f.record["batch_accounting_laws"] = 1.into();
    f.refresh();
    f.verify().unwrap();
    f.record["batch_accounting_laws"] = 2.into();
    f.save();
    f.reject();
}
#[test]
fn rehashing_cannot_launder_index_changes() {
    for mutation in ["missing", "duplicate", "name", "file", "line", "bool_line"] {
        let mut f = Fixture::new();
        match mutation {
            "missing" => {
                f.entries.as_array_mut().unwrap().pop();
            }
            "duplicate" => {
                let row = f.entries[0].clone();
                f.entries.as_array_mut().unwrap().push(row);
            }
            "name" => f.entries[0]["name"] = "Zetesis.wrong".into(),
            "file" => f.entries[0]["file"] = "Zetesis/Sub/B.lean".into(),
            "line" => f.entries[0]["line"] = 9.into(),
            _ => f.entries[0]["line"] = true.into(),
        }
        f.write("theorems.json", &f.entries.to_string());
        f.refresh();
        f.reject();
    }
}
#[test]
fn audit_requests_cover_every_unique_theorem() {
    for names in [
        vec!["Zetesis.first"],
        vec!["Zetesis.first", "Zetesis.first"],
        vec!["Zetesis.first", "Zetesis.unknown"],
    ] {
        let mut f = Fixture::new();
        let mut source = String::from("import Zetesis\n");
        for name in names {
            writeln!(source, "#print axioms {name}").unwrap();
        }
        f.write("Audit.lean", &source);
        f.refresh();
        f.reject();
    }
}
#[test]
fn audit_output_exactly_follows_requests() {
    let first = "'Zetesis.first' does not depend on any axioms\n";
    let second = "'Zetesis.Sub.second'' does not depend on any axioms\n";
    let original = Fixture::new().read("axiom-audit.txt");
    for output in [
        first.into(),
        first.to_owned() + first,
        original.clone() + first,
        second.to_owned() + first,
        original.clone() + "warning: ignored\n",
        "unknown\n".to_owned() + &original,
    ] {
        let mut f = Fixture::new();
        f.write("axiom-audit.txt", &output);
        f.write("verification/current/audit.log", &output);
        f.refresh();
        f.reject();
    }
}
#[test]
fn rehashing_cannot_admit_unapproved_axioms() {
    for axiom in [
        "sorryAx",
        "Lean.ofReduceBool",
        "Zetesis.custom",
        "propext, propext",
        "propext,",
    ] {
        let mut f = Fixture::new();
        let output = format!(
            "'Zetesis.first' depends on axioms: [{axiom}]\n'Zetesis.Sub.second'' does not depend on any axioms\n"
        );
        f.write("axiom-audit.txt", &output);
        f.write("verification/current/audit.log", &output);
        f.refresh();
        f.reject();
    }
}
#[test]
fn commands_require_current_successful_evidence() {
    for mutation in [
        "old_logs_only",
        "failed",
        "nonzero",
        "missing_log",
        "different_audit",
        "duplicate_log",
        "negative_time",
        "audit_flags",
        "no_build",
        "empty_command",
        "no_cwd",
    ] {
        let mut f = Fixture::new();
        match mutation {
            "old_logs_only" => {
                f.write("verification/old/audit.log", &f.read("axiom-audit.txt"));
                f.record["verification_log_sha256"] =
                    json!({"verification/old/audit.log":f.digest("verification/old/audit.log")});
            }
            "failed" => f.record["commands"][0]["result"] = "FAIL".into(),
            "nonzero" => f.record["commands"][0]["exit_code"] = 1.into(),
            "missing_log" => {
                fs::remove_file(f.root().join("verification/current/build.log")).unwrap();
            }
            "different_audit" => {
                f.write("verification/current/audit.log", "different but hashed\n");
                f.refresh();
            }
            "duplicate_log" => {
                let row = f.record["commands"][0].clone();
                f.record["commands"].as_array_mut().unwrap().push(row);
            }
            "negative_time" => f.record["commands"][0]["elapsed_seconds"] = (-1).into(),
            "audit_flags" => {
                f.record["commands"][1]["command"] = json!(["lake", "env", "lean", "Audit.lean"]);
            }
            "no_build" => {
                f.record["commands"].as_array_mut().unwrap().remove(0);
            }
            "empty_command" => f.record["commands"][0]["command"] = json!([]),
            _ => f.record["commands"][0]["cwd"] = "".into(),
        }
        f.save();
        f.reject();
    }
}
#[test]
fn nonfinite_command_time_is_refused() {
    let f = Fixture::new();
    f.write(
        "verification.json",
        &f.record.to_string().replace("0.1", "NaN"),
    );
    f.reject();
}
#[test]
fn historical_logs_remain_part_of_hash_validation() {
    let mut f = Fixture::new();
    f.write("verification/old/build.log", "historical\n");
    f.record["verification_log_sha256"]["verification/old/build.log"] =
        f.digest("verification/old/build.log").into();
    f.save();
    f.verify().unwrap();
    f.write("verification/old/build.log", "changed\n");
    f.reject();
}
#[test]
fn unsupported_declaration_conventions_fail_closed() {
    for source in [
        "namespace Zetesis\nprivate theorem hidden : True := by trivial\nend Zetesis\n",
        "namespace Zetesis\n@[simp] theorem hidden : True := by trivial\nend Zetesis\n",
        "namespace Zetesis\n@[simp]\ntheorem hidden : True := by trivial\nend Zetesis\n",
        "namespace Zetesis\ntheorem «hidden» : True := by trivial\nend Zetesis\n",
        "namespace Zetesis\nmacro \"hidden\" : command => pure default\nend Zetesis\n",
        "namespace Zetesis\nend Other\n",
        "/- unfinished",
        "def a := \"unfinished",
        "theorem escaped : True := by trivial\n",
        "namespace Zetesis\n theorem indented : True := by trivial\nend Zetesis\n",
        "namespace Zetesis\naxiom trusted : False\nend Zetesis\n",
    ] {
        let mut f = Fixture::new();
        f.write("Zetesis/A.lean", source);
        f.refresh();
        f.reject();
    }
}
#[test]
fn duplicate_json_keys_are_refused_recursively() {
    for text in [
        "{\"status\":\"PASS\",\"status\":\"FAIL\"}",
        "{\"nested\":{\"key\":1,\"key\":2}}",
    ] {
        let f = Fixture::new();
        f.write("verification.json", text);
        f.reject();
    }
}
#[test]
fn schema_version_is_an_integer() {
    let mut f = Fixture::new();
    f.record["schema_version"] = true.into();
    f.save();
    f.reject();
}
#[test]
fn recorded_paths_are_confined() {
    for path in [
        "../elsewhere",
        "/elsewhere",
        "verification//current/build.log",
        "verification/./current/build.log",
    ] {
        let mut f = Fixture::new();
        f.record["verification_log_sha256"][path] = "0".repeat(64).into();
        f.save();
        f.reject();
    }
}
#[test]
fn proof_claims_cannot_enable_shortcuts() {
    for key in [
        "proof_holes",
        "native_evaluation_proof_shortcuts",
        "project_axioms",
    ] {
        let mut f = Fixture::new();
        f.record[key] = if key == "project_axioms" {
            json!(["axiom"])
        } else {
            Value::Bool(true)
        };
        f.save();
        f.reject();
    }
}
#[test]
fn resource_limits_are_enforced() {
    for limits in [
        Limits {
            file_bytes: 1,
            ..Limits::default()
        },
        Limits {
            total_bytes: 1,
            ..Limits::default()
        },
        Limits {
            entries: 0,
            ..Limits::default()
        },
    ] {
        let f = Fixture::new();
        assert!(proofs::verify(f.root(), "verification.json", limits).is_err());
    }
}
#[test]
fn proof_cli_prints_derived_counts() {
    let f = Fixture::new();
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis-maintenance"))
        .args(["proof-record", "--proofs-dir"])
        .arg(f.root())
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("2 theorems; 2 semantic modules; 7")
    );
}
#[test]
fn proof_cli_cannot_certify_a_stale_alternative_record() {
    let mut f = Fixture::new();
    f.record["audit_consistency"]["indexed_theorems"] = 1.into();
    f.write("old.json", &f.record.to_string());
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis-maintenance"))
        .args(["proof-record", "--record", "old.json", "--proofs-dir"])
        .arg(f.root())
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(!String::from_utf8(result.stdout).unwrap().contains("PASS"));
}

#[cfg(unix)]
#[test]
fn semantic_inventory_refuses_symlinked_subtrees() {
    let f = Fixture::new();
    let elsewhere = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), f.root().join("Zetesis/Hidden")).unwrap();
    f.reject();
}
#[cfg(unix)]
#[test]
fn semantic_inventory_refuses_a_symlinked_root() {
    let f = Fixture::new();
    fs::rename(f.root().join("Zetesis"), f.root().join("Elsewhere")).unwrap();
    std::os::unix::fs::symlink(f.root().join("Elsewhere"), f.root().join("Zetesis")).unwrap();
    f.reject();
}
#[cfg(unix)]
#[test]
fn recorded_file_symlinks_cannot_escape_the_root() {
    let f = Fixture::new();
    let elsewhere = tempfile::tempdir().unwrap();
    let file = elsewhere.path().join("build.log");
    fs::write(&file, f.read("verification/current/build.log")).unwrap();
    fs::remove_file(f.root().join("verification/current/build.log")).unwrap();
    std::os::unix::fs::symlink(file, f.root().join("verification/current/build.log")).unwrap();
    f.reject();
}

#[test]
fn proof_inventory_renders_the_observed_declarations() {
    let f = Fixture::new();
    let inventory = proofs::inventory(f.root(), Limits::default()).unwrap();
    assert_eq!(
        inventory
            .modules()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Zetesis/A.lean", "Zetesis/Sub/B.lean"]
    );
    assert_eq!(
        inventory
            .declarations()
            .iter()
            .map(|declaration| (declaration.name(), declaration.file(), declaration.line()))
            .collect::<Vec<_>>(),
        [
            ("Zetesis.first", "Zetesis/A.lean", 8),
            ("Zetesis.Sub.second'", "Zetesis/Sub/B.lean", 2)
        ]
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&inventory.theorem_index().unwrap()).unwrap(),
        f.entries
    );
    assert_eq!(inventory.audit_source(), f.read("Audit.lean"));
}
#[test]
fn proof_inventory_hashes_observed_source_bytes() {
    let f = Fixture::new();
    let inventory = proofs::inventory(f.root(), Limits::default()).unwrap();
    assert_eq!(
        serde_json::to_value(inventory.source_sha256()).unwrap(),
        f.record["source_sha256"]
    );
}
#[test]
fn inventory_views_make_no_success_claim() {
    let f = Fixture::new();
    let inventory = proofs::inventory(f.root(), Limits::default()).unwrap();
    let value = serde_json::to_value(&inventory).unwrap();
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["declarations", "modules", "source_sha256"]
    );
}
#[test]
fn inventory_publication_remains_the_callers_operation() {
    let f = Fixture::new();
    let before = f.read("theorems.json");
    let audit = f.read("Audit.lean");
    let inventory = proofs::inventory(f.root(), Limits::default()).unwrap();
    let _views = (inventory.theorem_index().unwrap(), inventory.audit_source());
    assert_eq!(f.read("theorems.json"), before);
    assert_eq!(f.read("Audit.lean"), audit);
}
#[test]
fn proof_inventory_enforces_the_source_convention() {
    let f = Fixture::new();
    f.write(
        "Zetesis/A.lean",
        "namespace Zetesis\naxiom hidden : False\nend Zetesis\n",
    );
    assert!(proofs::inventory(f.root(), Limits::default()).is_err());
}
#[test]
fn proof_inventory_rejects_duplicate_qualified_names() {
    let f = Fixture::new();
    f.write(
        "Zetesis/Sub/B.lean",
        "namespace Zetesis\ntheorem first : True := by trivial\nend Zetesis\n",
    );
    assert!(proofs::inventory(f.root(), Limits::default()).is_err());
}
#[test]
fn proof_inventory_applies_read_limits() {
    let f = Fixture::new();
    assert!(
        proofs::inventory(
            f.root(),
            Limits {
                file_bytes: 0,
                ..Limits::default()
            }
        )
        .is_err()
    );
}

#[test]
fn inventory_cli_renders_each_requested_view() {
    let f = Fixture::new();
    let inventory = proofs::inventory(f.root(), Limits::default()).unwrap();
    for (view, expected) in [
        ("index", inventory.theorem_index().unwrap()),
        ("audit", inventory.audit_source().into_bytes()),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_zetesis-maintenance"))
            .args(["proof-inventory", "--view", view, "--proofs-dir"])
            .arg(f.root())
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, expected);
    }
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis-maintenance"))
        .args(["proof-inventory", "--proofs-dir"])
        .arg(f.root())
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap(),
        serde_json::to_value(&inventory).unwrap()
    );
}
