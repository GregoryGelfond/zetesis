//! Retained Lean proof-record consistency under a deliberately restricted convention.
//!
//! The checker recognizes plain line-leading theorems, explicit namespaces and
//! sections, and ASCII qualified names. Nested comments and strings are masked.
//! It refuses other declaration forms instead of pretending to parse Lean.
//! Actual pinned kernel checking must run independently before record validation.
//! Hashes establish consistency, not command execution or compiler refinement.
mod audit;
mod inventory;
mod source;

pub use inventory::{Inventory, inventory};
pub use source::Declaration;

pub use crate::files::Limits;
use crate::{
    Error,
    files::{self, Tree},
    json, require,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};

const FIXED: [&str; 5] = [
    "Audit.lean",
    "Zetesis.lean",
    "lakefile.lean",
    "lake-manifest.json",
    "lean-toolchain",
];
const ARTIFACTS: [&str; 3] = ["README.md", "theorems.json", "axiom-audit.txt"];

/// Counts derived from the actual record and source inventory.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Summary {
    /// Exact unique theorem declarations and matching audit entries.
    pub theorems: usize,
    /// Recursively inventoried semantic Lean modules.
    pub semantic_modules: usize,
    /// Semantic modules plus the fixed configuration/source set.
    pub source_files: usize,
}
/// Read and validate a retained record without executing commands or writing files.
///
/// The record name is relative to `root`. File reads and traversal use `limits`;
/// retained storage is bounded by read bytes plus inventory and parsed JSON
/// overhead. JSON nesting is limited by `serde_json`'s default recursion ceiling.
/// Work is linear in source and record bytes, apart from ordered-set indexing.
///
/// # Errors
/// Refuses malformed, stale, incomplete or unsupported records, escaped paths,
/// exceeded read budgets and filesystem failures. None of these is a Lean verdict.
pub fn verify(root: &Path, record_name: &str, limits: Limits) -> Result<Summary, Error> {
    verify_record(root, record_name, limits, None)
}

/// Check a retained record against the actual output of a newly executed audit.
///
/// `live_audit` must contain complete stdout from the pinned strict Audit command;
/// the caller separately requires its successful exit and empty stderr. This
/// operation executes nothing and compares exact bytes as well as the existing
/// source/index/axiom contracts. The borrowed output has the file-byte ceiling.
///
/// # Errors
/// Refuses every invalid retained record, oversized live output, or a live audit
/// differing from its retained counterpart. Equality does not attest execution.
pub fn verify_with_audit(
    root: &Path,
    record_name: &str,
    live_audit: &[u8],
    limits: Limits,
) -> Result<Summary, Error> {
    if live_audit.len() > limits.file_bytes {
        return Err(Error::Limit {
            resource: "live audit bytes",
            limit: limits.file_bytes,
        });
    }
    verify_record(root, record_name, limits, Some(live_audit))
}

