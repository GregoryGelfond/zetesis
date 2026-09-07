//! Immutable selected sources verified through the Rust corpus library.

use std::path::Path;
use std::sync::OnceLock;
use zetesis_validation::curated::{self, Corpus, Limits};

pub fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../validation/upstream/clingo-5.8.2/curated");
        curated::open(&root, Limits::default()).expect("sealed curated upstream corpus")
    })
}
