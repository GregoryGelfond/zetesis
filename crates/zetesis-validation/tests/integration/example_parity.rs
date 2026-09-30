//! Opt-in bounded clingo comparison of original and curated sources.

use std::path::Path;
use std::time::Duration;

use zetesis_clingo_support as oracle;
use zetesis_test_support::repository;
use zetesis_validation::{answers, examples};

fn run(root: &Path, source: &str) -> answers::ReportedAnswers {
    oracle::answers(&oracle::run_in(
        root,
        ["--models=0", "--outf=2", "--opt-mode=optN", source],
        &oracle::DECIDED,
        oracle::Limits {
            timeout: Duration::from_mins(1),
            max_output_bytes: 8 * 1024 * 1024,
        },
    ))
}

#[test]
#[ignore = "requires clingo: original and clean selected displays agree"]
fn original_and_clean_selected_displays_agree() {
    let repo = repository::root().canonicalize().unwrap();
    let originals = repo.join("validation/corpus/kr-domains");
    let corpus = examples::load(
        &repo.join("examples/correctness"),
        examples::Limits::default(),
    )
    .unwrap();
    examples::verify_originals(&corpus, &originals, examples::Limits::default()).unwrap();
    let mut models = 0_u64;
    for case in corpus.cases() {
        let original = run(&originals, case.path());
        let cleaned = run(corpus.root(), case.path());
        assert!(
            answers::same_displays(&original, &cleaned),
            "{}: original={original:?}, cleaned={cleaned:?}",
            case.path()
        );
        case.contract()
            .check(&cleaned)
            .unwrap_or_else(|error| panic!("{}: {error}", case.path()));
        models = models.checked_add(cleaned.model_count()).unwrap();
        println!(
            "pass: {} selected_models={} cost={:?}",
            case.path(),
            cleaned.model_count(),
            cleaned.cost()
        );
    }
    println!(
        "complete: {} cases, {} selected models per tree; displayed multiplicities, optimum ties, costs and typed contracts agree",
        corpus.cases().len(),
        models
    );
}
