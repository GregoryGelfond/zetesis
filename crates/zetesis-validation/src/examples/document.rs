//! Immutable checked corpus values and pinned document admission.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    CASE_COUNT, Contract, Error, Family, Limits, MANIFEST_SHA256, ORIGINAL_MANIFEST_SHA256,
    Resource, SOURCE_COUNT, Satisfiability, UPSTREAM_REVISION, ceiling, files,
};

/// Coordinates of one removed whole-line comment in the original source.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    pub(super) line: usize,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
    pub(super) source: String,
}
impl Annotation {
    /// One-based physical line in the original file.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }
    /// Inclusive original byte offset of the removed line.
    #[must_use]
    pub const fn start_byte(&self) -> usize {
        self.start_byte
    }
    /// Exclusive original byte offset, including its line terminator if present.
    #[must_use]
    pub const fn end_byte(&self) -> usize {
        self.end_byte
    }
    /// Original comment without its line terminator.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// One original include directive, whose spelling is unchanged in the example.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Include {
    line: usize,
    spelling: String,
    path: String,
}
impl Include {
    /// One-based original physical line, before annotation removal.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }
    /// Original spelling relative to its source directory.
    #[must_use]
    pub fn spelling(&self) -> &str {
        &self.spelling
    }
    /// Resolved dependency path relative to the examples root.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Sealed cleaned source with its separate original identity and deletion map.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    path: String,
    original_sha256: String,
    #[serde(rename = "source_sha256")]
    sha256: String,
    removed_annotations: Vec<Annotation>,
    includes: Vec<Include>,
    #[serde(skip)]
    text: String,
}
impl Source {
    /// Portable relative path inside the clean examples tree.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Retained cleaned source bytes decoded as UTF-8.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.text
    }
    /// SHA-256 of the cleaned source.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.sha256
    }
    /// SHA-256 of the untouched original, including annotations.
    #[must_use]
    pub fn original_sha256(&self) -> &str {
        &self.original_sha256
    }
    /// Exact ordered original annotation deletion coordinates.
    #[must_use]
    pub fn removed_annotations(&self) -> &[Annotation] {
        &self.removed_annotations
    }
    /// Original include dependencies and spellings.
    #[must_use]
    pub fn includes(&self) -> &[Include] {
        &self.includes
    }
}

/// A runnable scenario or standalone program and its typed display contract.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    path: String,
    source_sha256: String,
    original_sha256: String,
    includes: Vec<String>,
    transitive_source_paths: Vec<String>,
    contract: Contract,
}
impl Case {
    /// Entry source relative to the examples root.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Clean entry-source identity; include identities are in the source table.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    /// Original entry-source identity before annotation removal.
    #[must_use]
    pub fn original_sha256(&self) -> &str {
        &self.original_sha256
    }
    /// Direct include targets relative to the examples root.
    #[must_use]
    pub fn includes(&self) -> &[String] {
        &self.includes
    }
    /// Complete source closure, including the entry source itself.
    #[must_use]
    pub fn transitive_source_paths(&self) -> &[String] {
        &self.transitive_source_paths
    }
    /// Expectations on selected reported displays.
    #[must_use]
    pub const fn contract(&self) -> &Contract {
        &self.contract
    }
}

/// Historical contract-authoring toolchain, not a current execution receipt.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceToolchain {
    /// Clingo version named by the original corpus.
    pub clingo: String,
    /// Historical elenctic tag; elenctic is not required to run the clean corpus.
    pub elenctic_tag: String,
    /// Historical annotation-authoring tool revision.
    pub elenctic_revision: String,
    /// Upstream documents recording the toolchain.
    pub sources: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    upstream: String,
    revision: String,
    license: String,
    copyright: String,
    license_sha256: String,
    original_manifest_sha256: String,
    reference_toolchain: ReferenceToolchain,
    files: Vec<Source>,
    cases: Vec<Case>,
}

/// Entire sealed example corpus; source strings are owned independently of files.
#[derive(Debug)]
pub struct Corpus {
    root: PathBuf,
    upstream: String,
    revision: String,
    license_sha256: String,
    reference_toolchain: ReferenceToolchain,
    files: Vec<Source>,
    cases: Vec<Case>,
}
impl Corpus {
    /// Canonical root used for this load.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Runnable entry programs.
    #[must_use]
    pub fn cases(&self) -> &[Case] {
        &self.cases
    }
    /// Clean entry sources and shared schemas.
    #[must_use]
    pub fn files(&self) -> &[Source] {
        &self.files
    }
    /// Manifest seal checked before any schema-driven file access.
    #[must_use]
    pub const fn manifest_sha256(&self) -> &'static str {
        MANIFEST_SHA256
    }
    /// Source repository URL recorded by the corpus.
    #[must_use]
    pub fn upstream(&self) -> &str {
        &self.upstream
    }
    /// Original source revision.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }
    /// Historical contract-authoring toolchain.
    #[must_use]
    pub const fn reference_toolchain(&self) -> &ReferenceToolchain {
        &self.reference_toolchain
    }
    pub(super) fn license_sha256(&self) -> &str {
        &self.license_sha256
    }
}

