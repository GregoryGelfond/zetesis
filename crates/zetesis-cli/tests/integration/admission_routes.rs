//! Automatic source admission retains anonymous projection semantics.

use clap::Parser;
use zetesis_cli::{
    Completion, Options, RunError, run_bundle_with_diagnostics, run_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_reference_support::BundleFixture;
use zetesis_themelios::{AdmissionFailure, ExpansionFailure};
use zetesis_validation::answers::native_json;

const SPELLING_BYTES: usize = 8 * 1024;

fn options() -> Options {
    Options::try_parse_from(["zetesis", "--backend", "cpu", "--models", "0", "--json"]).unwrap()
}

fn cases() -> [(&'static str, Vec<Vec<&'static str>>); 8] {
    [
        ("p(X):-X=1.", vec![vec!["p(1)"]]),
        ("p(X):-X=#inf.", vec![vec!["p(#inf)"]]),
        ("p(X):-X=Y,Y=2.", vec![vec!["p(2)"]]),
        ("p(1). q :- not p(_).", vec![vec!["p(1)"]]),
        ("p(1). q :- not not p(_).", vec![vec!["p(1)", "q"]]),
        (
            "{p(1)}. q :- not p(_). #show q/0.",
            vec![vec!["p(1)"], vec!["q"]],
        ),
        (
            "{p(1)}. q :- not not p(_).",
            vec![vec![], vec!["p(1)", "q"]],
        ),
        (
            "d(1..2). p(1,a). q(X) :- d(X), not p(X,_).",
            vec![vec!["d(1)", "d(2)", "p(1,a)", "q(2)"]],
        ),
    ]
}

#[test]
fn automatic_formula_routing_preserves_complete_answers() {
    for (source, expected) in cases() {
        for bundled in [false, true] {
            let mut output = Vec::new();
            let mut diagnostics = Vec::new();
            let options = options();
            let report = if bundled {
                let fixture = BundleFixture::new();
                fixture.write("entry.lp", source);
                let bundle = fixture.bundle();
                run_bundle_with_diagnostics(
                    bundle,
                    &options,
                    &mut output,
                    &mut diagnostics,
                    &Cancellation::default(),
                )
            } else {
                run_with_diagnostics(
                    source.into(),
                    &options,
                    &mut output,
                    &mut diagnostics,
                    &Cancellation::default(),
                )
            }
            .unwrap_or_else(|error| panic!("{source} (bundle={bundled}): {error}"));
            assert_eq!(report.completion, Completion::Exhausted);
            assert!(report.countermodel_statistics.is_some(), "{source}");
            let answers = native_json::parse(&output, native_json::Limits::default()).unwrap();
            assert_eq!(
                answers.full_model_symbols(SPELLING_BYTES).unwrap(),
                expected,
                "{source} (bundle={bundled})",
            );
        }
    }
}

#[test]
fn automatic_admission_retains_named_variable_safety() {
    let error = run_with_diagnostics(
        "q(X) :- not p(X).".into(),
        &options(),
        &mut Vec::new(),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        RunError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Core { .. }))
    ));
}

/// Shared batching admits the relational route but forbids formula execution.
/// Evaluate its policy early without letting it replace an earlier source
/// refusal or cancellation, on either original-input door.
#[test]
fn automatic_policy_keeps_refusal_and_control_order_on_both_doors() {
    for grounder in ["lazy", "auto"] {
        let configured = Options::try_parse_from([
            "zetesis",
            "--backend",
            "cpu",
            "--models",
            "0",
            "--json",
            "--grounder",
            grounder,
            "--source-batching",
            "worlds",
        ])
        .unwrap();
        for bundled in [false, true] {
            let run = |text: &str, cancellation: &Cancellation| {
                let mut output = Vec::new();
                let mut diagnostics = Vec::new();
                if bundled {
                    let fixture = BundleFixture::new();
                    fixture.write("entry.lp", text);
                    run_bundle_with_diagnostics(
                        fixture.bundle(),
                        &configured,
                        &mut output,
                        &mut diagnostics,
                        cancellation,
                    )
                } else {
                    run_with_diagnostics(
                        text.into(),
                        &configured,
                        &mut output,
                        &mut diagnostics,
                        cancellation,
                    )
                }
            };
            let ordinary = run("a.", &Cancellation::default()).unwrap();
            assert_eq!(ordinary.completion, Completion::Exhausted);
            assert!(
                ordinary.shared_execution.is_some(),
                "relational route {grounder}, bundle={bundled}"
            );
            // If formula admission ran, undefined arithmetic would refuse. The
            // saved execution-policy refusal must precede that preparation.
            let error = run("1{a;b}1. p(1/0).", &Cancellation::default()).unwrap_err();
            assert!(matches!(error, RunError::UnsupportedSourceBatching));
            let error = run("p(1/0).", &Cancellation::default()).unwrap_err();
            if bundled {
                assert!(matches!(error, RunError::BundleAdmission(_)));
            } else {
                assert!(matches!(error, RunError::Expansion(_)));
            }
            let cancellation = Cancellation::default();
            cancellation.cancel();
            let stopped = run("1{a;b}1. p(1/0).", &cancellation).unwrap();
            assert_eq!(stopped.completion, Completion::Interrupted);
            assert!(stopped.shared_execution.is_none());
            assert!(stopped.countermodel_statistics.is_none());
        }
    }
}
