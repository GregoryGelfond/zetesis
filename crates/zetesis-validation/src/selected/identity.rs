//! Bounded primary-file identities; no filesystem lease or dependency attestation.
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::Error;

/// A primary input's requested path, resolved file identity and exact byte seal.
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

pub(super) fn absolute(path: &Path) -> Result<PathBuf, Error> {
    let path = std::path::absolute(path).map_err(|source| io(path, source))?;
    if path.to_str().is_none() {
        return Err(Error::Path {
            path,
            detail: "report paths require lossless UTF-8 representation",
        });
    }
    Ok(path)
}

pub(super) fn seal(path: &Path, limit: usize) -> Result<FileSeal, Error> {
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
    let mut digest = Sha256::new();
    let mut bytes = 0usize;
    let mut buffer = [0_u8; 8192];
    loop {
        let room = limit
            .saturating_sub(bytes)
            .saturating_add(1)
            .min(buffer.len());
        let count = file
            .read(&mut buffer[..room])
            .map_err(|source| io(&canonical, source))?;
        if count == 0 {
            break;
        }
        bytes = bytes.checked_add(count).ok_or_else(|| Error::Bytes {
            path: canonical.clone(),
            limit,
        })?;
        if bytes > limit {
            return Err(Error::Bytes {
                path: canonical,
                limit,
            });
        }
        digest.update(&buffer[..count]);
    }
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
        bytes: u64::try_from(bytes).map_err(|_| Error::Bytes {
            path: path.to_owned(),
            limit,
        })?,
        sha256: format!("{:x}", digest.finalize()),
        device,
        inode,
        limit,
    })
}

pub(super) fn recheck(before: &FileSeal) -> Change {
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

pub(super) fn aliases(left: &FileSeal, right: &FileSeal) -> bool {
    left.canonical == right.canonical
        || (left.device.is_some() && left.device == right.device && left.inode == right.inode)
}

pub(super) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
