//! Sealed constant-value variants of an admitted corpus entry, and sealed
//! generated programs whose bytes are a pure function of a family and a size.

mod constants;

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

use super::super::Error;
use super::super::families::Family;
use crate::examples;

const MAX_AMENDMENTS: usize = 128;

/// One requested integer constant replacement; borrowed metadata is caller-owned.
#[derive(Clone, Copy, Debug)]
pub struct ConstantAmendment<'a> {
    /// Entry/include path relative to the admitted corpus.
    pub source_path: &'a str,
    /// Exact declared constant name.
    pub name: &'a str,
    /// Required original nonnegative decimal i32 literal.
    pub expected: i32,
    /// Replacement i32, rendered as decimal without inserting other syntax.
    pub replacement: i32,
}

/// Inclusive admission bounds for one source workload.
///
/// Source bytes bound parser input and derived content, not syntax-tree
/// allocation or RSS. Metadata bytes count retained provenance text and cost
/// values; vector/object counts are bounded by source and amendment populations.
#[derive(Clone, Copy, Debug)]
pub struct WorkloadLimits {
    /// Input or derived bytes in one source.
    pub source_bytes: usize,
    /// Combined input plus derived closure bytes inspected during admission.
    pub total_source_bytes: usize,
    /// Retained path, digest, edit and default-contract payload bytes.
    pub metadata_bytes: usize,
    /// Requested replacements, also bounded by 128.
    pub amendments: usize,
}

impl Default for WorkloadLimits {
    fn default() -> Self {
        Self {
            source_bytes: 1_048_576,
            total_source_bytes: 4_194_304,
            metadata_bytes: 65_536,
            amendments: MAX_AMENDMENTS,
        }
    }
}

/// Validated constant-only derivation of one complete admitted source closure,
/// or one generated program.
///
/// No source payload is retained. Materialization repeats the checked derivation
/// or generation and seals the bytes actually launched. The unchanged corpus
/// contract remains provenance for amended workloads; a generated workload's
/// contract is its family's closed-form count.
#[derive(Clone, Debug, Serialize)]
pub struct Workload {
    entry: String,
    identity: String,
    /// Corpus identity of a derived workload; absent for a generated one.
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest_sha256: Option<&'static str>,
    sources: Vec<Source>,
    default_contract: examples::Contract,
    amended: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated: Option<Generated>,
    pub(super) metadata_bytes: usize,
    pub(super) source_bytes: usize,
}

/// Provenance of a generated program: the family, the size and the digest of
/// the bytes the generator produced when the workload was admitted.
#[derive(Clone, Debug, Serialize)]
struct Generated {
    family: Family,
    size: u32,
    sha256: String,
    bytes: usize,
}

impl Generated {
    /// The entry path of a generated program of the family and size.
    fn entry_of(family: Family, size: u32) -> String {
        format!("generated/{}-{size}.lp", family.label())
    }

    /// The entry path of the generated program.
    fn entry(&self) -> String {
        Self::entry_of(self.family, self.size)
    }

    /// The program the generator produces now, when it is the one admitted:
    /// the same length and digest.
    ///
    /// # Errors
    /// Refuses a size the family no longer admits, or a program other than
    /// the admitted one.
    fn regenerate(&self) -> Result<String, Error> {
        let source = self
            .family
            .source(self.size)
            .map_err(|_| Error::Configuration("generated workload size is not admitted"))?;
        if source.len() != self.bytes
            || format!("{:x}", Sha256::digest(source.as_bytes())) != self.sha256
        {
            return Err(Error::Configuration(
                "generated program differs from its admitted program",
            ));
        }
        Ok(source)
    }
}

#[derive(Clone, Debug, Serialize)]
struct Source {
    path: String,
    base_sha256: String,
    derived_sha256: String,
    derived_bytes: usize,
    edits: Vec<examples::Edit>,
}

impl Workload {
    /// Validate an unchanged entry and its parsed include closure.
    ///
    /// # Errors
    /// Refuses unknown entries, syntax/include discrepancies or admission bounds.
    pub fn original(
        corpus: &examples::Corpus,
        entry: &str,
        limits: WorkloadLimits,
    ) -> Result<Self, Error> {
        Self::amended(corpus, entry, &[], limits)
    }

