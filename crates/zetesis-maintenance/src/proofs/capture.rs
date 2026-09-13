//! Execute pinned Lean checks and publish their current record transactionally.
//!
//! The caller supplies actual tool directories and a fresh external evidence
//! directory. Existing generated index/Audit views must match current sources.
//! Captures have bounded polling/output and settle owned children before fallible
//! retention. No success is inferred from a historical record or supplied count.
//! Concurrent edits and other commands writing these artifacts are unsupported;
//! the capture lock excludes other captures, not arbitrary filesystem writers.
//! Kernel laws remain distinct from Rust/WGSL refinement and physical execution.

use crate::{inventory, proofs};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};
use zetesis_validation::process::Limits;

mod retention;
mod publication;
mod execution;
mod record;
use execution::{Tools, check_assurance_tools, command_environment, execute_kernel};
use record::{
    check_staged_record, construct_record, prepare_record_storage, retain_current_sources,
    source_preflight,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const INPUT_BYTES: usize = 256 * 1024 * 1024;

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn bytes(path: &Path) -> Result<Vec<u8>> {
    Ok(inventory::read(path, INPUT_BYTES)?)
}

fn digest(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(bytes(path)?)))
}

fn json_file(path: &Path, value: &Value) -> Result<()> {
    let mut output = serde_json::to_vec_pretty(value)?;
    output.push(b'\n');
    fs::write(path, output)?;
    Ok(())
}

fn copy_flat(source: &Path, destination: &Path) -> Result<()> {
    copy_flat_with(source, destination, proofs::Limits::default())
}
fn copy_flat_with(source: &Path, destination: &Path, limits: proofs::Limits) -> Result<()> {
    fs::create_dir(destination)?;
    let mut tree = crate::files::Tree::new(source, limits)?;
    for (index, entry) in fs::read_dir(source)?.enumerate() {
        require(index < limits.entries, "retained-log entry limit exceeded")?;
        let entry = entry?;
        require(
            entry.file_type()?.is_file(),
            "unexpected non-file in retained logs",
        )?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("non-UTF-8 retained-log name")?;
        fs::write(destination.join(name), tree.read(name)?)?;
    }
    Ok(())
}

/// Explicit effects and tool owners for a proof capture.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// Repository containing the `proofs` package.
    pub repository: &'a Path,
    /// Existing empty evidence directory, disjoint from the repository.
    pub evidence: &'a Path,
    /// Directory containing the actual pinned Lean and Lake executables.
    pub lean_bin: &'a Path,
    /// Directory containing the actual pinned Rust and Cargo executables.
    pub rust_bin: &'a Path,
    /// Actual maintenance executable to copy into the external evidence owner.
    /// The immutable copy is used for checks even if Cargo replaces this path.
    pub maintenance: &'a Path,
    /// Per-command polling, output and initial cleanup limits.
    pub command_limits: Limits,
}

/// Operation whose failure prevented a complete capture/publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Paths, current source/views or tool owners were not admitted.
    Preflight,
    /// Pinned Lean version/build/strict checks or audit failed.
    Kernel,
    /// Rust/maintenance identity or checker regressions failed.
    Assurance,
    /// Captured sources, tools or staged record did not agree.
    Validation,
    /// Artifact publication or recovery failed; inspect preserved evidence.
    Publication,
    /// The new record is committed, but owned scratch/lock cleanup failed.
    Cleanup,
}

