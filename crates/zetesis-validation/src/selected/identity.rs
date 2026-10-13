//! Bounded primary-file identities; no filesystem lease or dependency attestation.
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::Error;
use crate::corpus_files::spelled;

/// A primary input's requested path, resolved file identity and exact byte seal.
/// The bounded read must agree with the opened file's length before and after
/// reading. This consistency check does not establish an immutable snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileSeal {
    requested: PathBuf,
    canonical: PathBuf,
    bytes: u64,
    sha256: String,
    device: Option<u64>,
    inode: Option<u64>,
    #[serde(skip)]
    limit: usize,
}
impl FileSeal {
    /// Absolute path requested for this input, preserving symlink traversal.
    #[must_use]
    pub fn requested(&self) -> &Path {
        &self.requested
    }
    /// Canonical path resolved at this check.
    #[must_use]
    pub fn canonical(&self) -> &Path {
        &self.canonical
    }
    /// SHA-256 of the primary file's exact contents.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    /// Number of bytes hashed.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Compare observed file identity independently of each read's byte ceiling.
    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        self.requested == other.requested
            && self.canonical == other.canonical
            && self.bytes == other.bytes
            && self.sha256 == other.sha256
            && self.device == other.device
            && self.inode == other.inode
    }
}

/// One after-run input check, retaining failure instead of inventing an identity.
#[derive(Debug, Serialize)]
pub struct Change {
    path: PathBuf,
    unchanged: bool,
    after: Option<FileSeal>,
    error: Option<String>,
}
impl Change {
    /// Whether requested/resolved path, file identity and byte seal agree.
    #[must_use]
    pub const fn unchanged(&self) -> bool {
        self.unchanged
    }
    /// Observed post-run seal, absent when the recheck failed.
    #[must_use]
    pub const fn after(&self) -> Option<&FileSeal> {
        self.after.as_ref()
    }
    /// Original diagnostic if the post-run input could not be checked.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

pub(crate) fn absolute(path: &Path) -> Result<PathBuf, Error> {
    let path = std::path::absolute(path).map_err(|source| io(path, source))?;
    if path.to_str().is_none() {
        return Err(Error::Path {
            path,
            detail: "report paths require lossless UTF-8 representation",
        });
    }
    Ok(path)
}

pub(crate) fn seal(path: &Path, limit: usize) -> Result<FileSeal, Error> {
    let requested = absolute(path)?;
    let canonical = std::fs::canonicalize(&requested).map_err(|source| io(&requested, source))?;
    if canonical.to_str().is_none() {
        return Err(Error::Path {
            path: canonical,
            detail: "canonical path requires lossless UTF-8 representation",
        });
    }
    let mut file = File::open(&canonical).map_err(|source| io(&canonical, source))?;
    let metadata = file.metadata().map_err(|source| io(&canonical, source))?;
    if !metadata.is_file() {
        return Err(Error::Path {
            path: canonical,
            detail: "sealed input must be a regular file",
        });
    }
    if u128::from(metadata.len()) > limit as u128 {
        return Err(Error::Bytes {
            path: canonical,
            limit,
        });
    }
    let (bytes, sha256) = content_seal(&mut file, &canonical, limit, metadata.len(), |file| {
        file.metadata().map(|metadata| metadata.len())
    })?;
    #[cfg(unix)]
    let (device, inode) = {
        use std::os::unix::fs::MetadataExt;
        (Some(metadata.dev()), Some(metadata.ino()))
    };
    #[cfg(not(unix))]
    let (device, inode) = (None, None);
    Ok(FileSeal {
        requested,
        canonical,
        bytes,
        sha256,
        device,
        inode,
        limit,
    })
}

/// Hash a bounded stream, then require its length to agree with both metadata
/// observations. Production obtains both lengths from the same open file.
fn content_seal<R: Read>(
    reader: &mut R,
    path: &Path,
    limit: usize,
    before: u64,
    after_length: impl FnOnce(&R) -> std::io::Result<u64>,
) -> Result<(u64, String), Error> {
    let mut digest = Sha256::new();
    let mut bytes = 0usize;
    let mut buffer = [0_u8; 8192];
    loop {
        let room = limit
            .saturating_sub(bytes)
            .saturating_add(1)
            .min(buffer.len());
        let count = reader
            .read(&mut buffer[..room])
            .map_err(|source| io(path, source))?;
        if count == 0 {
            break;
        }
        bytes = bytes.checked_add(count).ok_or_else(|| Error::Bytes {
            path: path.to_owned(),
            limit,
        })?;
        if bytes > limit {
            return Err(Error::Bytes {
                path: path.to_owned(),
                limit,
            });
        }
        digest.update(&buffer[..count]);
    }
    let bytes = u64::try_from(bytes).map_err(|_| Error::Bytes {
        path: path.to_owned(),
        limit,
    })?;
    let after = after_length(reader).map_err(|source| io(path, source))?;
    if bytes != before || bytes != after {
        let kind = if bytes < before {
            std::io::ErrorKind::UnexpectedEof
        } else {
            std::io::ErrorKind::InvalidData
        };
        return Err(io(
            path,
            std::io::Error::new(
                kind,
                format!(
                    "sealed input length disagrees: before {before} bytes, read {bytes} bytes, after {after} bytes"
                ),
            ),
        ));
    }
    Ok((bytes, spelled(&digest.finalize())))
}

pub(crate) fn recheck(before: &FileSeal) -> Change {
    match seal(&before.requested, before.limit) {
        Ok(after) => Change {
            path: before.requested.clone(),
            unchanged: &after == before,
            after: Some(after),
            error: None,
        },
        Err(error) => Change {
            path: before.requested.clone(),
            unchanged: false,
            after: None,
            error: Some(error.to_string()),
        },
    }
}

/// What a native run's statistics say about the executable that did its work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Named {
    /// The report carries no statistics object, so it names nothing the rule
    /// could settle.
    NoStatistics,
    /// No `executable` key: a build that predates the key, which arrived with
    /// the default install's front, so it cannot have handed the run off.
    Omitted,
    /// `null`: the run could not resolve its own executable's path.
    Unresolved,
    /// The executable's path, resolved through links.
    Path(PathBuf),
}

