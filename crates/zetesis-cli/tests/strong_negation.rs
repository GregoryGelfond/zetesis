//! Signed atoms survive all native routes; coherence precedes scoring/display.

use clap::Parser;
use zetesis_cli::{
    Completion, Interruption, OptimizationStop, Options, Report, RunError, run_with_diagnostics,
};
use zetesis_cpu::Control;
use zetesis_themelios::observation::{ErrorKind, Resource};

fn solve(source: &str, arguments: &[&str]) -> (Result<Report, RunError>, String, String) {
    let options = Options::try_parse_from(
        ["zetesis", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = run_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    );
    (
        result,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn displays(output: &str) -> Vec<Vec<&str>> {
    let mut records = Vec::new();
    let mut lines = output.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            let mut atoms: Vec<_> = lines
                .next()
                .expect("complete display")
                .split_whitespace()
                .collect();
            atoms.sort_unstable();
            records.push(atoms);
        }
    }
    records.sort();
    records
}

#[test]
fn coherence_is_enforced_by_closure_and_frozen_reduct_routes() {
    for arguments in [
        vec![
            "--backend",
            "cpu",
            "--oracle",
            "closure",
            "--grounder",
            "lazy",
        ],
        vec![
            "--backend",
            "cpu",
            "--oracle",
            "closure",
            "--grounder",
            "eager",
        ],
        vec!["--backend", "cpu", "--oracle", "countermodel"],
    ] {
        let (report, output, _) = solve("p:-not -p. -p:-not p.", &arguments);
        assert_eq!(report.unwrap().completion, Completion::Exhausted);
        assert_eq!(displays(&output), vec![vec!["-p"], vec!["p"]]);
        let (report, output, _) = solve("p(1). -p(1).", &arguments);
        let report = report.unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.models, 0);
        assert!(output.starts_with("UNSATISFIABLE\n"), "{output}");
        assert!(!output.contains("Answer:"));
    }
}

#[test]
fn sign_specific_signatures_preserve_hidden_full_model_multiplicity() {
    for (source, expected) in [
        ("{p;-p}. #show p/0.", vec![vec![], vec![], vec!["p"]]),
        ("{p;-p}. #show -p/0.", vec![vec![], vec![], vec!["-p"]]),
        ("{p;-p}. #show.", vec![vec![], vec![], vec![]]),
        (
            "{-p}. #show -p/0. #show -p: -p.",
            vec![vec![], vec!["-p", "-p"]],
        ),
        (
            "{hidden}. -p(1). #show. #show -seen(X): -p(X).",
            vec![vec!["-seen(1)"], vec!["-seen(1)"]],
        ),
    ] {
        let (report, output, _) = solve(source, &[]);
        let report = report.unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.models, expected.len());
        assert_eq!(displays(&output), expected, "{source}");
    }
}

#[test]
fn objective_bounds_match_signed_relations_and_keep_every_optimal_tie() {
    let original = "1{p(1,5);-p(2,2)}1. {hidden}. \
        #minimize{W@1,X:p(X,W);W@1,X: -p(X,W)}.";
    for hidden in [false, true] {
        let source = format!(
            "{original} {}",
            if hidden { "#show -p/2. #show p/2." } else { "" }
        );
        let mut expected = None;
        for bound in ["0", "10000000"] {
            let (report, output, diagnostics) = solve(
                &source,
                &[
                    "--backend",
                    "cpu",
                    "--oracle",
                    "countermodel",
                    "--max-objective-bound-work",
                    bound,
                ],
            );
            let report = report.unwrap();
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, 2);
            let optimum = report.optimization.unwrap();
            assert_eq!(optimum.score.costs(), &[(1, 2)]);
            assert_eq!(optimum.tied_models, 2);
            assert_eq!(output.matches("Optimization: 2\n").count(), 2);
            assert!(!diagnostics.contains("Objective pruning stopped"));
            let actual: Vec<Vec<String>> = displays(&output)
                .into_iter()
                .map(|row| row.into_iter().map(str::to_owned).collect())
                .collect();
            let reference = vec![
                vec!["-p(2,2)".to_owned()],
                if hidden {
                    vec!["-p(2,2)".to_owned()]
                } else {
                    vec!["-p(2,2)".to_owned(), "hidden".to_owned()]
                },
            ];
            assert_eq!(actual, reference);
            if let Some(previous) = &expected {
                assert_eq!(&actual, previous);
            }
            expected = Some(actual);
            let restrictions = report
                .countermodel_statistics
                .unwrap()
                .candidate_restrictions;
            assert_eq!(restrictions > 0, bound != "0");
        }
    }
}

