//! Clean, pinned correctness examples and contracts on solver-reported displays.
//!
//! Normal loading needs only the examples directory. An explicit provenance
//! audit checks the retained originals and removes exactly the recorded comment
//! lines, then applies recorded byte replacements. Include spellings remain
//! unchanged. Curated sources have distinct byte identities and physical line
//! numbers; provenance is not a semantic equivalence proof. Contracts describe
//! selected reported displays; they do not implement consequence computation.

mod contract;
mod document;
mod files;
mod originals;

use std::fmt;
use std::path::{Path, PathBuf};

pub use contract::{Contract, ContractMismatch, Family, Satisfiability};
pub use document::{Annotation, Case, Corpus, Edit, Include, ReferenceToolchain, Source};
pub use originals::{derive_source, verify_originals};

/// Seal of the complete cleaned-source and typed-contract manifest.
pub const MANIFEST_SHA256: &str =
    "b43df1adf17ae0c035f1e310a5c15345c26cbcad8b59596932627c46fd1c6958";
/// Pinned upstream revision, shared with the retained historical corpus.
pub const UPSTREAM_REVISION: &str = "38f0660ded448ed268c5a68759ceb0e2840dd497";
const ORIGINAL_MANIFEST_SHA256: &str =
    "a99dafc272fb0047c01f984e27bf22943f2aa5f9c8acf04e4ed1de6ac1a3fe88";
const SOURCE_COUNT: usize = 108;
const CASE_COUNT: usize = 94;

/// Inclusive serialized-input and retained-source ceilings; zero means zero.
/// JSON allocations are bounded by manifest bytes. These are acceptance limits,
/// not allocator overhead or process RSS estimates. Originals are read one at a
/// time and bounded separately from the retained clean sources.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Serialized manifest bytes.
    pub manifest_bytes: usize,
    /// Bytes in one source, original source, or license.
    pub source_bytes: usize,
    /// Combined bytes of all retained clean sources.
    pub total_source_bytes: usize,
    /// Source records, including shared schemas.
    pub files: usize,
    /// Runnable case records.
    pub cases: usize,
    /// Printed-symbol occurrences across all witness and required-symbol contracts.
    pub contract_symbols: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            manifest_bytes: 1_048_576,
            source_bytes: 1_048_576,
            total_source_bytes: 4_194_304,
            files: SOURCE_COUNT,
            cases: CASE_COUNT,
            contract_symbols: 65_536,
        }
    }
}

/// Quantity refused while establishing corpus integrity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Serialized manifest bytes.
    ManifestBytes,
    /// Bytes in one source or license.
    SourceBytes,
    /// Combined retained clean-source bytes.
    TotalSourceBytes,
    /// Source records.
    Files,
    /// Runnable cases.
    Cases,
    /// Printed-symbol occurrences in contracts.
    ContractSymbols,
}

/// A refusal to establish corpus integrity, never a solver verdict.
#[derive(Debug)]
pub enum Error {
    /// Storage for a bounded source derivation could not be reserved.
    Allocation(std::collections::TryReserveError),
    /// Filesystem operation failed.
    Io {
        /// Input path.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// Manifest does not obey its declared JSON schema.
    Json(serde_json::Error),
    /// A source is not UTF-8.
    Utf8(std::string::FromUtf8Error),
    /// An inclusive ceiling was exceeded.
    Limit {
        /// Refused quantity.
        resource: Resource,
        /// Observed quantity.
        observed: u128,
        /// Inclusive ceiling.
        limit: usize,
    },
    /// Bytes do not match the pinned identity.
    Digest {
        /// Relative input name.
        path: String,
        /// Required SHA-256.
        expected: String,
        /// Observed SHA-256.
        actual: String,
    },
    /// An input path is not confined to its selected root.
    Path(String),
    /// Provenance, dependencies, or a typed contract is inconsistent.
    Contract(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation(error) => write!(f, "examples source allocation: {error}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Json(error) => write!(f, "examples manifest: {error}"),
            Self::Utf8(error) => write!(f, "examples source: {error}"),
            Self::Limit {
                resource,
                observed,
                limit,
            } => {
                write!(f, "examples {resource:?} {observed} exceeds {limit}")
            }
            Self::Digest {
                path,
                expected,
                actual,
            } => {
                write!(f, "{path}: SHA-256 {actual} differs from {expected}")
            }
            Self::Path(path) => write!(f, "examples path is not confined: {path}"),
            Self::Contract(detail) => write!(f, "examples contract: {detail}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Json(error) => Some(error),
            Self::Utf8(error) => Some(error),
            _ => None,
        }
    }
}

fn ceiling(resource: Resource, observed: u128, limit: usize) -> Result<(), Error> {
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

/// Load and seal the self-contained examples, license, dependencies and contracts.
/// No upstream checkout, original corpus, external tool, or solver is consulted.
///
/// # Errors
/// Refuses changed bytes, unconfined paths, invalid metadata or exceeded limits.
pub fn load(root: &Path, limits: Limits) -> Result<Corpus, Error> {
    document::load(root, limits)
}
