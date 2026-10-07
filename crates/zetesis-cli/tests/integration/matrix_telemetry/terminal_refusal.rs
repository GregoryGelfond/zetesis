//! Typed publication refusal preserves only complete reconstructed answers.

use clap::Parser;
use serde_json::Value;
use zetesis_cli::{
    Completion, JsonRenderer, Options, PublicationConfig, PublicationOutcome, PublicationReport,
    Report, RunError, RunFailure, publish_prepared,
};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{FormulaFailure, FormulaResource, ReconstructionError};
use zetesis_validation::answers::{self, native_json};

/// The answer b(1) is reconstructed first; a(1), reconstructed second, also
/// derives w(1,1) and w(1,2), so its reconstruction alone costs more.
const PROGRAM: &str = "n(1;2). a(1) | b(1). c(X):-a(X). c(X):-b(X). w(X,Y):-a(X),n(Y).";

fn capture(work: Option<u64>, models: &str) -> (Result<Report, RunFailure>, Vec<u8>) {
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--grounder",
        "auto",
        "--search",
        "regions",
        "--workers",
        "1",
        "--models",
        models,
        "--json",
        "--stats",
    ])
    .unwrap();
    let resources = zetesis_solve::Resources::default();
    let mut formula = resources.formula_limits();
    if let Some(work) = work {
        formula.max_work = work;
    }
    let prepared =
        zetesis_themelios::ParsedSource::new(PROGRAM.into(), resources.admission_options())
            .unwrap()
            .prepare_formula(resources.expansion_limits(), formula)
            .unwrap();
    let owner =
        zetesis_solve::ground_formula(prepared, zetesis_solve::Grounder::Auto, None).unwrap();
    let mut output = Vec::new();
    let result = publish_prepared(
        owner.input(),
        &PublicationConfig::from(&options),
        &mut JsonRenderer::new(
            &mut output,
            resources.json_record_bytes(),
            resources.formula_limits().theory.max_atoms,
        ),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .and_then(PublicationOutcome::into_legacy)
    .map(PublicationReport::into_report)
    .map_err(zetesis_cli::PublicationFailure::into_legacy);
    (result, output)
}

#[test]
fn work_refusal_retains_only_the_original_checked_prefix() {
    let (complete, output) = capture(None, "0");
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
    assert_eq!(
        family,
        [
            vec!["a(1)", "c(1)", "n(1)", "n(2)", "w(1,1)", "w(1,2)"],
            vec!["b(1)", "c(1)", "n(1)", "n(2)"]
        ]
    );
    let complete_document: Value = serde_json::from_slice(&output).unwrap();

    // Each answer gets the headroom left after admission: a ceiling one short
    // of admission plus the second answer's cost admits the first answer only.
    let second = receipt.reconstruction.latest.work;
    let admission = receipt.reconstruction.admission.work;
    let (first, _) = capture(None, "1");
    let first = first.unwrap().terminal_execution.unwrap().reconstruction;
    assert_eq!(first.admission.work, admission);
    assert!(
        first.latest.work < second,
        "the fixture's second answer costs more"
    );
    let ceiling = admission + second - 1;
    let (refused, output) = capture(Some(ceiling), "0");
    let refused = refused.unwrap_err();
    let RunError::Reconstruction(ReconstructionError::Source(cause)) = refused.cause.as_ref()
    else {
        panic!("expected a typed reconstruction source refusal: {refused:?}");
    };
    assert!(matches!(cause.as_ref(), FormulaFailure::Limit {
        resource: FormulaResource::Work, observed, limit, ..
    } if *limit == u128::from(ceiling - admission) && *observed > *limit));
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
    // The refused call used its whole headroom; the first answer's work counts
    // in the session total, not against the refused one.
    assert_eq!(receipt.reconstruction.allowance.work, ceiling - admission);
    assert_eq!(receipt.reconstruction.latest.work, ceiling - admission);
    assert_eq!(receipt.reconstruction.peak.work, ceiling - admission);
    assert_eq!(receipt.reconstruction.work, ceiling + first.latest.work);

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
