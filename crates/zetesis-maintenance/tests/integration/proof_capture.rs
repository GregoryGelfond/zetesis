//! Synthetic execution/publication controls, never evidence of Lean acceptance.
#![cfg(all(
    feature = "test-fixtures",
    any(target_os = "linux", target_os = "macos")
))]
use crate::support::process as subprocess;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use subprocess::{Command, Output};
use zetesis_maintenance::proofs::{
    self,
    capture::{self, Phase, Request},
};

struct Fixture {
    directory: Option<tempfile::TempDir>,
    repository: PathBuf,
    evidence: PathBuf,
    tools: PathBuf,
}
impl Fixture {
    fn new(mode: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let base = directory.path().canonicalize().unwrap();
        let repository = base.join("repository");
        let evidence = base.join("evidence");
        let tools = base.join("tools");
        for path in [&repository, &evidence, &tools] {
            fs::create_dir(path).unwrap();
        }
        let proof_root = repository.join("proofs");
        fs::create_dir_all(proof_root.join("Zetesis")).unwrap();
        fs::create_dir_all(proof_root.join("verification/current")).unwrap();
        for (name, content) in [
            (
                "Zetesis/Example.lean",
                "namespace Example\ntheorem fact : True := by trivial\nend Example\n",
            ),
            ("Zetesis.lean", "import Zetesis.Example\n"),
            ("Audit.lean", "import Zetesis\n"),
            ("lakefile.lean", "import Lake\n"),
            ("lean-toolchain", "leanprover/lean4:v4.33.1\n"),
            ("lake-manifest.json", "{\"packages\":[]}\n"),
            ("README.md", "Synthetic capture; no kernel was run.\n"),
            ("theorems.json", "[]"),
            ("verification.json", "previous record"),
            ("axiom-audit.txt", "previous audit"),
            ("verification/current/build.log", "previous log"),
        ] {
            fs::write(proof_root.join(name), content).unwrap();
        }
        let inventory = proofs::inventory(&proof_root, proofs::Limits::default()).unwrap();
        fs::write(
            proof_root.join("theorems.json"),
            inventory.theorem_index().unwrap(),
        )
        .unwrap();
        fs::write(proof_root.join("Audit.lean"), inventory.audit_source()).unwrap();
        fs::write(repository.join(".capture-fixture"), mode).unwrap();
        publish_tools(&tools);
        Self {
            directory: Some(directory),
            repository,
            evidence,
            tools,
        }
    }
    fn capture(&self) -> Result<proofs::Summary, capture::Failure> {
        capture::capture(Request {
            repository: &self.repository,
            evidence: &self.evidence,
            lean_bin: &self.tools,
            rust_bin: &self.tools,
            maintenance: &self.tools.join("zetesis-maintenance"),
            command_limits: zetesis_validation::process::Limits {
                timeout: Duration::from_secs(5),
                max_output_bytes: 1024 * 1024,
                cleanup_timeout: Duration::from_secs(1),
            },
        })
    }
    fn cli(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zetesis-maintenance"))
            .arg("proof-capture")
            .arg("--repository")
            .arg(&self.repository)
            .arg("--evidence")
            .arg(&self.evidence)
            .arg("--lean-bin")
            .arg(&self.tools)
            .arg("--rust-bin")
            .arg(&self.tools)
            .current_dir(&self.repository)
            .bounded_output()
    }
    fn original_remains(&self) {
        assert_eq!(
            fs::read(self.repository.join("proofs/verification.json")).unwrap(),
            b"previous record"
        );
        assert_eq!(
            fs::read(self.evidence.join("before/verification.json")).unwrap(),
            b"previous record"
        );
        assert!(
            !self
                .repository
                .join("proofs/verification/.capture-lock")
                .exists()
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(directory) = self.directory.take()
        {
            eprintln!(
                "Proof capture fixture retained at {}",
                directory.keep().display()
            );
        }
    }
}

fn publish_tools(tools: &Path) {
    // Every case owns one completed, nonwritable executable inode. Role names
    // may share that owner, but never the mutable Cargo artifact. Publish only
    // after the private copy is closed and checked; do not rewrite executables.
    let source = Path::new(env!("CARGO_BIN_EXE_zetesis-maintenance-fixture"));
    let admitted = fs::read(source).unwrap();
    let staging = tools.join(".fixture-pending");
    fs::copy(source, &staging).unwrap();
    assert_eq!(fs::read(&staging).unwrap(), admitted);
    let mut permissions = fs::metadata(&staging).unwrap().permissions();
    permissions.set_mode(permissions.mode() & !0o222);
    fs::set_permissions(&staging, permissions).unwrap();
    let owner = tools.join("fixture-owner");
    fs::rename(staging, &owner).unwrap();
    for name in ["lake", "lean", "rustc", "cargo", "zetesis-maintenance"] {
        fs::hard_link(&owner, tools.join(name)).unwrap();
    }
}

#[test]
fn tool_roles_share_only_the_private_executable_owner() {
    let fixture = Fixture::new("");
    let original = Path::new(env!("CARGO_BIN_EXE_zetesis-maintenance-fixture"));
    let original_metadata = fs::metadata(original).unwrap();
    let owner = fixture.tools.join("fixture-owner");
    let metadata = fs::metadata(&owner).unwrap();
    assert_ne!(
        (metadata.dev(), metadata.ino()),
        (original_metadata.dev(), original_metadata.ino())
    );
    assert_eq!(fs::read(owner).unwrap(), fs::read(original).unwrap());
    assert_eq!(metadata.permissions().mode() & 0o222, 0);
    assert_ne!(metadata.permissions().mode() & 0o111, 0);
    assert!(!fixture.tools.join(".fixture-pending").exists());
    for name in ["lake", "lean", "rustc", "cargo", "zetesis-maintenance"] {
        let role = fs::metadata(fixture.tools.join(name)).unwrap();
        assert_eq!((role.dev(), role.ino()), (metadata.dev(), metadata.ino()));
    }
}

#[test]
fn cli_publishes_the_record_it_summarizes() {
    let fixture = Fixture::new("");
    let output = fixture.cli();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        format!(
            "Proof capture: PASS: 1 theorems; 1 semantic modules; evidence {}\n",
            fixture.evidence.display()
        )
        .as_bytes()
    );
    // The actual CLI freezes itself; the Lean/Cargo responses remain synthetic.
    let checker = fs::canonicalize(env!("CARGO_BIN_EXE_zetesis-maintenance")).unwrap();
    let admitted = fs::read(&checker).unwrap();
    let frozen = fixture.evidence.join("tools/zetesis-maintenance");
    assert_eq!(fs::read(&frozen).unwrap(), admitted);
    let identity: Value =
        serde_json::from_slice(&fs::read(fixture.evidence.join("tools/identity.json")).unwrap())
            .unwrap();
    assert_eq!(identity["original_path"], checker.to_str().unwrap());
    assert_eq!(identity["frozen_path"], frozen.to_str().unwrap());
    let digest = format!("{:x}", Sha256::digest(&admitted));
    assert_eq!(identity["original_sha256"], digest);
    assert_eq!(identity["frozen_sha256"], digest);
    let record: Value = serde_json::from_slice(
        &fs::read(fixture.repository.join("proofs/verification.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(record["platform"], "fixture-host");
    assert_eq!(record["tool_sha256"]["zetesis-maintenance"], digest);
    let summary = proofs::verify(
        &fixture.repository.join("proofs"),
        "verification.json",
        proofs::Limits::default(),
    )
    .unwrap();
    assert_eq!(summary.semantic_modules, 1);
    assert_eq!(summary.theorems, 1);
    assert_eq!(summary.source_files, 6);
}

#[test]
fn cli_version_refusal_preserves_the_prior_record() {
    let fixture = Fixture::new("lean-version");
    let output = fixture.cli();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"Maintenance: FAIL: proof capture Kernel: wrong pinned Lean identity\n"
    );
    fixture.original_remains();
    assert_eq!(
        fs::read(fixture.repository.join("proofs/axiom-audit.txt")).unwrap(),
        b"previous audit"
    );
    assert_eq!(
        fs::read(
            fixture
                .repository
                .join("proofs/verification/current/build.log")
        )
        .unwrap(),
        b"previous log"
    );
    assert!(!fixture.evidence.join("commands/build").exists());
    let observed = fs::read_to_string(fixture.evidence.join("commands/version/stdout")).unwrap();
    assert!(observed.starts_with("Lean (version 4.32.0, fixture-host,"));
}

#[test]
fn capture_derives_the_current_population_and_identity() {
    let fixture = Fixture::new("");
    let summary = fixture.capture().unwrap();
    assert_eq!(summary.semantic_modules, 1);
    assert_eq!(summary.theorems, 1);
    assert_eq!(summary.source_files, 6);
    let record: Value = serde_json::from_slice(
        &fs::read(fixture.repository.join("proofs/verification.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(record["platform"], "fixture-host");
    assert_eq!(
        record["lean_commit"],
        "0123456789012345678901234567890123456789"
    );
    assert_eq!(
        fs::read(fixture.evidence.join("before/verification.json")).unwrap(),
        b"previous record"
    );
    let receipt: Value = serde_json::from_slice(
        &fs::read(fixture.evidence.join("commands/audit/invocation.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["executable"], "/usr/bin/env");
    assert_eq!(receipt["arguments"][0], "-i");
    assert!(
        receipt["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg.as_str() == Some(fixture.tools.join("lake").to_str().unwrap()))
    );
    assert_eq!(
        fs::read(fixture.evidence.join("commands/audit/stdout")).unwrap(),
        b"'Example.fact' does not depend on any axioms\n"
    );
}

#[test]
fn a_failed_build_preserves_the_original_record() {
    let fixture = Fixture::new("build");
    assert_eq!(fixture.capture().unwrap_err().phase(), Phase::Kernel);
    fixture.original_remains();
    let receipt: Value = serde_json::from_slice(
        &fs::read(fixture.evidence.join("commands/build/result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["stop"], "completed");
    assert_eq!(receipt["exit"]["code"], 37);
    assert!(!fixture.evidence.join("commands/audit").exists());
}

#[test]
fn changed_sources_cannot_publish_the_record() {
    let fixture = Fixture::new("source-change");
    assert_eq!(fixture.capture().unwrap_err().phase(), Phase::Validation);
    fixture.original_remains();
}

#[test]
fn changed_tool_bytes_cannot_publish_the_record() {
    let fixture = Fixture::new("tool-change");
    let original = fs::read(fixture.tools.join("lean")).unwrap();
    let failure = fixture.capture().unwrap_err();
    assert_eq!(failure.phase(), Phase::Validation);
    assert!(
        failure
            .to_string()
            .contains("capture tools changed during execution")
    );
    fixture.original_remains();
    assert_eq!(
        fs::read(fixture.tools.join("lean")).unwrap(),
        b"changed Lean executable input"
    );
    assert_eq!(
        fs::read(fixture.tools.join("fixture-owner")).unwrap(),
        original
    );
    let inputs: Value =
        serde_json::from_slice(&fs::read(fixture.evidence.join("inputs.json")).unwrap()).unwrap();
    assert_eq!(
        inputs["tool_sha256"]["lean"],
        format!("{:x}", Sha256::digest(original))
    );
    assert!(!fixture.evidence.join("stage/verification.json").exists());
}

#[test]
fn changed_documentation_cannot_publish_the_record() {
    let fixture = Fixture::new("documentation-change");
    let original = fs::read(fixture.repository.join("proofs/README.md")).unwrap();
    let failure = fixture.capture().unwrap_err();
    assert_eq!(failure.phase(), Phase::Validation);
    assert!(
        failure
            .to_string()
            .contains("proof documentation/views changed during capture")
    );
    fixture.original_remains();
    let inputs: Value =
        serde_json::from_slice(&fs::read(fixture.evidence.join("inputs.json")).unwrap()).unwrap();
    assert_eq!(
        inputs["document_sha256"]["README.md"],
        format!("{:x}", Sha256::digest(original))
    );
    assert_eq!(
        fs::read(fixture.repository.join("proofs/README.md")).unwrap(),
        b"changed during capture\n"
    );
    assert!(!fixture.evidence.join("stage/verification.json").exists());
}

#[test]
fn refused_command_start_retains_its_boundary_receipt() {
    let fixture = Fixture::new("");
    let failure = capture::capture(Request {
        repository: &fixture.repository,
        evidence: &fixture.evidence,
        lean_bin: &fixture.tools,
        rust_bin: &fixture.tools,
        maintenance: &fixture.tools.join("zetesis-maintenance"),
        command_limits: zetesis_validation::process::Limits {
            timeout: Duration::MAX,
            ..zetesis_validation::process::Limits::default()
        },
    })
    .unwrap_err();
    assert_eq!(failure.phase(), Phase::Kernel);
    let start = std::error::Error::source(&failure)
        .unwrap()
        .downcast_ref::<zetesis_validation::process::StartError>()
        .unwrap();
    assert!(matches!(
        start,
        zetesis_validation::process::StartError::DeadlineOverflow
    ));
    fixture.original_remains();
    let command = fixture.evidence.join("commands/version");
    let receipt: Value =
        serde_json::from_slice(&fs::read(command.join("result.json")).unwrap()).unwrap();
    assert_eq!(receipt["start_error"], start.to_string());
    assert!(receipt.get("exit").is_none());
    assert!(command.join("invocation.json").is_file());
    assert!(!command.join("stdout").exists());
    assert!(!fixture.evidence.join("commands/build").exists());
}

#[test]
fn successful_audit_stderr_cannot_publish_the_record() {
    let fixture = Fixture::new("audit-stderr");
    assert_eq!(fixture.capture().unwrap_err().phase(), Phase::Kernel);
    fixture.original_remains();
    assert_eq!(
        fs::read(fixture.evidence.join("commands/audit/stderr")).unwrap(),
        b"synthetic audit warning\n"
    );
}

#[test]
fn wrong_observed_versions_cannot_publish_the_record() {
    for (mode, phase) in [
        ("lean-version", Phase::Kernel),
        ("rust-version", Phase::Assurance),
        ("cargo-version", Phase::Assurance),
    ] {
        let fixture = Fixture::new(mode);
        assert_eq!(fixture.capture().unwrap_err().phase(), phase, "{mode}");
        fixture.original_remains();
    }
}

#[test]
fn repository_overlap_is_refused_before_tool_execution() {
    let mut fixture = Fixture::new("");
    fixture.evidence = fixture.repository.join("evidence");
    fs::create_dir(&fixture.evidence).unwrap();
    assert_eq!(fixture.capture().unwrap_err().phase(), Phase::Preflight);
    assert!(fs::read_dir(&fixture.evidence).unwrap().next().is_none());
}

#[test]
fn stale_generated_views_are_refused_before_execution() {
    let fixture = Fixture::new("");
    fs::write(fixture.repository.join("proofs/theorems.json"), "[]").unwrap();
    assert_eq!(fixture.capture().unwrap_err().phase(), Phase::Preflight);
    assert!(!fixture.evidence.join("commands").exists());
    assert_eq!(
        fs::read(fixture.repository.join("proofs/verification.json")).unwrap(),
        b"previous record"
    );
    assert!(
        !fixture
            .repository
            .join("proofs/verification/.capture-lock")
            .exists()
    );
}

#[test]
fn populated_evidence_is_never_overwritten() {
    let fixture = Fixture::new("");
    fs::write(fixture.evidence.join("previous"), "retained").unwrap();
    assert_eq!(fixture.capture().unwrap_err().phase(), Phase::Preflight);
    assert_eq!(
        fs::read(fixture.evidence.join("previous")).unwrap(),
        b"retained"
    );
    assert_eq!(fs::read_dir(&fixture.evidence).unwrap().count(), 1);
}

#[test]
fn self_rebuild_cannot_replace_the_admitted_checker() {
    let fixture = Fixture::new("self-replace");
    let before = fs::read(fixture.tools.join("zetesis-maintenance")).unwrap();
    fixture.capture().unwrap();
    assert_eq!(
        fs::read(fixture.tools.join("zetesis-maintenance")).unwrap(),
        b"a subsequently rebuilt command"
    );
    assert_eq!(
        fs::read(fixture.evidence.join("tools/zetesis-maintenance")).unwrap(),
        before
    );
    let receipt: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .evidence
                .join("commands/maintenance-version/invocation.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let frozen = fixture.evidence.join("tools/zetesis-maintenance");
    assert!(
        receipt["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|argument| argument.as_str() == frozen.to_str())
    );
    let identity: Value =
        serde_json::from_slice(&fs::read(fixture.evidence.join("tools/identity.json")).unwrap())
            .unwrap();
    assert_eq!(identity["original_sha256"], identity["frozen_sha256"]);
    let record: Value = serde_json::from_slice(
        &fs::read(fixture.repository.join("proofs/verification.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        record["tool_sha256"]["zetesis-maintenance"],
        identity["frozen_sha256"]
    );
}
