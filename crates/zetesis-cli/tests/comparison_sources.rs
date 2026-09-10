//! Automatic CLI admission replays the frontend's complete comparison references.

use std::collections::BTreeSet;

use clap::Parser;
use serde_json::Value;
use zetesis_cli::{Completion, Options, run_detailed_with_diagnostics};
use zetesis_cpu::Control;

#[test]
fn automatic_and_explicit_reduct_routes_preserve_complete_comparison_models() {
    let cases: Vec<Value> =
        include_str!("../../zetesis-themelios/tests/fixtures/comparison-generators.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let mut admitted = 0;
    let mut reference_models = 0;
    for case in cases.iter().filter(|case| case["native"] == "admit") {
        let expected: BTreeSet<BTreeSet<&str>> = case["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|model| {
                model
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| {
                        let atom = atom.as_str().unwrap();
                        // These captured full-model fixtures contain no displayed
                        // whitespace inside atoms. Refuse to misparse a future addition.
                        assert!(!atom.chars().any(char::is_whitespace));
                        atom
                    })
                    .collect()
            })
            .collect();
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
            let source = case["source"].as_str().unwrap();
            assert!(
                !source.contains("#show"),
                "this campaign compares full models"
            );
            let report = run_detailed_with_diagnostics(
                source.to_owned(),
                &options,
                &mut output,
                &mut diagnostics,
                &Control::default(),
            )
            .unwrap_or_else(|error| panic!("{} via {oracle}: {error}", case["name"]));
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, expected.len());
            let output = String::from_utf8(output).unwrap();
            let mut lines = output.lines();
            let mut actual = BTreeSet::new();
            while let Some(line) = lines.next() {
                if line.starts_with("Answer:") {
                    let model = lines
                        .next()
                        .expect("complete Answer record")
                        .split_ascii_whitespace()
                        .collect::<BTreeSet<_>>();
                    assert!(actual.insert(model), "no duplicate full-model publication");
                }
            }
            assert_eq!(actual, expected, "{} via {oracle}", case["name"]);
            assert!(report.phase_timings.is_some());
            assert!(
                String::from_utf8(diagnostics)
                    .unwrap()
                    .contains("Phase timings:")
            );
        }
        admitted += 1;
        reference_models += expected.len();
    }
    assert_eq!(admitted, 120);
    assert_eq!(reference_models, 139);
}
