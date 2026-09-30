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

/// The repository's correctness suite, the curated examples the tests load.
#[must_use]
pub fn correctness() -> PathBuf {
    examples().join("correctness")
}

/// The vendored clingo 5.8.2 upstream sources.
#[must_use]
pub fn upstream() -> PathBuf {
    root().join("validation/upstream/clingo-5.8.2")
}

/// The kr-domains originals the correctness suite was adapted from.
#[must_use]
pub fn kr_domains() -> PathBuf {
    root().join("validation/corpus/kr-domains")
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
    fn the_correctness_suite_holds_its_manifest() {
        assert!(correctness().join("manifest.json").is_file());
    }

    #[test]
    fn the_upstream_sources_hold_the_curated_corpus() {
        assert!(upstream().join("curated/manifest.json").is_file());
    }

    #[test]
    fn the_kr_domains_originals_keep_their_license() {
        assert!(kr_domains().join("LICENSE").is_file());
    }
}
