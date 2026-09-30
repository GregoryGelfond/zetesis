//! Helpers shared by the integration test modules.

pub(crate) mod process;

use std::io;
use std::path::{Path, PathBuf};

use zetesis_maintenance::inventory::{self, Limits};

pub(crate) use zetesis_test_support::repository::root as repository;

/// The largest maintained source the audits read.
const MAX_SOURCE_BYTES: usize = 1_048_576;

/// The maintained Rust sources the library's authored-source inventory lists,
/// each under `root`.
pub(crate) fn authored_sources(root: &Path) -> Vec<PathBuf> {
    inventory::authored(root, Limits::default())
        .unwrap()
        .into_iter()
        .map(|relative| root.join(relative))
        .collect()
}

/// A maintained source's text, read through the library's bounded reader.
pub(crate) fn source(path: &Path) -> io::Result<String> {
    let bytes = inventory::read(path, MAX_SOURCE_BYTES).map_err(io::Error::other)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Every Markdown document of the repository, skipping hidden and build
/// directories, as (path relative to the repository, text).
pub(crate) fn documents() -> Vec<(String, String)> {
    fn visit(directory: &Path, root: &Path, documents: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            if path.is_dir() {
                if !name.starts_with('.') && name != "target" {
                    visit(&path, root, documents);
                }
            } else if path.extension().is_some_and(|extension| extension == "md") {
                let relative = path.strip_prefix(root).unwrap().display().to_string();
                documents.push((relative, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let root = repository().canonicalize().unwrap();
    let mut documents = Vec::new();
    visit(&root, &root, &mut documents);
    documents
}
