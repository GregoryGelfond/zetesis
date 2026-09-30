//! Confined, bounded reading; recorded paths are identities, never traversal instructions.
use crate::{Error, require};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

/// Inclusive serialized-input and directory traversal ceilings.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Bytes read from one file, excluding allocator overhead.
    pub file_bytes: usize,
    /// Total bytes read, including repeated reads of the same file.
    pub total_bytes: usize,
    /// Directory entries visited during recursive inventory.
    pub entries: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            file_bytes: 67_108_864,
            total_bytes: 536_870_912,
            entries: 16_384,
        }
    }
}
pub(crate) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) struct Tree {
    root: PathBuf,
    limits: Limits,
    remaining: usize,
}
impl Tree {
    pub(crate) fn new(root: &Path, limits: Limits) -> Result<Self, Error> {
        let root = root.canonicalize().map_err(|error| io(root, error))?;
        require(root.is_dir(), "record root must be a directory")?;
        Ok(Self {
            root,
            limits,
            remaining: limits.total_bytes,
        })
    }
    pub(crate) fn member(&self, relative: &str) -> Result<PathBuf, Error> {
        require(
            !relative.is_empty()
                && !relative.starts_with('/')
                && !relative.contains('\\')
                && relative
                    .split('/')
                    .all(|part| !matches!(part, "" | "." | "..")),
            format!("noncanonical record path: {relative}"),
        )?;
        let path = self.root.join(relative);
        let canonical = path.canonicalize().map_err(|error| io(&path, error))?;
        require(
            canonical.starts_with(&self.root),
            format!("record path escapes root: {relative}"),
        )?;
        require(
            canonical.is_file(),
            format!("missing regular record file: {relative}"),
        )?;
        Ok(canonical)
    }
    pub(crate) fn read(&mut self, relative: &str) -> Result<Vec<u8>, Error> {
        let path = self.member(relative)?;
        let limit = self.limits.file_bytes.min(self.remaining);
        let bound = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(|error| io(&path, error))?
            .take(bound)
            .read_to_end(&mut bytes)
            .map_err(|error| io(&path, error))?;
        if bytes.len() > self.limits.file_bytes {
            return Err(Error::Limit {
                resource: "file bytes",
                limit: self.limits.file_bytes,
            });
        }
        if bytes.len() > self.remaining {
            return Err(Error::Limit {
                resource: "total read bytes",
                limit: self.limits.total_bytes,
            });
        }
        self.remaining -= bytes.len();
        Ok(bytes)
    }
    pub(crate) fn text(&mut self, relative: &str) -> Result<String, Error> {
        String::from_utf8(self.read(relative)?)
            .map_err(|_| Error::Invalid(format!("invalid UTF-8: {relative}")))
    }
    /// Whether `relative` names an entry under the root; a symbolic link is refused.
    pub(crate) fn exists(&self, relative: &str) -> Result<bool, Error> {
        let path = self.root.join(relative);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                require(
                    !metadata.file_type().is_symlink(),
                    format!("inventory cannot contain symbolic links: {relative}"),
                )?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(io(&path, error)),
        }
    }
    /// `directory` under the root, which must be a real directory.
    fn directory(&self, directory: &str) -> Result<PathBuf, Error> {
        let path = self.root.join(directory);
        let kind = fs::symlink_metadata(&path)
            .map_err(|error| io(&path, error))?
            .file_type();
        require(
            kind.is_dir() && !kind.is_symlink(),
            "inventory root must be a real directory",
        )?;
        Ok(path)
    }
    /// The subdirectories directly under `directory`, relative to the root;
    /// other entries are skipped and a symbolic link is refused.
    pub(crate) fn directories(&self, directory: &str) -> Result<BTreeSet<String>, Error> {
        let path = self.directory(directory)?;
        let mut result = BTreeSet::new();
        let mut remaining = self.limits.entries;
        for entry in fs::read_dir(&path).map_err(|error| io(&path, error))? {
            let entry = entry.map_err(|error| io(&path, error))?;
            remaining = remaining.checked_sub(1).ok_or(Error::Limit {
                resource: "directory entries",
                limit: self.limits.entries,
            })?;
            let kind = entry
                .file_type()
                .map_err(|error| io(&entry.path(), error))?;
            require(
                !kind.is_symlink(),
                "inventory cannot contain symbolic links",
            )?;
            if kind.is_dir() {
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::Invalid("non-UTF-8 inventory path".into()))?;
                result.insert(format!("{directory}/{name}"));
            }
        }
        Ok(result)
    }
    pub(crate) fn inventory(
        &self,
        directory: &str,
        suffix: &str,
    ) -> Result<BTreeSet<String>, Error> {
        let mut pending = vec![self.directory(directory)?];
        let mut result = BTreeSet::new();
        let mut remaining = self.limits.entries;
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).map_err(|error| io(&directory, error))? {
                let entry = entry.map_err(|error| io(&directory, error))?;
                remaining = remaining.checked_sub(1).ok_or(Error::Limit {
                    resource: "directory entries",
                    limit: self.limits.entries,
                })?;
                let path = entry.path();
                let kind = entry.file_type().map_err(|error| io(&path, error))?;
                require(
                    !kind.is_symlink(),
                    "inventory cannot contain symbolic links",
                )?;
                if kind.is_dir() {
                    pending.push(path);
                } else if path.to_string_lossy().ends_with(suffix) {
                    let relative = path
                        .strip_prefix(&self.root)
                        .expect("inventory starts inside root")
                        .to_str()
                        .ok_or_else(|| Error::Invalid("non-UTF-8 inventory path".into()))?
                        .replace(std::path::MAIN_SEPARATOR, "/");
                    self.member(&relative)?;
                    result.insert(relative);
                }
            }
        }
        Ok(result)
    }
}
