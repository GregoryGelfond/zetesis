//! Exact selected clingo sources, provenance, and complete model contracts.
//!
//! Normal verification reads only the curated manifest, license, and `.lp`
//! files. The explicit legacy importer additionally checks preserved originals
//! and their restricted C++ literal spelling. Neither operation runs a solver.

mod document;
mod files;
mod contracts;
mod cpp;
mod import;

use std::fmt;
use std::path::{Path, PathBuf};

pub use document::{Case, Contract, Corpus, Origin, Provenance};
pub use import::import_legacy;

/// Immutable curated target identity, independent of native admission labels.
pub const MANIFEST_SHA256: &str =
    "305341267f879edf28df759af132dc00dbb0aba88da083adc73ee4cc2b1d9545";
/// Original clingo revision of these selected assertions.
pub const UPSTREAM_REVISION: &str = "a99ffb2a58293c68b28fcc283a1d1c9ccad900fe";
pub(super) const CASE_COUNT: usize = 24;
pub(super) const LICENSE_SHA256: &str =
    "8de01c48f0adf249cd631e7320fbf4f15b6f5c25ddb18cdf1410c95c9f0c445d";

/// Inclusive serialized-input and retained-source ceilings. Zero means zero.
/// JSON allocations are bounded by the manifest byte ceiling; these measures
/// do not claim allocator overhead or RSS. Import reads originals sequentially
/// and retains the three pinned original texts while checking assertions.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum bytes in either manifest or legacy JSONL catalog.
    pub manifest_bytes: usize,
    /// Maximum bytes in one decoded source file.
    pub source_bytes: usize,
    /// Maximum combined bytes of retained decoded source files.
    pub total_source_bytes: usize,
    /// Maximum bytes in one original C++ file or the license.
    pub original_bytes: usize,
    /// Maximum cases before processing their source files.
    pub cases: usize,
    /// Maximum recorded full-model occurrences across the corpus.
    pub models: usize,
    /// Maximum atom occurrences across all recorded full models.
    pub atoms: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            manifest_bytes: 1_048_576,
            source_bytes: 65_536,
            total_source_bytes: 1_048_576,
            original_bytes: 1_048_576,
            cases: 24,
            models: 4_096,
            atoms: 65_536,
        }
    }
}

/// A precisely scoped corpus integrity resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Serialized manifest/catalog bytes.
    ManifestBytes,
    /// Bytes of one decoded source.
    SourceBytes,
    /// Combined retained source bytes.
    TotalSourceBytes,
    /// Bytes of one original authority file.
    OriginalBytes,
    /// Case records.
    Cases,
    /// Full-model occurrences, retaining multiplicities.
    Models,
    /// Atom occurrences in full-model records.
    Atoms,
}

/// A refusal to establish integrity, never a semantic solver verdict.
#[derive(Debug)]
pub enum Error {
    /// Filesystem operation failed at the named path.
    Io {
        /// Relevant path.
        path: PathBuf,
        /// Underlying filesystem failure.
        source: std::io::Error,
    },
    /// Serialized data is outside the declared schema.
    Json(serde_json::Error),
    /// An inclusive resource ceiling was exceeded.
    Limit {
        /// Measured resource.
        resource: Resource,
        /// Requested amount.
        observed: u128,
        /// Inclusive allowance.
        limit: usize,
    },
    /// Source, license, authority, or manifest bytes differ from their seal.
    Digest {
        /// Named input.
        path: String,
        /// Required SHA-256.
        expected: String,
        /// Observed SHA-256.
        actual: String,
    },
    /// A relative path is not confined to the chosen root.
    Path(String),
    /// A case, provenance, or model contract is inconsistent.
    Contract(String),
    /// The restricted importer encountered unsupported C++ spelling.
    Literal(String),
    /// Import will not replace any existing destination.
    DestinationExists(PathBuf),
    /// Publication failed and removal of the new directory also failed.
    ImportCleanup {
        /// Newly created directory that may retain incomplete output.
        destination: PathBuf,
        /// Original publication or verification failure.
        failure: Box<Error>,
        /// Secondary cleanup failure, without replacing the original cause.
        cleanup: std::io::Error,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json(error) => write!(formatter, "corpus document: {error}"),
            Self::Limit {
                resource,
                observed,
                limit,
            } => write!(formatter, "corpus {resource:?} {observed} exceeds {limit}"),
            Self::Digest {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "{path}: SHA-256 {actual} differs from {expected}"
            ),
            Self::Path(path) => write!(formatter, "corpus path is not confined: {path}"),
            Self::Contract(reason) => write!(formatter, "corpus contract: {reason}"),
            Self::Literal(reason) => {
                write!(formatter, "unsupported C++ literal contract: {reason}")
            }
            Self::DestinationExists(path) => write!(
                formatter,
                "import destination already exists: {}",
                path.display()
            ),
            Self::ImportCleanup {
                destination,
                failure,
                cleanup,
            } => write!(
                formatter,
                "{failure}; cleanup of {} also failed: {cleanup}",
                destination.display()
            ),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(error) => Some(error),
            Self::ImportCleanup { failure, .. } => Some(failure.as_ref()),
            _ => None,
        }
    }
}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
pub(super) fn ceiling(resource: Resource, observed: u128, limit: usize) -> Result<(), Error> {
    if observed > limit as u128 {
        Err(Error::Limit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

/// Verify the pinned curated target and retain its exact UTF-8 source bytes.
/// Returned paths describe verified inputs; later filesystem changes do not
/// change the retained sources. No original C++ files or subprocesses are read.
///
/// # Errors
/// Refuses changed seals, malformed/duplicate contracts, unconfined paths,
/// invalid UTF-8, missing files, or exceeded inclusive ceilings.
pub fn open(root: &Path, limits: Limits) -> Result<Corpus, Error> {
    let root = files::canonical(root)?;
    let bytes = files::read(
        &files::confined(&root, "manifest.json")?,
        limits.manifest_bytes,
        Resource::ManifestBytes,
    )?;
    files::digest("manifest.json", &bytes, MANIFEST_SHA256)?;
    let document: document::Document = serde_json::from_slice(&bytes)?;
    document::load(root, document, limits)
}

#[cfg(test)]
mod tests;
