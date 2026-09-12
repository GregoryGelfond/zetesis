//! Automatic source admission retains anonymous projection semantics.

use clap::Parser;
use zetesis_cli::{
    Completion, Options, RunError, run_bundle_with_diagnostics, run_with_diagnostics,
};
use zetesis_cpu::Control;
use zetesis_themelios::{AdmissionFailure, BundleLimits, ExpansionFailure, SourceBundle};
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
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("projection.lp");
                std::fs::write(&path, source).unwrap();
                let bundle = SourceBundle::load(&path, BundleLimits::default()).unwrap();
                run_bundle_with_diagnostics(
                    bundle,
                    &options,
                    &mut output,
                    &mut diagnostics,
                    &Control::default(),
                )
            } else {
                run_with_diagnostics(
                    source.into(),
                    &options,
                    &mut output,
                    &mut diagnostics,
                    &Control::default(),
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
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        RunError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Core { .. }))
    ));
}