pub(super) fn load(root: &Path, limits: Limits) -> Result<Corpus, Error> {
    let root = files::canonical(root)?;
    let bytes = files::read(
        &files::confined(&root, "manifest.json")?,
        limits.manifest_bytes,
        Resource::ManifestBytes,
    )?;
    files::digest("manifest.json", &bytes, MANIFEST_SHA256)?;
    let mut document: Document = serde_json::from_slice(&bytes).map_err(Error::Json)?;
    validate_header(&document, limits)?;
    let license = files::read(
        &files::confined(&root, "LICENSE")?,
        limits.source_bytes,
        Resource::SourceBytes,
    )?;
    files::digest("LICENSE", &license, &document.license_sha256)?;
    let mut total = 0_u128;
    let mut paths = BTreeSet::new();
    for source in &mut document.files {
        files::relative(&source.path)?;
        if !paths.insert(source.path.clone())
            || Path::new(&source.path).extension() != Some(std::ffi::OsStr::new("lp"))
            || source.path.contains("clingcon")
        {
            return Err(Error::Contract(format!(
                "duplicate or excluded source {}",
                source.path
            )));
        }
        let bytes = files::read(
            &files::confined(&root, &source.path)?,
            limits.source_bytes,
            Resource::SourceBytes,
        )?;
        files::digest(&source.path, &bytes, &source.sha256)?;
        total += bytes.len() as u128;
        ceiling(Resource::TotalSourceBytes, total, limits.total_source_bytes)?;
        source.text = String::from_utf8(bytes).map_err(Error::Utf8)?;
        if source
            .text
            .lines()
            .any(|line| line.trim_start().starts_with("% @"))
        {
            return Err(Error::Contract(format!(
                "annotation remains in {}",
                source.path
            )));
        }
    }
    validate_cases(&document, &root, limits)?;
    Ok(Corpus {
        root,
        upstream: document.upstream,
        revision: document.revision,
        license_sha256: document.license_sha256,
        reference_toolchain: document.reference_toolchain,
        files: document.files,
        cases: document.cases,
    })
}

fn validate_header(document: &Document, limits: Limits) -> Result<(), Error> {
    ceiling(Resource::Files, document.files.len() as u128, limits.files)?;
    ceiling(Resource::Cases, document.cases.len() as u128, limits.cases)?;
    if document.schema_version != 1
        || document.upstream != "https://github.com/GregoryGelfond/kr-domains"
        || document.revision != UPSTREAM_REVISION
        || document.license != "MIT"
        || document.copyright != "Copyright (c) 2026 Gregory Gelfond"
        || document.original_manifest_sha256 != ORIGINAL_MANIFEST_SHA256
        || document.files.len() != SOURCE_COUNT
        || document.cases.len() != CASE_COUNT
    {
        return Err(Error::Contract("pinned corpus header differs".into()));
    }
    Ok(())
}

fn validate_cases(document: &Document, root: &Path, limits: Limits) -> Result<(), Error> {
    let sources: BTreeMap<_, _> = document
        .files
        .iter()
        .map(|source| (source.path(), source))
        .collect();
    let mut entries = BTreeSet::new();
    let mut symbols = 0_u128;
    for case in &document.cases {
        let source = sources
            .get(case.path())
            .ok_or_else(|| Error::Contract(format!("missing entry {}", case.path)))?;
        if !entries.insert(case.path())
            || case.source_sha256 != source.sha256
            || case.original_sha256 != source.original_sha256
            || !case
                .includes
                .iter()
                .map(String::as_str)
                .eq(source.includes.iter().map(Include::path))
        {
            return Err(Error::Contract(format!(
                "entry metadata differs for {}",
                case.path
            )));
        }
        let mut reached = BTreeSet::new();
        let mut pending = vec![case.path()];
        while let Some(path) = pending.pop() {
            if reached.insert(path) {
                let dependency = sources
                    .get(path)
                    .ok_or_else(|| Error::Contract(format!("missing dependency {path}")))?;
                for include in &dependency.includes {
                    let parent = Path::new(&dependency.path)
                        .parent()
                        .ok_or_else(|| Error::Path(dependency.path.clone()))?;
                    let resolved = files::canonical(&root.join(parent).join(&include.spelling))?;
                    if resolved != files::confined(root, include.path())? {
                        return Err(Error::Contract(format!(
                            "include spelling differs in {}",
                            dependency.path
                        )));
                    }
                    pending.push(include.path());
                }
            }
        }
        let recorded: BTreeSet<_> = case
            .transitive_source_paths
            .iter()
            .map(String::as_str)
            .collect();
        if recorded.len() != case.transitive_source_paths.len() || reached != recorded {
            return Err(Error::Contract(format!(
                "dependency closure differs for {}",
                case.path
            )));
        }
        validate_contract(&case.contract)?;
        symbols += case
            .contract
            .witnesses
            .iter()
            .map(|witness| witness.len() as u128)
            .sum::<u128>()
            + case.contract.required_symbols.len() as u128;
        ceiling(Resource::ContractSymbols, symbols, limits.contract_symbols)?;
    }
    Ok(())
}

pub(super) fn validate_contract(contract: &Contract) -> Result<(), Error> {
    let sorted = |symbols: &[String]| symbols.windows(2).all(|pair| pair[0] <= pair[1]);
    if contract.witnesses.iter().any(|witness| !sorted(witness))
        || !sorted(&contract.required_symbols)
        || contract.family == Family::All && contract.cost.is_some()
        || contract.satisfiability == Satisfiability::Unsat
            && (contract.cost.is_some()
                || !contract.witnesses.is_empty()
                || !contract.required_symbols.is_empty()
                || contract.model_count.is_some_and(|count| count != 0))
        || contract.satisfiability == Satisfiability::Sat && contract.model_count == Some(0)
    {
        return Err(Error::Contract("inconsistent display contract".into()));
    }
    Ok(())
}
