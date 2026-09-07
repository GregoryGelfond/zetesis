//! Validated owned corpus and borrowed views of exact contracts.

use super::{
    CASE_COUNT, Error, LICENSE_SHA256, Limits, Resource, UPSTREAM_REVISION, ceiling, contracts,
    files,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
    pub schema: u32,
    pub upstream_revision: String,
    pub license_path: String,
    pub license_sha256: String,
    pub origins: Vec<OriginRecord>,
    pub cases: Vec<CaseRecord>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OriginRecord {
    pub path: String,
    pub url: String,
    pub sha256: String,
    pub copyright_notice: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseRecord {
    pub id: String,
    pub path: String,
    pub source_sha256: String,
    pub license: String,
    pub provenance: ProvenanceRecord,
    pub contract: ContractRecord,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProvenanceRecord {
    pub source_file: String,
    pub section: String,
    pub assertion_ordinal: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub assertion: String,
    pub helper_arguments: Vec<String>,
    pub expected_helper_output: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContractRecord {
    pub full_models: Vec<Vec<String>>,
    pub prefixes: Vec<String>,
    pub helper_models: Vec<Vec<String>>,
}

/// A verified pinned corpus retaining the exact source bytes independently of files.
#[derive(Debug)]
pub struct Corpus {
    root: PathBuf,
    origins: Vec<OriginRecord>,
    cases: Vec<Case>,
}
impl Corpus {
    /// Canonical directory read at verification; this is not a filesystem lease.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Complete selected case population, in original catalog order.
    #[must_use]
    pub fn cases(&self) -> &[Case] {
        &self.cases
    }
    /// Source-file authorities and verbatim individual copyright notices.
    pub fn origins(&self) -> impl Iterator<Item = Origin<'_>> {
        self.origins.iter().map(|record| Origin { record })
    }
}
/// One source with checked provenance and full-model contracts.
#[derive(Debug)]
pub struct Case {
    record: CaseRecord,
    source: String,
}
impl Case {
    /// Original assertion identity; unrelated to current native admission.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.record.id
    }
    /// Confined relative `.lp` path whose bytes were checked.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.record.path
    }
    /// SHA-256 of the exact decoded ASP bytes.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.record.source_sha256
    }
    /// Exact original decoded ASP, including every space and terminal newline.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
    /// MIT applies with the original-file notice in [`Corpus::origins`].
    #[must_use]
    pub fn license(&self) -> &str {
        &self.record.license
    }
    /// Original assertion spelling and source coordinates.
    #[must_use]
    pub fn provenance(&self) -> Provenance<'_> {
        Provenance {
            record: &self.record.provenance,
        }
    }
    /// Distinct full-model and original helper-display contracts.
    #[must_use]
    pub fn contract(&self) -> Contract<'_> {
        Contract {
            record: &self.record.contract,
        }
    }
}
/// Borrowed original-file authority; the notice is preserved verbatim.
#[derive(Clone, Copy, Debug)]
pub struct Origin<'a> {
    record: &'a OriginRecord,
}
impl<'a> Origin<'a> {
    /// Original path relative to the pinned upstream repository.
    #[must_use]
    pub fn path(self) -> &'a str {
        &self.record.path
    }
    /// Public immutable source link at the pinned commit.
    #[must_use]
    pub fn url(self) -> &'a str {
        &self.record.url
    }
    /// Exact original file identity, checked during import.
    #[must_use]
    pub fn sha256(self) -> &'a str {
        &self.record.sha256
    }
    /// Exact original notice block, with its existing copyright attribution.
    #[must_use]
    pub fn copyright_notice(self) -> &'a str {
        &self.record.copyright_notice
    }
}
/// Borrowed provenance of one selected original assertion.
#[derive(Clone, Copy, Debug)]
pub struct Provenance<'a> {
    record: &'a ProvenanceRecord,
}
impl<'a> Provenance<'a> {
    /// Original file key resolving through [`Corpus::origins`].
    #[must_use]
    pub fn source_file(self) -> &'a str {
        &self.record.source_file
    }
    /// Original named test section.
    #[must_use]
    pub fn section(self) -> &'a str {
        &self.record.section
    }
    /// One-based assertion ordinal within the section.
    #[must_use]
    pub fn assertion_ordinal(self) -> usize {
        self.record.assertion_ordinal
    }
    /// Half-open original UTF-8 byte range.
    #[must_use]
    pub fn bytes(self) -> std::ops::Range<usize> {
        self.record.byte_start..self.record.byte_end
    }
    /// Inclusive original one-based line range.
    #[must_use]
    pub fn lines(self) -> std::ops::RangeInclusive<usize> {
        self.record.line_start..=self.record.line_end
    }
    /// Exact assertion spelling, including adjacent C++ string literals.
    #[must_use]
    pub fn assertion(self) -> &'a str {
        &self.record.assertion
    }
    /// Original helper argument spellings after the source literal.
    #[must_use]
    pub fn helper_arguments(self) -> &'a [String] {
        &self.record.helper_arguments
    }
    /// Exact original model/diagnostic expectation; native diagnostic text is separate.
    #[must_use]
    pub fn expected_helper_output(self) -> &'a str {
        &self.record.expected_helper_output
    }
}
/// Borrowed complete models and the original prefix-selected helper view.
#[derive(Clone, Copy, Debug)]
pub struct Contract<'a> {
    record: &'a ContractRecord,
}
impl<'a> Contract<'a> {
    /// Complete recorded model occurrences. Repetition is never deduplicated.
    #[must_use]
    pub fn full_models(self) -> &'a [Vec<String>] {
        &self.record.full_models
    }
    /// Prefixes applied to each full model by the original helper.
    #[must_use]
    pub fn prefixes(self) -> &'a [String] {
        &self.record.prefixes
    }
    /// Selected model occurrences, retaining repeated and empty displays.
    #[must_use]
    pub fn helper_models(self) -> &'a [Vec<String>] {
        &self.record.helper_models
    }
}

pub(super) fn validate(document: &Document, limits: Limits) -> Result<(), Error> {
    ceiling(Resource::Cases, document.cases.len() as u128, limits.cases)?;
    if document.schema != 1
        || document.upstream_revision != UPSTREAM_REVISION
        || document.cases.len() != CASE_COUNT
        || document.license_path != "LICENSE.md"
        || document.license_sha256 != LICENSE_SHA256
    {
        return Err(Error::Contract(
            "expected schema 1 and the pinned 24-case MIT target".into(),
        ));
    }
    contracts::validate(document, limits)
}
pub(super) fn load(root: PathBuf, document: Document, limits: Limits) -> Result<Corpus, Error> {
    validate(&document, limits)?;
    let license = files::read(
        &files::confined(&root, &document.license_path)?,
        limits.original_bytes,
        Resource::OriginalBytes,
    )?;
    files::digest(&document.license_path, &license, &document.license_sha256)?;
    let mut cases = Vec::new();
    let mut total = 0_u128;
    for record in document.cases {
        let bytes = files::read(
            &files::confined(&root, &record.path)?,
            limits.source_bytes,
            Resource::SourceBytes,
        )?;
        total += bytes.len() as u128;
        ceiling(Resource::TotalSourceBytes, total, limits.total_source_bytes)?;
        files::digest(&record.path, &bytes, &record.source_sha256)?;
        let source = String::from_utf8(bytes)
            .map_err(|_| Error::Contract(format!("{}: source must be UTF-8", record.id)))?;
        cases.push(Case { record, source });
    }
    Ok(Corpus {
        root,
        origins: document.origins,
        cases,
    })
}