/// What a native run's report names as its executable, read from the report's
/// `statistics` value: absent or not an object, the report names nothing.
pub(crate) fn named(statistics: &serde_json::Value) -> Named {
    let Some(statistics) = statistics.as_object() else {
        return Named::NoStatistics;
    };
    match statistics.get("executable") {
        None => Named::Omitted,
        Some(value) => value
            .as_str()
            .map_or(Named::Unresolved, |path| Named::Path(PathBuf::from(path))),
    }
}

/// The executables a report's native runs name, settled against the native
/// executable it sealed and invoked. A run naming the native executable, or
/// naming none because it predates the key, changes nothing. The first other
/// executable a run names, as when the default install's front hands a GPU run
/// to its sibling, is sealed at that report; later runs may name the native
/// executable or that one.
#[derive(Debug)]
pub(crate) struct Executables {
    native: PathBuf,
    other: Option<FileSeal>,
    limit: usize,
}

impl Executables {
    /// The rule for runs of the native executable sealed as `native`, sealing
    /// another within `limit` bytes.
    pub(crate) fn new(native: &FileSeal, limit: usize) -> Self {
        Self {
            native: native.canonical().to_owned(),
            other: None,
            limit,
        }
    }

    /// Settle what one native run's statistics name.
    ///
    /// # Errors
    /// Returns why the run's evidence is refused: its report carries no
    /// statistics object to name its executable; it could not resolve its own
    /// executable, which after a hand-off would be the sibling; the executable
    /// it names could not be sealed; or it names a third executable, beside the
    /// native one and the other one sealed earlier.
    pub(crate) fn settle(&mut self, named: &Named) -> Result<(), String> {
        let named = match named {
            Named::NoStatistics => {
                return Err(
                    "the native report carries no statistics to name its executable".to_owned(),
                );
            }
            Named::Omitted => return Ok(()),
            Named::Unresolved => {
                return Err("this run could not name the executable that did its work".to_owned());
            }
            Named::Path(path) if *path == self.native => return Ok(()),
            Named::Path(path) => path,
        };
        match &self.other {
            None => {
                self.other = Some(seal(named, self.limit).map_err(|error| {
                    format!(
                        "the executable this run names, {}, could not be sealed: {error}",
                        named.display()
                    )
                })?);
                Ok(())
            }
            Some(seal) if seal.canonical() == named => Ok(()),
            Some(seal) => Err(format!(
                "this run names {}, but an earlier run named {} besides the native {}",
                named.display(),
                seal.canonical().display(),
                self.native.display()
            )),
        }
    }

    /// The other executable runs named, as sealed at its first report, and
    /// its recheck now; both absent when no run named one. The value names no
    /// other executable afterwards.
    pub(crate) fn close(&mut self) -> (Option<FileSeal>, Option<Change>) {
        let other = self.other.take();
        let after = other.as_ref().map(recheck);
        (other, after)
    }
}

pub(crate) fn aliases(left: &FileSeal, right: &FileSeal) -> bool {
    left.canonical == right.canonical
        || (left.device.is_some() && left.device == right.device && left.inode == right.inode)
}

pub(crate) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests;
