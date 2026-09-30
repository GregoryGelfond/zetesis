//! Immutable selected sources verified through the Rust corpus library.

use std::sync::OnceLock;
use zetesis_test_support::repository;
use zetesis_validation::curated::{self, Corpus, Limits};

pub fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let root = repository::upstream().join("curated");
        curated::open(&root, Limits::default()).expect("sealed curated upstream corpus")
    })
}
