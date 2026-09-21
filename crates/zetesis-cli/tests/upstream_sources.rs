//! Complete full-model regression references from exact clingo 5.8.2 assertions.

use clap::Parser;
use serde_json::Value;
use std::path::Path;
use zetesis_cli::{Completion, Options, run_detailed_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_validation::curated::{self, Limits};

#[test]
fn upstream_admissions_preserve_complete_models_on_both_reduct_routes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation/upstream/clingo-5.8.2/curated");
    let corpus = curated::open(&root, Limits::default()).unwrap();
    replay(
        corpus.cases().iter().map(|case| Reference {
            id: case.id(),
            source: case.source(),
            models: case
                .contract()
                .full_models()
                .iter()
                .map(|model| model.iter().map(String::as_str).collect())
                .collect(),
        }),
        24,
        73,
    );
}

#[test]
fn combined_binding_and_head_features_preserve_complete_models() {
    let cases: Vec<Value> = include_str!("../../../validation/upstream/cross-feature.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(cases.iter().all(|case| case["native"] == "admit"));
    replay(cases.iter().map(|case| reference(case, "id")), 6, 15);
}

#[test]
fn negative_disjuncts_preserve_complete_models_through_ordinary_solving() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../zetesis-themelios/tests/fixtures/negative-heads.json"
    ))
    .unwrap();
    replay(cases.iter().map(|case| reference(case, "name")), 27, 49);
}

#[test]
fn negative_singleton_heads_preserve_complete_models() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../zetesis-themelios/tests/fixtures/singleton-heads.json"
    ))
    .unwrap();
    replay(cases.iter().map(|case| reference(case, "name")), 12, 19);
}

struct Reference<'a> {
    id: &'a str,
    source: &'a str,
    models: Vec<Vec<&'a str>>,
}

fn reference<'a>(case: &'a Value, identity: &str) -> Reference<'a> {
    Reference {
        id: case[identity].as_str().unwrap(),
        source: case["source"].as_str().unwrap(),
        models: case["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|model| {
                model
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| atom.as_str().unwrap())
                    .collect()
            })
            .collect(),
    }
}

fn replay<'a>(
    cases: impl IntoIterator<Item = Reference<'a>>,
    expected_cases: usize,
    expected_records: usize,
) {
    let mut admitted = 0;
    let mut records = 0;
    for case in cases {
        let mut expected = case.models;
        for model in &mut expected {
            assert!(
                model
                    .iter()
                    .all(|atom| !atom.chars().any(char::is_whitespace)),
                "fixture tokenization contract"
            );
            model.sort_unstable();
        }
        expected.sort_unstable();
        assert!(
            !case.source.contains("#show"),
            "full atom identities required"
        );
        for oracle in ["auto", "countermodel"] {
            let options = Options::try_parse_from([
                "zetesis",
                "--backend",
                "cpu",
                "--workers",
                "1",
                "--oracle",
                oracle,
                "--models",
                "0",
                "--stats",
            ])
            .unwrap();
            let mut output = Vec::new();
            let mut diagnostics = Vec::new();
            let report = run_detailed_with_diagnostics(
                case.source.to_owned(),
                &options,
                &mut output,
                &mut diagnostics,
                &Cancellation::default(),
            )
            .unwrap_or_else(|error| panic!("{} via {oracle}: {error}", case.id));
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, expected.len());
            let output = String::from_utf8(output).unwrap();
            let mut lines = output.lines();
            let mut actual = Vec::new();
            while let Some(line) = lines.next() {
                if line.starts_with("Answer:") {
                    let mut atoms: Vec<_> = lines
                        .next()
                        .expect("complete Answer record")
                        .split_ascii_whitespace()
                        .collect();
                    atoms.sort_unstable();
                    actual.push(atoms);
                }
            }
            actual.sort_unstable();
            assert_eq!(actual, expected, "{} via {oracle}", case.id);
            assert!(report.phase_timings.is_some());
        }
        admitted += 1;
        records += expected.len();
    }
    assert_eq!(admitted, expected_cases);
    assert_eq!(records, expected_records);
}
