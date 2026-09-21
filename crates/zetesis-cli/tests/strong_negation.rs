//! Signed atoms survive all native routes; coherence precedes scoring/display.

#[path = "support/clingo_report.rs"]
mod clingo_report;

use clap::Parser;
use zetesis_cli::{
    Completion, Interruption, OptimizationStop, Options, Report, RunError, run_with_diagnostics,
};
use zetesis_core::{Atom, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_solve::AnswerSelection;
use zetesis_themelios::observation::{ErrorKind, Resource};
use zetesis_themelios::{AdmissionFailure, ExpansionFailure, ProfileFeature};
use zetesis_validation::answers;

const NUMERIC_SHOW_NEGATION: &str = "p(1). #show -X:p(X).";
const MIXED_CHOICES: &str = "d(1..3). {c(X)} :- d(X). p(X) | q(X) :- c(X).";

fn mixed_choice_atoms(selection: usize) -> Vec<Atom> {
    let mut atoms = Vec::new();
    let mut choices = selection;
    for number in 1..=3 {
        let mut names = vec!["d"];
        match choices % 3 {
            1 => names.extend(["c", "p"]),
            2 => names.extend(["c", "q"]),
            _ => {}
        }
        choices /= 3;
        atoms.extend(names.into_iter().map(|name| {
            Atom::new(
                Predicate::new(name, 1).unwrap(),
                vec![Value::Number(number)],
            )
            .unwrap()
        }));
    }
    atoms.sort();
    atoms
}

#[test]
fn conditional_choices_preserve_the_complete_supported_family() {
    let (report, output, _) = solve(
        MIXED_CHOICES,
        &[
            "--backend",
            "cpu",
            "--oracle",
            "countermodel",
            "--search",
            "clauses",
            "--grounder",
            "eager",
            "--json",
        ],
    );
    let report = report.unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 27);
    let statistics = report.countermodel_statistics.unwrap();
    assert_eq!(
        statistics.support.unwrap().status,
        zetesis_sat::SupportStatus::Applied
    );
    assert_eq!(statistics.candidates, 27);
    assert_eq!(statistics.countermodel_queries, 27);
    assert_eq!(statistics.countermodels, 0);
    let answers =
        answers::native_json::parse(output.as_bytes(), answers::native_json::Limits::default())
            .unwrap();
    let mut actual = answers
        .records()
        .iter()
        .map(|answer| answer.full_model().to_vec())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = (0..27).map(mixed_choice_atoms).collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires independently installed clingo"]
fn mixed_choice_support_matches_complete_original_references() {
    for source in [
        MIXED_CHOICES,
        "d(1;\"x\";f(1)). {-c(X)} :- d(X). p(X) | q(X) :- -c(X).",
    ] {
        let (report, output, _) = solve(
            source,
            &[
                "--backend",
                "cpu",
                "--oracle",
                "countermodel",
                "--search",
                "clauses",
                "--grounder",
                "eager",
            ],
        );
        let report = report.unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.models, 27);
        let statistics = report.countermodel_statistics.unwrap();
        assert_eq!(
            statistics.support.unwrap().status,
            zetesis_sat::SupportStatus::Applied
        );
        assert_eq!(statistics.candidates, 27);
        let reference = clingo_report::complete(source, AnswerSelection::All);
        let actual = displays(&output)
            .into_iter()
            .map(|atoms| {
                (
                    atoms.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                    1_u64,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(reference.model_count(), 27);
        assert_eq!(reference.displays(), actual.as_slice());
    }
}

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
        &Cancellation::default(),
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
fn signed_values_preserve_their_rendering() {
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
}

#[test]
fn numeric_show_negation_preserves_the_full_answer() {
    let expected = Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(1)]).unwrap();
    for oracle in ["auto", "countermodel"] {
        let (report, output, _) = solve(
            NUMERIC_SHOW_NEGATION,
            &["--backend", "cpu", "--oracle", oracle, "--json"],
        );
        let report = report.unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.models, 1);
        assert!(report.optimization.is_none());
        let answers =
            answers::native_json::parse(output.as_bytes(), answers::native_json::Limits::default())
                .unwrap();
        assert_eq!(answers.records().len(), 1);
        let answer = &answers.records()[0];
        assert_eq!(answer.full_model(), std::slice::from_ref(&expected));
        assert_eq!(answer.shown_atom_indices(), &[0]);
        assert_eq!(answer.shown_terms(), &[Value::Number(-1)]);
        assert!(answer.costs().is_none());

        let (report, output, _) = solve(
            NUMERIC_SHOW_NEGATION,
            &["--backend", "cpu", "--oracle", oracle],
        );
        assert_eq!(report.unwrap().completion, Completion::Exhausted);
        assert_eq!(displays(&output), vec![vec!["-1", "p(1)"]]);
    }
}

#[test]
fn closure_preserves_its_term_output_boundary() {
    let (result, output, _) = solve(
        NUMERIC_SHOW_NEGATION,
        &["--backend", "cpu", "--oracle", "closure"],
    );
    assert!(matches!(
        result,
        Err(RunError::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Profile {
                feature: ProfileFeature::ShowTerm,
                ..
            }
        )))
    ));
    assert!(output.is_empty());
}

#[test]
#[ignore = "requires independently installed clingo"]
fn numeric_show_negation_matches_the_original_reference() {
    // Numeric negation is ordinary clingo-compatible arithmetic. This source
    // does not use the separate native extension for signed anonymous queries.
    let report = clingo_report::complete(NUMERIC_SHOW_NEGATION, AnswerSelection::All);
    assert!(report.satisfiable());
    assert_eq!(report.model_count(), 1);
    assert!(report.cost().is_none());
    assert_eq!(report.displays(), &[(vec!["-1".into(), "p(1)".into()], 1)]);
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
fn both_signs_have_the_same_retained_tag_size() {
    // Catalog length (8), atom header (16), sign (1), and name p (1).
    // The minus is not part of that name. One selected position adds a
    // selection length (8) and its original catalog index (8).
    const CATALOG_BYTES: usize = 8 + 16 + 1 + 1;
    const SELECTION_BYTES: usize = 8 + 8;
    // One shared optional score: present tag, length, i32 priority and i64 cost.
    const SCORE_BYTES: usize = 1 + 8 + 4 + 8;
    const PAYLOAD: usize = CATALOG_BYTES + SELECTION_BYTES + SCORE_BYTES;
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
