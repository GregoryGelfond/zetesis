//! Deterministic inventory of maintained Rust source roots. Historical evidence,
//! vendor imports and package build outputs are outside the selected roots.
//! A directory named target inside a source root remains authored source.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_SOURCE_BYTES: u64 = 1_048_576;
const MAX_SOURCE_FILES: usize = 4_096;
const MAX_DIRECTORY_ENTRIES: usize = 32_768;
const MAX_DIRECTORY_DEPTH: usize = 64;

/// Workspace packages follow the repository's crates/* membership. The three
/// maintained standalone packages are explicit. Each package contributes only
/// src, tests, benches, examples and its optional build.rs. Shared manual examples
/// are maintained source too, even though they live outside package directories.
pub(super) fn inventory(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut walk = Inventory::default();
    let mut packages = Vec::new();
    for entry in walk.entries(&root.join("crates"))? {
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::other("symlink in workspace package inventory"));
        }
        if kind.is_dir() {
            packages.push(entry.path());
        }
    }
    packages.extend([
        root.join("validation/reference"),
        root.join("experiments/gate-transfer"),
        root.join("refinement/membership/rust"),
    ]);
    packages.sort();
    for package in packages {
        if !package.join("Cargo.toml").is_file() {
            return Err(io::Error::other(format!(
                "missing maintained package manifest: {}",
                package.display()
            )));
        }
        for directory in ["src", "tests", "benches", "examples"] {
            let source = package.join(directory);
            if source.try_exists()? {
                walk.directory(&source)?;
            }
        }
        let build = package.join("build.rs");
        if build.try_exists()? {
            if !fs::symlink_metadata(&build)?.is_file() {
                return Err(io::Error::other("build.rs is not a regular source file"));
            }
            walk.source(build)?;
        }
    }
    walk.directory(&root.join("docs/book/examples"))?;
    walk.sources.sort();
    Ok(walk.sources)
}

/// Read one byte beyond the ceiling to detect growth without unbounded capture.
pub(super) fn read(path: &Path) -> io::Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u128 > u128::from(MAX_SOURCE_BYTES) {
        return Err(io::Error::other("authored source byte limit"));
    }
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[derive(Default)]
struct Inventory {
    entries: usize,
    sources: Vec<PathBuf>,
}

impl Inventory {
    fn entries(&mut self, path: &Path) -> io::Result<Vec<fs::DirEntry>> {
        if !fs::symlink_metadata(path)?.is_dir() {
            return Err(io::Error::other("source root is not an ordinary directory"));
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            if self.entries >= MAX_DIRECTORY_ENTRIES {
                return Err(io::Error::other("authored directory entry limit"));
            }
            self.entries += 1;
            entries.push(entry?);
        }
        entries.sort_by_key(fs::DirEntry::file_name);
        Ok(entries)
    }

    fn directory(&mut self, root: &Path) -> io::Result<()> {
        let mut pending = vec![(root.to_owned(), 0)];
        while let Some((path, depth)) = pending.pop() {
            for entry in self.entries(&path)? {
                let path = entry.path();
                let kind = entry.file_type()?;
                if kind.is_symlink() {
                    return Err(io::Error::other(format!(
                        "symlink in authored source inventory: {}",
                        path.display()
                    )));
                }
                if kind.is_dir() {
                    if depth >= MAX_DIRECTORY_DEPTH {
                        return Err(io::Error::other("authored directory depth limit"));
                    }
                    pending.push((path, depth + 1));
                } else if kind.is_file() && path.extension().is_some_and(|value| value == "rs") {
                    self.source(path)?;
                }
            }
        }
        Ok(())
    }

    fn source(&mut self, path: PathBuf) -> io::Result<()> {
        if self.sources.len() >= MAX_SOURCE_FILES {
            return Err(io::Error::other("authored source file limit"));
        }
        self.sources.push(path);
        Ok(())
    }
}
