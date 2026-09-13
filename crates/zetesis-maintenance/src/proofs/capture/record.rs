//! Source preservation and construction of a validated current record.
use super::execution::{Kernel, Runner, Tools};
use super::{Result, bytes, copy_flat, digest, json_file, require};
use crate::proofs;
use regex::Regex;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
use zetesis_validation::process::Limits;

pub(super) fn source_preflight(proof_root: &Path) -> Result<proofs::Inventory> {
    let before = proofs::inventory(proof_root, proofs::Limits::default())?;
    require(
        bytes(&proof_root.join("lean-toolchain"))? == b"leanprover/lean4:v4.33.1\n",
        "capture requires the declared Lean 4.33.1 toolchain",
    )?;
    require(
        before.theorem_index()? == bytes(&proof_root.join("theorems.json"))?,
        "rendered theorem index differs",
    )?;
    require(
        before.audit_source().as_bytes() == bytes(&proof_root.join("Audit.lean"))?,
        "rendered Audit source differs",
    )?;
    let manifest: Value = serde_json::from_slice(&bytes(&proof_root.join("lake-manifest.json"))?)?;
    require(
        manifest["packages"] == json!([]),
        "unexpected external Lean packages",
    )?;

    Ok(before)
}

pub(super) fn prepare_record_storage(
    repository: PathBuf,
    work: &Path,
    proof_root: &Path,
    environment: BTreeMap<String, OsString>,
    command_limits: Limits,
) -> Result<(PathBuf, Runner)> {
    let archive = work.join("before");
    fs::create_dir(&archive)?;
    fs::write(
        archive.join("verification.json"),
        bytes(&proof_root.join("verification.json"))?,
    )?;
    fs::write(
        archive.join("axiom-audit.txt"),
        bytes(&proof_root.join("axiom-audit.txt"))?,
    )?;
    copy_flat(
        &proof_root.join("verification/current"),
        &archive.join("current"),
    )?;
    let stage = work.join("stage");
    fs::create_dir(&stage)?;
    fs::create_dir_all(stage.join("verification/current"))?;
    let evidence = work.join("commands");
    fs::create_dir(&evidence)?;
    let runner = Runner {
        repository,
        stage,
        evidence,
        commands: Vec::new(),
        logs: BTreeMap::new(),
        environment,
        command_limits,
    };

    Ok((archive, runner))
}

pub(super) fn retain_current_sources(
    runner: &mut Runner,
    proof_root: &Path,
    before: &proofs::Inventory,
) -> Result<proofs::Inventory> {
    let after = proofs::inventory(proof_root, proofs::Limits::default())?;
    require(
        before.source_sha256() == after.source_sha256(),
        "proof sources changed during execution",
    )?;
    let inventory_path = runner.stage.join("verification/current/inventory.json");
    json_file(&inventory_path, &serde_json::to_value(&after)?)?;
    runner.logs.insert(
        "verification/current/inventory.json".into(),
        digest(&inventory_path)?,
    );
    for name in after.source_sha256().keys() {
        let destination = runner.stage.join(name);
        fs::create_dir_all(destination.parent().ok_or("source path has no parent")?)?;
        fs::write(destination, bytes(&proof_root.join(name))?)?;
    }
    for name in ["README.md", "theorems.json"] {
        fs::write(runner.stage.join(name), bytes(&proof_root.join(name))?)?;
    }
    Ok(after)
}

pub(super) fn construct_record(
    runner: &Runner,
    after: &proofs::Inventory,
    kernel: &Kernel,
    tools: &Tools,
    maintenance: &Path,
) -> Result<Value> {
    let artifacts: BTreeMap<String, String> = ["README.md", "theorems.json", "axiom-audit.txt"]
        .into_iter()
        .map(|name| Ok((name.into(), digest(&runner.stage.join(name))?)))
        .collect::<Result<_>>()?;
    let audit_text = std::str::from_utf8(&kernel.audit)?;
    let axiom_brackets = Regex::new(r"\[([^\]]*)\]")?;
    let axioms: BTreeSet<&str> = axiom_brackets
        .captures_iter(audit_text)
        .flat_map(|capture| capture.get(1).unwrap().as_str().split(',').map(str::trim))
        .filter(|value| !value.is_empty())
        .collect();
    require(
        axioms
            .iter()
            .all(|name| ["Classical.choice", "Quot.sound", "propext"].contains(name)),
        "unexpected transitive axiom",
    )?;
    let count = after.declarations().len();
    let record = json!({
        "schema_version": 1,
        "project": "zetesis",
        "status": "PASS",
        "lean_version": kernel.identity.version,
        "lean_commit": kernel.identity.commit,
        "platform": kernel.identity.platform,
        "external_packages": [],
        "semantic_modules": after.modules().len(),
        "theorems_audited": count,
        "path_convention": {
            "command_cwd": "Paths are relative to the repository root; proofs denotes its Lean package directory.",
            "record_members": "Source, generated-artifact and log paths are relative to proofs/.",
            "toolchain": "Command executable names denote the explicitly selected pinned tools; executable hashes are recorded separately.",
            "capture": "Each command declares its log layout. stdout_then_stderr retains raw stdout followed by raw stderr. json_stdout_and_stderr stores those exact UTF-8 streams reversibly, with the raw concatenation digest. Separate byte lengths are recorded. Unix timestamps are seconds since 1970-01-01 UTC."
        },
        "tool_sha256": tools.hashes(maintenance)?,
        "rust_toolchain": "1.97.1",
        "assurance_tool": concat!("zetesis-maintenance ", env!("CARGO_PKG_VERSION"), ": retained record consistency; not Lean kernel execution"),
        "command_environment": { "CARGO_BUILD_JOBS": "1", "CARGO_NET_OFFLINE": "true", "RUSTUP_TOOLCHAIN": "1.97.1" },
        "capture_limits": { "polling_deadline_seconds": runner.command_limits.timeout.as_secs_f64(), "retained_output_bytes": runner.command_limits.max_output_bytes, "cleanup_seconds": runner.command_limits.cleanup_timeout.as_secs_f64(), "cleanup_retry_seconds": 2 },
        "commands": runner.commands,
        "transitive_axioms": axioms,
        "project_axioms": [],
        "proof_holes": false,
        "native_evaluation_proof_shortcuts": false,
        "source_sha256": after.source_sha256(),
        "generated_artifact_sha256": artifacts,
        "verification_log_sha256": runner.logs,
        "inventory_observation": {
            "producer": "zetesis_maintenance::proofs::inventory",
            "path": "verification/current/inventory.json",
            "scope": "Observed declarations and source hashes; not a kernel verdict"
        },
        "audit_consistency": {
            "source_theorem_declarations": count,
            "indexed_theorems": count,
            "printed_axiom_entries": count,
            "unique_qualified_names": count,
            "source_index_line_check": "PASS",
            "allowed_transitive_axioms_only": "PASS"
        },
        "refinement_boundary": "Kernel-checked mathematical definitions and laws. Concrete parsing, source lowering, grounding, Rust representations, resource behavior, scheduling and WGSL execution remain separate unproved implementation correspondences. Rust record checks establish retained evidence consistency; they do not replace Lean kernel execution."
    });
    Ok(record)
}

pub(super) fn check_staged_record(runner: &Runner, record: &Value, count: usize) -> Result<()> {
    json_file(&runner.stage.join("verification.json"), record)?;
    let summary = proofs::verify(
        &runner.stage,
        "verification.json",
        proofs::Limits::default(),
    )?;
    require(summary.theorems == count, "stage consistency count differs")?;

    Ok(())
}
