//! Confined, bounded corpus reads and explicit SHA-256 identities.

use super::{Error, Resource, ceiling};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub(super) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
pub(super) fn canonical(path: &Path) -> Result<PathBuf, Error> {
    path.canonicalize().map_err(|source| io(path, source))
}
pub(super) fn relative(text: &str) -> Result<&Path, Error> {
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
        return Err(Error::Path(text.to_owned()));
    }
    Ok(path)
}
pub(super) fn confined(root: &Path, text: &str) -> Result<PathBuf, Error> {
    let actual = canonical(&root.join(relative(text)?))?;
    if !actual.starts_with(root) {
        return Err(Error::Path(text.to_owned()));
    }
    Ok(actual)
}
pub(super) fn read(path: &Path, maximum: usize, resource: Resource) -> Result<Vec<u8>, Error> {
    let file = std::fs::File::open(path).map_err(|source| io(path, source))?;
    let metadata = file.metadata().map_err(|source| io(path, source))?;
    if !metadata.is_file() {
        return Err(Error::Path(path.display().to_string()));
    }
    ceiling(resource, u128::from(metadata.len()), maximum)?;
    let cap = u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    file.take(cap)
        .read_to_end(&mut bytes)
        .map_err(|source| io(path, source))?;
    ceiling(resource, bytes.len() as u128, maximum)?;
    Ok(bytes)
}
pub(super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(super) fn digest(path: &str, bytes: &[u8], expected: &str) -> Result<(), Error> {
    let actual = hash(bytes);
    if actual != expected {
        return Err(Error::Digest {
            path: path.to_owned(),
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(())
}
