//! The repository's directories that tests read in place.

use std::path::{Path, PathBuf};

/// The repository's root directory.
#[must_use]
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The repository's examples directory.
#[must_use]
pub fn examples() -> PathBuf {
    root().join("examples")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_holds_the_workspace_manifest() {
        let manifest = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
        assert!(manifest.lines().any(|line| line == "[workspace]"));
    }

    #[test]
    fn the_examples_directory_holds_the_correctness_suite() {
        assert!(examples().join("correctness/manifest.json").is_file());
    }
}
