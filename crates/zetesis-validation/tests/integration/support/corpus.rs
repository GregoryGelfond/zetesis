//! The repository's correctness corpus, loaded under the default limits.

use std::path::Path;

use zetesis_validation::examples;

/// The corpus under examples/correctness, loaded under the default limits.
pub fn corpus() -> examples::Corpus {
    examples::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness"),
        examples::Limits::default(),
    )
    .unwrap()
}