#[test]
fn signed_rendering_keeps_numeric_minus_strings_and_scalar_boundaries() {
    let (report, output, _) = solve(
        "-p(-1,\"a b\",\"-p\"). p(1,k,\"q\").",
        &["--backend", "cpu"],
    );
    assert_eq!(report.unwrap().models, 1);
    assert!(output.contains("-p(-1,\"a b\",\"-p\")"), "{output}");
    assert!(output.contains("p(1,k,\"q\")"));
    for (source, atom) in [("p(-a).", "p(-a)"), ("-p(f(1)).", "-p(f(1))")] {
        let (report, output, _) = solve(source, &[]);
        assert_eq!(report.unwrap().models, 1);
        assert_eq!(displays(&output), vec![vec![atom]]);
    }
    {
        let source = "p(1). #show -X:p(X).";
        let (result, output, _) = solve(source, &[]);
        assert!(result.is_err(), "{source}");
        assert!(
            output.is_empty(),
            "refusal emitted no partial answer: {output}"
        );
    }
}

#[test]
fn signed_display_limits_refuse_before_any_partial_answer() {
    for (flag, resource) in [
        ("--max-observation-work", Resource::Work),
        ("--max-observation-bindings", Resource::Bindings),
        ("--max-observation-terms", Resource::Terms),
        ("--max-observation-bytes", Resource::OutputBytes),
    ] {
        for suffix in ["", "#minimize{0: -p(1)}."] {
            let source = format!("-p(1). #show. #show -seen(X): -p(X). {suffix}");
            let (result, output, _) = solve(&source, &[flag, "0"]);
            assert!(
                matches!(result, Err(RunError::Observation(error)) if
                matches!(error.kind(), ErrorKind::Limit {resource: actual, ..} if *actual == resource)),
                "{flag}: {source}"
            );
            assert!(output.is_empty());
        }
    }
}

#[test]
fn retained_model_bytes_include_a_sign_tag_for_both_signs_before_display() {
    // One model-length header (8), one atom header (16), one predicate-sign
    // tag (1), and the one-byte name p. The minus is not part of that name.
    const PAYLOAD: usize = 8 + 16 + 1 + 1;
    for atom in ["p", "-p"] {
        for hidden in [false, true] {
            let source = format!(
                "{atom}. #minimize{{0@1,k: {atom}}}. {}",
                if hidden { "#show." } else { "" }
            );
            for ceiling in [PAYLOAD, PAYLOAD - 1] {
                let (result, output, _) = solve(
                    &source,
                    &[
                        "--backend",
                        "cpu",
                        "--oracle",
                        "countermodel",
                        "--max-objective-bound-work",
                        "0",
                        "--max-optimal-bytes",
                        &ceiling.to_string(),
                    ],
                );
                let report = result.expect("retention stop is a typed partial report");
                assert_eq!(
                    report.checked, 1,
                    "the original reduct checked the sole model"
                );
                if ceiling == PAYLOAD {
                    assert_eq!(report.completion, Completion::Exhausted);
                    assert!(report.interruption.is_none());
                    assert_eq!(report.models, 1);
                    let optimum = report.optimization.expect("retained complete optimum");
                    assert_eq!(optimum.score.costs(), &[(1, 0)]);
                    assert_eq!(optimum.tied_models, 1);
                    assert_eq!(optimum.scored_models, 1);
                    let displayed = if hidden { "" } else { atom };
                    assert!(
                        output.starts_with(&format!("Answer: 1\n{displayed}\nOptimization: 0\n")),
                        "{output}"
                    );
                    assert!(output.contains("OPTIMUM FOUND\nCoverage: exhausted"));
                } else {
                    assert_eq!(report.completion, Completion::Interrupted);
                    assert!(matches!(
                        report.interruption,
                        Some(Interruption::Incumbent(OptimizationStop::Bytes))
                    ));
                    assert_eq!(report.models, 0);
                    assert!(
                        report.optimization.is_none(),
                        "the first model could not be retained"
                    );
                    assert!(
                        output.starts_with(
                            "INCOMPLETE: incumbent retention stopped: Bytes\nCoverage: partial\n"
                        ),
                        "{output}"
                    );
                    for forbidden in [
                        "Answer:",
                        "Optimization:",
                        "OPTIMUM FOUND",
                        "SATISFIABLE",
                        "Coverage: exhausted",
                    ] {
                        assert!(!output.contains(forbidden), "{output}");
                    }
                }
            }
        }
    }
}
