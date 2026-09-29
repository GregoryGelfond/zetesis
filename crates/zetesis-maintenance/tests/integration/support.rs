//! Helpers shared by the integration test modules.

pub(crate) mod process;

use std::path::{Path, PathBuf};

/// The repository's root directory.
pub(crate) fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
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