fn verify_record(
    root: &Path,
    record_name: &str,
    limits: Limits,
    live_audit: Option<&[u8]>,
) -> Result<Summary, Error> {
    let mut tree = Tree::new(root, limits)?;
    let record = json::parse(&tree.read(record_name)?)?;
    json::object(&record, "verification record")?;
    require(
        record["schema_version"].as_u64() == Some(1) && record["status"] == "PASS",
        "unsupported or unsuccessful verification record",
    )?;
    let modules = tree.inventory("Zetesis", ".lean")?;
    require(!modules.is_empty(), "missing semantic modules")?;
    let mut sources = modules.clone();
    sources.extend(FIXED.map(str::to_owned));
    hashes(
        &mut tree,
        &record["source_sha256"],
        Some(&sources),
        "source",
    )?;
    hashes(
        &mut tree,
        &record["generated_artifact_sha256"],
        Some(&ARTIFACTS.map(str::to_owned).into()),
        "generated artifacts",
    )?;
    let mut entries = Vec::new();
    for module in &modules {
        entries.extend(source::declarations(module, &tree.text(module)?)?);
    }
    let names: BTreeSet<String> = entries.iter().map(|entry| entry.name.clone()).collect();
    require(!entries.is_empty(), "empty source theorem inventory")?;
    require(
        names.len() == entries.len(),
        "duplicate qualified source theorem",
    )?;
    index(&mut tree, &entries)?;
    let axioms = audit::check(&mut tree, &names)?;
    if let Some(live) = live_audit {
        require(
            tree.read("axiom-audit.txt")? == live,
            "live audit differs from the retained kernel output",
        )?;
    }
    counts(&record, &entries, modules.len())?;
    require(
        record["transitive_axioms"] == serde_json::to_value(axioms).map_err(Error::Json)?,
        "transitive axiom inventory mismatch",
    )?;
    require(
        record["project_axioms"]
            .as_array()
            .is_some_and(Vec::is_empty)
            && record["proof_holes"].as_bool() == Some(false)
            && record["native_evaluation_proof_shortcuts"].as_bool() == Some(false),
        "unsupported proof assurance flags",
    )?;
    audit::commands(&mut tree, &record)?;
    Ok(Summary {
        theorems: entries.len(),
        semantic_modules: modules.len(),
        source_files: sources.len(),
    })
}
fn hashes(
    tree: &mut Tree,
    value: &Value,
    expected: Option<&BTreeSet<String>>,
    label: &str,
) -> Result<(), Error> {
    let map = json::object(value, label)?;
    if let Some(expected) = expected {
        require(
            map.keys().cloned().collect::<BTreeSet<_>>() == *expected,
            format!("{label} inventory mismatch"),
        )?;
    }
    for (name, value) in map {
        let digest = json::string(value, "digest")?;
        require(
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            format!("invalid {label} digest: {name}"),
        )?;
        require(
            files::digest(&tree.read(name)?) == digest,
            format!("{label} hash mismatch: {name}"),
        )?;
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedDeclaration {
    name: String,
    file: String,
    line: usize,
}

fn index(tree: &mut Tree, expected: &[source::Declaration]) -> Result<(), Error> {
    let value = json::parse(&tree.read("theorems.json")?)?;
    let recorded: Vec<RecordedDeclaration> = serde_json::from_value(value).map_err(Error::Json)?;
    let entries: Vec<_> = recorded
        .into_iter()
        .map(|entry| source::Declaration {
            name: entry.name,
            file: entry.file,
            line: entry.line,
        })
        .collect();
    require(
        entries.iter().all(|entry| entry.line > 0),
        "invalid theorem index location",
    )?;
    let names: BTreeSet<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    require(names.len() == entries.len(), "duplicate theorem index name")?;
    require(
        entries.iter().collect::<BTreeSet<_>>() == expected.iter().collect(),
        "theorem index names/files/lines differ from source declarations",
    )
}
fn count(value: &Value, expected: usize, label: &str) -> Result<(), Error> {
    require(
        value.as_u64().and_then(|n| usize::try_from(n).ok()) == Some(expected),
        format!("{label} count mismatch"),
    )
}
fn counts(record: &Value, entries: &[source::Declaration], modules: usize) -> Result<(), Error> {
    count(&record["semantic_modules"], modules, "semantic module")?;
    count(
        &record["theorems_audited"],
        entries.len(),
        "top-level theorem",
    )?;
    if let Some(value) = record.get("batch_accounting_laws") {
        let expected = entries
            .iter()
            .filter(|entry| entry.file == "Zetesis/BatchAccounting.lean")
            .count();
        require(
            expected > 0,
            "batch accounting module theorem count mismatch",
        )?;
        count(value, expected, "batch accounting module theorem")?;
    }
    let consistency = &record["audit_consistency"];
    json::object(consistency, "nested audit consistency")?;
    for key in [
        "source_theorem_declarations",
        "indexed_theorems",
        "printed_axiom_entries",
        "unique_qualified_names",
    ] {
        count(&consistency[key], entries.len(), &format!("nested {key}"))?;
    }
    for key in ["source_index_line_check", "allowed_transitive_axioms_only"] {
        require(
            consistency[key] == "PASS",
            format!("nested audit failure: {key}"),
        )?;
    }
    Ok(())
}
