//! Confined, bounded reads of a sealed corpus's files and their SHA-256
//! identities, shared by the curated and the self-contained example corpora.
//! Each corpus reports their failures in its own error type, through
//! [`Failure`], and reads through its own alias of [`Files`].

use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};

/// The failures a corpus's file reads report, in that corpus's error type.
pub(crate) trait Failure: Sized {
    /// The quantities the corpus bounds; one names each read's byte ceiling.
    type Resource: Copy;
    /// A filesystem operation failed at `path`.
    fn io(path: &Path, source: io::Error) -> Self;
    /// `text` does not name a file confined to the corpus.
    fn unconfined(text: String) -> Self;
    /// `observed` exceeds the inclusive allowance `limit` of `resource`.
    fn limit(resource: Self::Resource, observed: u128, limit: usize) -> Self;
    /// The bytes named `path` have the SHA-256 `actual`, not their seal.
    fn digest(path: String, expected: String, actual: String) -> Self;
}

/// The file operations of a corpus whose failures are `E`.
pub(crate) struct Files<E>(PhantomData<E>);

impl<E: Failure> Files<E> {
    /// `path` resolved through links.
    pub(crate) fn canonical(path: &Path) -> Result<PathBuf, E> {
        path.canonicalize().map_err(|source| E::io(path, source))
    }

    /// `text` as a relative path of normal components only, separated by `/`.
    pub(crate) fn relative(text: &str) -> Result<&Path, E> {
        let path = Path::new(text);
        // Backslashes and drive separators must not acquire platform-specific meaning.
        if text.is_empty()
            || text.contains(['\\', ':'])
            || text
                .split('/')
                .any(|part| part.is_empty() || matches!(part, "." | ".."))
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(E::unconfined(text.to_owned()));
        }
        Ok(path)
    }

    /// The file `text` names under the canonical `root`, resolved, which must
    /// remain under `root`.
    pub(crate) fn confined(root: &Path, text: &str) -> Result<PathBuf, E> {
        let actual = Self::canonical(&root.join(Self::relative(text)?))?;
        if !actual.starts_with(root) {
            return Err(E::unconfined(text.to_owned()));
        }
        Ok(actual)
    }

    /// The bytes of the regular file at `path`, at most `maximum` of them,
    /// whose excess is refused as `resource`.
    pub(crate) fn read(path: &Path, maximum: usize, resource: E::Resource) -> Result<Vec<u8>, E> {
        let file = std::fs::File::open(path).map_err(|source| E::io(path, source))?;
        let metadata = file.metadata().map_err(|source| E::io(path, source))?;
        if !metadata.is_file() {
            return Err(E::unconfined(path.display().to_string()));
        }
        ceiling(resource, u128::from(metadata.len()), maximum)?;
        let cap = u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1);
        let mut bytes = Vec::new();
        file.take(cap)
            .read_to_end(&mut bytes)
            .map_err(|source| E::io(path, source))?;
        ceiling(resource, bytes.len() as u128, maximum)?;
        Ok(bytes)
    }

    /// Whether `bytes`, named `path`, have the SHA-256 `expected`.
    pub(crate) fn digest(path: &str, bytes: &[u8], expected: &str) -> Result<(), E> {
        let actual = sha256(bytes);
        if actual != expected {
            return Err(E::digest(path.to_owned(), expected.to_owned(), actual));
        }
        Ok(())
    }
}

/// The SHA-256 of `bytes`, in lowercase hexadecimal, as corpus seals spell it.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn ceiling<E: Failure>(resource: E::Resource, observed: u128, limit: usize) -> Result<(), E> {
    if observed > limit as u128 {
        Err(E::limit(resource, observed, limit))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
