//! Repository assurance, independent of solver execution and answer semantics.
//!
//! These APIs inspect retained records and qualification policy. They do not
//! establish that a recorded command ran, that a proof kernel accepted a theorem,
//! or that a physical device executed a shader. Execution remains a separate
//! boundary. All file reads have explicit inclusive resource ceilings.
#![forbid(unsafe_code)]

mod files;
mod json;
pub mod proofs;

use std::{fmt, io, path::PathBuf};

/// A refusal to establish a maintenance contract.
#[derive(Debug)]
pub enum Error {
    /// A filesystem operation failed at a named path.
    Io {
        /// Relevant path.
        path: PathBuf,
        /// Original error.
        source: io::Error,
    },
    /// Serialized input is malformed or contains duplicate object keys.
    Json(serde_json::Error),
    /// Input contradicts the declared record or policy convention.
    Invalid(String),
    /// An inclusive resource allowance was exceeded.
    Limit {
        /// Resource being counted.
        resource: &'static str,
        /// Inclusive ceiling.
        limit: usize,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Json(error) => error.fmt(f),
            Self::Invalid(reason) => f.write_str(reason),
            Self::Limit { resource, limit } => write!(f, "{resource} exceeds {limit}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}
fn require(condition: bool, message: impl Into<String>) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}
