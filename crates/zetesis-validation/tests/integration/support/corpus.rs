//! The repository's correctness corpus, loaded under the default limits.

use zetesis_test_support::repository;
use zetesis_validation::examples;

/// The corpus under examples/correctness, loaded under the default limits.
pub fn corpus() -> examples::Corpus {
    examples::load(&repository::correctness(), examples::Limits::default()).unwrap()
}
