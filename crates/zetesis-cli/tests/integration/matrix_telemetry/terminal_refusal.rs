//! Actual CLI resource refusal preserves only complete reconstructed answers.

use clap::Parser;
use serde_json::Value;
use zetesis_cli::{
    Completion, Options, Report, RunError, RunFailure, run_detailed_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{FormulaFailure, FormulaResource, ReconstructionError};
use zetesis_validation::answers::{self, native_json};

fn capture(work: Option<u64>) -> (Result<Report, RunFailure>, Vec<u8>) {
    let options = Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--grounder",
            "auto",
            "--search",
            "regions",
            "--workers",
            "1",
            "--completion-workers",
            "1",
            "--models",
            "0",
            "--json",
            "--stats",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain(
            work.into_iter()
                .flat_map(|limit| ["--max-expansion-work".to_owned(), limit.to_string()]),
        ),
    )
    .unwrap();
    let mut output = Vec::new();
    let result = run_detailed_with_diagnostics(
        "a(1) | b(1). c(X):-a(X). c(X):-b(X).".into(),
        &options,
        &mut output,
        &mut Vec::new(),
        &Cancellation::default(),
    );
    (result, output)
}

#[test]
fn work_refusal_retains_only_the_original_checked_prefix() {
    let (complete, output) = capture(None);
    let complete = complete.unwrap();
    assert_eq!(complete.completion, Completion::Exhausted);
    let receipt = complete.terminal_execution.unwrap();
    assert_eq!(
        (receipt.base_answers, receipt.reconstructed, receipt.pending),
        (2, 2, 0)
    );
    let decoded = native_json::parse(&output, native_json::Limits::default()).unwrap();
    let mut family = decoded.full_model_symbols(4096).unwrap();
    for model in &mut family {
        model.sort();
    }
    family.sort();
    assert_eq!(family, [vec!["a(1)", "c(1)"], vec!["b(1)", "c(1)"]]);
    let complete_document: Value = serde_json::from_slice(&output).unwrap();

    let ceiling = receipt.reconstruction.work.checked_sub(1).unwrap();
    let (refused, output) = capture(Some(ceiling));
    let refused = refused.unwrap_err();
    let RunError::Reconstruction(ReconstructionError::Source(cause)) = refused.cause.as_ref()
    else {
        panic!("expected a typed reconstruction source refusal: {refused:?}");
    };
    assert!(matches!(cause.as_ref(), FormulaFailure::Limit {
        resource: FormulaResource::Work, observed, limit, ..
    } if *limit == u128::from(ceiling) && *observed > *limit));
    let partial = refused.partial_report.as_ref().unwrap();
    assert_eq!((partial.published_models, partial.verified_models), (1, 1));
    assert_eq!(partial.completion, None);
    assert_eq!(partial.interruption, None);
    assert!(partial.summary_published);
    assert!(refused.secondary_output.is_none());
    let receipt = partial.terminal_execution.unwrap();
    assert_eq!(
        (receipt.base_answers, receipt.reconstructed, receipt.pending),
        (2, 1, 1)
    );
    assert_eq!(
        (
            receipt.reconstruction.attempts,
            receipt.reconstruction.completed
        ),
        (2, 1)
    );
    assert_eq!(receipt.reconstruction.work, ceiling);

    check_refused_publication(&output, &complete_document);
}

fn check_refused_publication(output: &[u8], complete_document: &Value) {
    let document: Value = serde_json::from_slice(output).unwrap();
    assert_eq!(document["outcome"]["status"], "failed");
    assert!(document["outcome"]["completion"].is_null());
    assert_eq!(document["outcome"]["coverage"], "unavailable");
    assert_eq!(document["outcome"]["verified_models"], 1);
    assert_eq!(document["outcome"]["published_models"], 1);
    // The successful public decoder authenticated these complete models and
    // their cumulative dictionary. Compare the exact serialized prefix, without
    // treating per-record atom additions as a standalone atom dictionary.
    assert_eq!(
        document["models"].as_array().unwrap(),
        &complete_document["models"].as_array().unwrap()[..1]
    );
    assert!(matches!(
        native_json::parse(output, native_json::Limits::default()),
        Err(answers::Error::Invalid {
            issue: answers::Issue::Incomplete,
            ..
        })
    ));
    // Matrix outcome controls separately establish that this exact typed code
    // classifies as Refused, distinct from allocator/identity InvocationFailure.
    assert_eq!(
        document["outcome"]["error"]["kind"],
        "answer_reconstruction_limit"
    );
    assert_eq!(document["statistics"]["search"]["scope"], "terminal_base");
    assert_eq!(document["statistics"]["terminal_execution"]["pending"], 1);
}
