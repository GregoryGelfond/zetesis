//! Complete full-model regression references from exact clingo 5.8.2 assertions.

use clap::Parser;
use serde_json::Value;
use zetesis_cli::{Completion, Options, run_detailed_with_diagnostics};
use zetesis_cpu::Control;

#[test]
fn upstream_admissions_preserve_complete_models_on_both_reduct_routes() {
    replay(
        include_str!("../../../validation/upstream/clingo-5.8.2/cases.jsonl"),
        18,
        43,
    );
}

#[test]
fn combined_binding_and_head_features_preserve_complete_models() {
    replay(
        include_str!("../../../validation/upstream/cross-feature.jsonl"),
        6,
        15,
    );
}

#[test]
fn negative_disjuncts_preserve_complete_models_through_ordinary_solving() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../zetesis-themelios/tests/fixtures/negative-heads.json"
    ))
    .unwrap();
    let mut fixture = String::new();
    for mut case in cases {
        case["native"] = "admit".into();
        case["id"] = case["name"].clone();
        fixture.push_str(&serde_json::to_string(&case).unwrap());
        fixture.push('\n');
    }
    replay(&fixture, 27, 49);
}

fn replay(fixtures: &str, expected_cases: usize, expected_records: usize) {
    let mut admitted = 0;
    let mut records = 0;
    for line in fixtures.lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        if case["native"] != "admit" {
            continue;
        }
        let mut expected: Vec<Vec<&str>> = case["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|model| {
                let mut atoms: Vec<_> = model
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| {
                        let atom = atom.as_str().unwrap();
                        assert!(
                            !atom.chars().any(char::is_whitespace),
                            "fixture tokenization contract"
                        );
                        atom
                    })
                    .collect();
                atoms.sort_unstable();
                atoms
            })
            .collect();
        expected.sort_unstable();
        let source = case["source"].as_str().unwrap();
        assert!(!source.contains("#show"), "full atom identities required");
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
                source.to_owned(),
                &options,
                &mut output,
                &mut diagnostics,
                &Control::default(),
            )
            .unwrap_or_else(|error| panic!("{} via {oracle}: {error}", case["id"]));
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
            assert_eq!(actual, expected, "{} via {oracle}", case["id"]);
            assert!(report.phase_timings.is_some());
        }
        admitted += 1;
        records += expected.len();
    }
    assert_eq!(admitted, expected_cases);
    assert_eq!(records, expected_records);
}