    /// Validate literal constant replacements through the pinned syntax parser.
    ///
    /// Only unannotated nonnegative decimal i32 source literals are admitted.
    /// Each named declaration must occur once in its specified source; repeated
    /// requests for a declaration are refused. Replacements cannot alter includes.
    /// Parser input and all derived content are bounded before retention.
    ///
    /// # Errors
    /// Refuses unknown paths/names, duplicate requests, unsupported declarations,
    /// unexpected literal values, syntax/include discrepancies and resource bounds.
    pub fn amended(
        corpus: &examples::Corpus,
        entry: &str,
        amendments: &[ConstantAmendment<'_>],
        limits: WorkloadLimits,
    ) -> Result<Self, Error> {
        let case = corpus
            .cases()
            .iter()
            .find(|case| case.path() == entry)
            .ok_or(Error::Configuration(
                "workload entry is not in the admitted corpus",
            ))?;
        if amendments.len() > limits.amendments.min(MAX_AMENDMENTS) {
            return Err(Error::Configuration(
                "workload amendment count exceeds its ceiling",
            ));
        }
        let mut requested = 0usize;
        for (position, amendment) in amendments.iter().enumerate() {
            requested = add(requested, amendment.source_path.len())?;
            requested = add(requested, amendment.name.len())?;
            if requested > limits.metadata_bytes
                || !case
                    .transitive_source_paths()
                    .iter()
                    .any(|path| path == amendment.source_path)
                || amendments[..position].iter().any(|prior| {
                    prior.source_path == amendment.source_path && prior.name == amendment.name
                })
            {
                return Err(Error::Configuration(
                    "workload amendment metadata is invalid or exceeds its ceiling",
                ));
            }
        }
        let mut budget = Budget {
            limits,
            metadata: 0,
            sources: 0,
        };
        budget.metadata(std::mem::size_of_val(
            case.contract().cost().unwrap_or_default(),
        ))?;
        for text in case
            .contract()
            .witnesses()
            .iter()
            .flatten()
            .chain(case.contract().required_symbols())
            .chain(case.contract().notes())
        {
            budget.metadata(text.len())?;
        }
        let default_contract = case.contract().clone();
        let entry = budget.text(entry)?;
        let mut sources = reserve(case.transitive_source_paths().len())?;
        for path in case.transitive_source_paths() {
            let source = corpus
                .files()
                .iter()
                .find(|source| source.path() == path)
                .ok_or(Error::Configuration(
                    "workload source closure is incomplete",
                ))?;
            budget.source(source.source().len())?;
            let edits = constants::edits(source, amendments, &mut budget)?;
            let remaining = limits.total_source_bytes - budget.sources;
            let derived = examples::derive_source(
                source.source(),
                &edits,
                limits.source_bytes.min(remaining),
            )
            .map_err(Error::Corpus)?;
            budget.source(derived.len())?;
            let digest = format!("{:x}", Sha256::digest(derived.as_bytes()));
            sources.push(Source {
                path: budget.text(path)?,
                base_sha256: budget.text(source.source_sha256())?,
                derived_sha256: budget.text(&digest)?,
                derived_bytes: derived.len(),
                edits,
            });
        }
        let mut hash = Sha256::new();
        hash.update(b"zetesis-workload-v1\0");
        field(&mut hash, entry.as_bytes());
        field(&mut hash, corpus.manifest_sha256().as_bytes());
        for source in &sources {
            field(&mut hash, source.path.as_bytes());
            field(&mut hash, source.base_sha256.as_bytes());
            field(&mut hash, source.derived_sha256.as_bytes());
        }
        let identity = budget.text(&format!("{:x}", hash.finalize()))?;
        Ok(Self {
            entry,
            identity,
            manifest_sha256: Some(corpus.manifest_sha256()),
            amended: !amendments.is_empty(),
            sources,
            default_contract,
            generated: None,
            metadata_bytes: budget.metadata,
            source_bytes: budget.sources,
        })
    }

    /// Admit the generated program of `family` at `size` as a workload.
    ///
    /// The entry path is `generated/<family>-<size>.lp`; the program's bytes
    /// are bounded by the source ceiling and its digest is retained, so that
    /// materialization can check that the generator still produces them.
    ///
    /// # Errors
    /// Refuses a size outside the family's range or a program above the
    /// source ceiling.
    pub fn generated(family: Family, size: u32, limits: WorkloadLimits) -> Result<Self, Error> {
        let source = family
            .source(size)
            .map_err(|_| Error::Configuration("generated workload size is not admitted"))?;
        let contract = family
            .contract(size)
            .map_err(|_| Error::Configuration("generated workload size is not admitted"))?;
        let mut budget = Budget {
            limits,
            metadata: 0,
            sources: 0,
        };
        budget.source(source.len())?;
        let digest = format!("{:x}", Sha256::digest(source.as_bytes()));
        let entry = budget.text(&Generated::entry_of(family, size))?;
        let mut hash = Sha256::new();
        hash.update(b"zetesis-generated-workload-v1\0");
        field(&mut hash, family.label().as_bytes());
        field(&mut hash, &size.to_le_bytes());
        field(&mut hash, digest.as_bytes());
        let identity = budget.text(&format!("{:x}", hash.finalize()))?;
        let sha256 = budget.text(&digest)?;
        Ok(Self {
            entry,
            identity,
            manifest_sha256: None,
            sources: Vec::new(),
            default_contract: contract,
            amended: false,
            generated: Some(Generated {
                family,
                size,
                sha256,
                bytes: source.len(),
            }),
            metadata_bytes: budget.metadata,
            source_bytes: budget.sources,
        })
    }

    /// Original manifest-relative entry path; distinct workloads may share it.
    #[must_use]
    pub fn entry(&self) -> &str {
        &self.entry
    }

    /// Human label with the generated size or recorded constant replacements.
    ///
    /// This allocates only presentation text from retained metadata. Labels are
    /// descriptive, not authenticated identities; use [`Self::identity`] when
    /// comparing workloads from different reports.
    #[must_use]
    pub fn label(&self) -> String {
        workload_label(
            &self.entry,
            self.is_generated(),
            self.sources.iter().flat_map(|source| {
                source
                    .edits
                    .iter()
                    .map(|edit| (edit.before(), edit.after()))
            }),
        )
    }

    /// Content identity including the entry and original/derived source closure.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Whether explicit amendments were requested.
    #[must_use]
    pub const fn is_amended(&self) -> bool {
        self.amended
    }

    /// Whether this workload is a generated program rather than a corpus entry.
    #[must_use]
    pub const fn is_generated(&self) -> bool {
        self.generated.is_some()
    }

    /// The contract the campaign checks: the corpus contract of an unchanged
    /// entry, the closed-form contract of a generated program, and none for
    /// an amended entry, whose family is established by the reference alone.
    #[must_use]
    pub fn contract(&self) -> Option<&examples::Contract> {
        (!self.amended).then_some(&self.default_contract)
    }

    pub(super) fn validate(
        &self,
        corpus: &examples::Corpus,
        limits: super::super::Limits,
    ) -> Result<(), Error> {
        if let Some(generated) = &self.generated {
            generated.regenerate()?;
            if self.entry != generated.entry()
                || generated.bytes > limits.corpus.source_bytes
                || self.metadata_bytes > limits.corpus.manifest_bytes
            {
                return Err(Error::Configuration(
                    "generated workload differs from its admitted program",
                ));
            }
            return Ok(());
        }
        let case = corpus
            .cases()
            .iter()
            .find(|case| case.path() == self.entry)
            .ok_or(Error::Configuration(
                "workload entry differs from the request corpus",
            ))?;
        if self.manifest_sha256 != Some(corpus.manifest_sha256())
            || self.default_contract != *case.contract()
            || self.sources.len() != case.transitive_source_paths().len()
            || self.metadata_bytes > limits.corpus.manifest_bytes
        {
            return Err(Error::Configuration(
                "workload provenance differs from the request corpus",
            ));
        }
        for (source, path) in self.sources.iter().zip(case.transitive_source_paths()) {
            let original = corpus
                .files()
                .iter()
                .find(|original| original.path() == path)
                .ok_or(Error::Configuration("workload base source is missing"))?;
            if source.path != *path
                || source.base_sha256 != original.source_sha256()
                || source.derived_bytes > limits.corpus.source_bytes
            {
                return Err(Error::Configuration(
                    "workload source identity or size differs",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn materialize(
        &self,
        corpus: &examples::Corpus,
        directory: &Path,
        limit: usize,
    ) -> Result<Vec<crate::selected::FileSeal>, Error> {
        if let Some(generated) = &self.generated {
            let source = generated.regenerate()?;
            let path = directory.join(&self.entry);
            let parent = path
                .parent()
                .ok_or(Error::Configuration("generated workload has no parent"))?;
            std::fs::create_dir_all(parent).map_err(|error| super::super::io(parent, error))?;
            std::fs::write(&path, source).map_err(|error| super::super::io(&path, error))?;
            return Ok(vec![super::super::run::checked_seal(
                &path,
                limit,
                &generated.sha256,
            )?]);
        }
        let mut sealed = reserve(self.sources.len())?;
        for source in &self.sources {
            let original = corpus
                .files()
                .iter()
                .find(|original| original.path() == source.path)
                .ok_or(Error::Configuration("workload base source is missing"))?;
            let derived = examples::derive_source(original.source(), &source.edits, limit)
                .map_err(Error::Corpus)?;
            if derived.len() != source.derived_bytes
                || format!("{:x}", Sha256::digest(derived.as_bytes())) != source.derived_sha256
            {
                return Err(Error::Configuration(
                    "workload derivation differs from its admitted hash",
                ));
            }
            let path = directory.join(&source.path);
            let parent = path
                .parent()
                .ok_or(Error::Configuration("workload source has no parent"))?;
            std::fs::create_dir_all(parent).map_err(|error| super::super::io(parent, error))?;
            std::fs::write(&path, derived).map_err(|error| super::super::io(&path, error))?;
            sealed.push(super::super::run::checked_seal(
                &path,
                limit,
                &source.derived_sha256,
            )?);
        }
        Ok(sealed)
    }
}

// Both typed campaign summaries and retained-report views use the same label
// format. The latter supplies decoded edits without rebuilding a Workload.
pub(crate) fn workload_label<'a>(
    entry: &str,
    generated: bool,
    edits: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let path = entry.strip_suffix(".lp").unwrap_or(entry);
    if generated {
        return path.rsplit('/').next().unwrap_or(path).to_owned();
    }
    let components: Vec<&str> = path.rsplit('/').take(2).collect();
    let mut label = components.into_iter().rev().collect::<Vec<_>>().join("/");
    for (before, after) in edits {
        label.push(' ');
        label.push_str(before);
        label.push('→');
        label.push_str(after);
    }
    label
}

fn field(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u128).to_le_bytes());
    hash.update(value);
}

fn add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right)
        .ok_or(Error::Configuration("workload byte count overflow"))
}

fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Error::Configuration("workload allocation failed"))?;
    Ok(result)
}

struct Budget {
    limits: WorkloadLimits,
    metadata: usize,
    sources: usize,
}
impl Budget {
    fn metadata(&mut self, bytes: usize) -> Result<(), Error> {
        let next = add(self.metadata, bytes)?;
        if next > self.limits.metadata_bytes {
            return Err(Error::Configuration(
                "workload metadata exceeds its byte ceiling",
            ));
        }
        self.metadata = next;
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<String, Error> {
        self.metadata(text.len())?;
        let mut result = String::new();
        result
            .try_reserve_exact(text.len())
            .map_err(|_| Error::Configuration("workload text allocation failed"))?;
        result.push_str(text);
        Ok(result)
    }
    fn source(&mut self, bytes: usize) -> Result<(), Error> {
        let next = add(self.sources, bytes)?;
        if bytes > self.limits.source_bytes || next > self.limits.total_source_bytes {
            return Err(Error::Configuration(
                "workload source exceeds its byte ceiling",
            ));
        }
        self.sources = next;
        Ok(())
    }
}
