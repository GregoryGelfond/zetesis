//! Pure model-value JSON encoding vocabulary, independent of a CLI envelope.
//!
//! Schema 1 uses typed full atoms, preorder term nodes, separate shown channels
//! and descending priority/cost pairs. All integers are exact decimal JSON numbers;
//! consumers must preserve integers beyond JavaScript's exact Number range.
//! The version is supplied out of band; it adds no field to existing model values.

use std::fmt;

pub use super::view::{ViewError as Error, ViewLimits as Limits};

/// Version of the model-value representation, independent of a stream envelope.
pub const SCHEMA_VERSION: u32 = 1;

/// Charged encoding work, distinct from observation evaluation and publication.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Successful work charges; the refused next charge is excluded.
    pub work: u64,
    /// UTF-8 bytes buffered before success or refusal. On failure these were
    /// discarded, and must never be interpreted as published bytes.
    pub buffered_bytes: usize,
}

/// One complete private model-value record with its encoding accounting.
#[derive(Debug)]
pub struct Encoded {
    text: String,
    statistics: Statistics,
}
impl Encoded {
    pub(super) fn new(text: String, statistics: Statistics) -> Self {
        Self { text, statistics }
    }
    /// Complete schema-1 model-value text; no external bytes have been written.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Consume the record without copying its text.
    #[must_use]
    pub fn into_text(self) -> String {
        self.text
    }
    /// Charged work for this encoding alone.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}

/// Typed refusal and retained accounting; no incomplete record escapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    cause: Error,
    statistics: Statistics,
}
impl Failure {
    pub(super) const fn new(cause: Error, statistics: Statistics) -> Self {
        Self { cause, statistics }
    }
    /// Authoritative typed encoding cause.
    #[must_use]
    pub const fn cause(&self) -> Error {
        self.cause
    }
    /// Charged work and discarded private bytes before refusal.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for Failure {}
