//! Synthetic execution/publication controls, never evidence of Lean acceptance.
#![cfg(feature = "test-fixtures")]
use serde_json::Value;
use std::{fs, path::PathBuf, time::Duration};
use zetesis_maintenance::proofs::{
    self,
    capture::{self, Phase, Request},
};

struct Fixture {
    _directory: tempfile::TempDir,
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
        for name in ["lake", "lean", "rustc", "cargo", "zetesis-maintenance"] {
            fs::hard_link(
                env!("CARGO_BIN_EXE_zetesis-maintenance-fixture"),
                tools.join(name),
            )
            .unwrap();
        }
        Self {
            _directory: directory,
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
