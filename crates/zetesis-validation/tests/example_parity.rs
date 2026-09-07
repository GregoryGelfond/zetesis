//! Opt-in bounded clingo comparison of original and comment-cleaned sources.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use zetesis_validation::{answers, examples, process};

fn run(clingo: &Path, root: &Path, source: &str) -> answers::ReportedAnswers {
    let arguments: Vec<OsString> = ["--models=0", "--outf=2", "--opt-mode=optN", source]
        .into_iter()
        .map(Into::into)
        .collect();
    let outcome = process::invoke(
        process::Invocation {
            executable: clingo,
            arguments: &arguments,
            directory: root,
        },
        process::Limits {
            timeout: Duration::from_mins(1),
            max_output_bytes: 8 * 1024 * 1024,
            cleanup_timeout: Duration::from_secs(2),
        },
    )
    .unwrap();
    let (capture, pending) = outcome.into_parts();
    if let Some(child) = pending {
        let cleanup = child.retry(Duration::from_secs(2));
        if let Some(child) = cleanup.pending {
            panic!(
                "example {source}: cleanup abandoned child {}",
                child.abandon()
            );
        }
    }
    assert_eq!(
        capture.stop(),
        process::Stop::Completed,
        "{source}: {capture:?}"
    );
    assert!(capture.failure().is_none(), "{source}: {capture:?}");
    assert!(capture.cleanup_failure().is_none(), "{source}: {capture:?}");
    let exit = capture.exit().unwrap();
    assert_eq!(exit.signal, None, "{source}");
    assert!(
        matches!(exit.code, Some(10 | 20 | 30)),
        "{source}: {capture:?}"
    );
    answers::clingo_json(capture.stdout(), answers::Limits::default()).unwrap()
}

#[test]
#[ignore = "requires CLINGO as an absolute clingo 5.8.x executable path"]
fn original_and_clean_selected_displays_agree() {
    let clingo = PathBuf::from(
        std::env::var_os("CLINGO").expect("set CLINGO to an absolute executable path"),
    );
    assert!(clingo.is_absolute());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let originals = repo.join("validation/corpus/kr-domains");
    let corpus = examples::load(
        &repo.join("examples/kr-domains"),
        examples::Limits::default(),
    )
    .unwrap();
    examples::verify_originals(&corpus, &originals, examples::Limits::default()).unwrap();
    let mut models = 0_u64;
    for case in corpus.cases() {
        let original = run(&clingo, &originals, case.path());
        let cleaned = run(&clingo, corpus.root(), case.path());
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