/// Capture failure with a typed operation boundary and original diagnostic chain.
#[derive(Debug)]
pub struct Failure {
    phase: Phase,
    cause: Box<dyn Error>,
}
impl Failure {
    /// Operation whose contract was not completed.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "proof capture {:?}: {}", self.phase, self.cause)
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
fn phase<T>(which: Phase, work: impl FnOnce() -> Result<T>) -> std::result::Result<T, Failure> {
    work().map_err(|cause| Failure {
        phase: which,
        cause,
    })
}

fn freeze_maintenance(original: &Path, work: &Path) -> Result<PathBuf> {
    let directory = work.join("tools");
    fs::create_dir(&directory)?;
    let frozen = directory.join("zetesis-maintenance");
    let original_bytes = bytes(original)?;
    let hash = format!("{:x}", Sha256::digest(&original_bytes));
    fs::write(&frozen, original_bytes)?;
    fs::set_permissions(&frozen, fs::metadata(original)?.permissions())?;
    require(digest(&frozen)? == hash, "frozen maintenance copy differs")?;
    json_file(
        &directory.join("identity.json"),
        &serde_json::json!({
            "original_path": original, "original_sha256": hash,
            "frozen_path": frozen, "frozen_sha256": hash,
            "scope": "exact admitted executable bytes; permission-preserving copy, not a dependency closure"
        }),
    )?;
    Ok(frozen)
}

struct Inputs {
    repository: PathBuf,
    work: PathBuf,
    tools: Tools,
    maintenance: PathBuf,
    before: proofs::Inventory,
    tool_hashes: BTreeMap<String, String>,
    documents: BTreeMap<&'static str, String>,
    lock: publication::Lock,
}
fn preflight(request: Request<'_>) -> Result<Inputs> {
    let repository = request.repository.canonicalize()?;
    let work = request.evidence.canonicalize()?;
    require(
        repository.is_dir() && work.is_dir(),
        "repository/evidence must be directories",
    )?;
    require(
        !work.starts_with(&repository) && !repository.starts_with(&work),
        "evidence must be disjoint from the repository",
    )?;
    require(
        fs::read_dir(&work)?.next().is_none(),
        "evidence directory must be empty",
    )?;
    let proof_root = repository.join("proofs");
    let lock = publication::Lock::acquire(&proof_root)?;
    let before = source_preflight(&proof_root)?;
    let documents = ["README.md", "theorems.json"]
        .into_iter()
        .map(|name| Ok((name, digest(&proof_root.join(name))?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let tools = Tools::new(&request)?;
    let original_maintenance = request.maintenance.canonicalize()?;
    let maintenance = freeze_maintenance(&original_maintenance, &work)?;
    let tool_hashes = tools.hashes(&maintenance)?;
    json_file(
        &work.join("inputs.json"),
        &serde_json::json!({
            "source_sha256": before.source_sha256(), "document_sha256": documents,
            "tool_sha256": tool_hashes,
            "system_executables": {
                "/usr/bin/env": digest(Path::new("/usr/bin/env"))?,
                "/bin/sh": digest(Path::new("/bin/sh"))?
            }
        }),
    )?;
    Ok(Inputs {
        repository,
        work,
        tools,
        maintenance,
        before,
        tool_hashes,
        documents,
        lock,
    })
}

/// Execute current pinned checks and publish only their validated record.
///
/// The index and Audit must already equal `inventory`'s deterministic views.
/// Before execution, archive original records in `evidence/before`; failed
/// commands retain raw receipts under `evidence/commands` and record views under
/// `evidence/stage`. The process owner settles
/// children before fallible retention. A staged record must verify against the
/// unchanged source/tool identities before publication. Publication uses local
/// renames with rollback; concurrent readers can observe a temporary mismatch,
/// so this is an exclusive multi-file transaction, not a lock-free snapshot.
/// Failed recovery retains the original archive and reports it explicitly.
///
/// This operation writes records and Lean/Cargo build artifacts and runs the
/// recorded commands. It never imports coverage or changes semantic sources.
/// Byte/traversal ceilings apply to each bounded read pass; they are not a total
/// process-memory or filesystem snapshot guarantee. Theorem counts are derived.
///
/// # Errors
/// Refuses bad/freshness-overlapping paths, stale views, wrong tools, failed or
/// interrupted commands, changed inputs, invalid audit/records and publication
/// failures. No failure establishes a proof result or implementation refinement.
pub fn capture(request: Request<'_>) -> std::result::Result<proofs::Summary, Failure> {
    let Inputs {
        repository,
        work,
        tools,
        maintenance,
        before,
        tool_hashes,
        documents,
        lock,
    } = phase(Phase::Preflight, || preflight(request))?;
    let proof_root = repository.join("proofs");
    let (archive, mut runner) = phase(Phase::Preflight, || {
        prepare_record_storage(
            repository,
            &work,
            &proof_root,
            command_environment(&tools)?,
            request.command_limits,
        )
    })?;
    let kernel = phase(Phase::Kernel, || {
        execute_kernel(&mut runner, &before, &tools)
    })?;
    phase(Phase::Assurance, || {
        check_assurance_tools(&mut runner, &tools, &maintenance)
    })?;
    let (after, summary) = phase(Phase::Validation, || {
        require(
            tools.hashes(&maintenance)? == tool_hashes,
            "capture tools changed during execution",
        )?;
        for (name, hash) in &documents {
            require(
                digest(&proof_root.join(name))? == *hash,
                "proof documentation/views changed during capture",
            )?;
        }
        let after = retain_current_sources(&mut runner, &proof_root, &before)?;
        let record = construct_record(&runner, &after, &kernel, &tools, &maintenance)?;
        check_staged_record(&runner, &record, after.declarations().len())?;
        let summary = proofs::verify_with_audit(
            &runner.stage,
            "verification.json",
            &kernel.audit,
            proofs::Limits::default(),
        )?;
        Ok((after, summary))
    })?;
    let published = phase(Phase::Publication, || {
        publication::publish(&runner.stage, &proof_root, &archive, || {
            proofs::verify_with_audit(
                &proof_root,
                "verification.json",
                &kernel.audit,
                proofs::Limits::default(),
            )?;
            require(
                proofs::inventory(&proof_root, proofs::Limits::default())?.source_sha256()
                    == after.source_sha256(),
                "proof sources changed before publication",
            )?;
            Ok(())
        })
    })?;
    phase(Phase::Cleanup, || published.cleanup())?;
    phase(Phase::Cleanup, || lock.release())?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_copy_admits_inclusive_read_limits() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("one"), "abc").unwrap();
        fs::write(source.join("two"), "def").unwrap();
        let exact = proofs::Limits {
            file_bytes: 3,
            total_bytes: 6,
            entries: 2,
        };
        copy_flat_with(&source, &directory.path().join("exact"), exact).unwrap();
        for (label, limits) in [
            (
                "file",
                proofs::Limits {
                    file_bytes: 2,
                    ..exact
                },
            ),
            (
                "total",
                proofs::Limits {
                    total_bytes: 5,
                    ..exact
                },
            ),
            (
                "entries",
                proofs::Limits {
                    entries: 1,
                    ..exact
                },
            ),
        ] {
            assert!(copy_flat_with(&source, &directory.path().join(label), limits).is_err());
            assert_eq!(fs::read(source.join("one")).unwrap(), b"abc");
            assert_eq!(fs::read(source.join("two")).unwrap(), b"def");
        }
    }

    #[test]
    fn archive_copy_refuses_nested_owners() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(source.join("nested")).unwrap();
        assert!(copy_flat(&source, &directory.path().join("copy")).is_err());
    }
}
