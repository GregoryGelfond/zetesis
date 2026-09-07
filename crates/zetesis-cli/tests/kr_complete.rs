//! Recorded external whole-source parity runs portably without a solver dependency.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

#[test]
fn unchanged_entry_graphs_match_complete_external_results() {
    let recorded: Value =
        serde_json::from_str(include_str!("fixtures/kr-domains/complete-models.json")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/corpus/kr-domains");
    let cases = recorded["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 94);
    for case in cases {
        let path = case["path"].as_str().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(["--models", "0"])
            .arg(root.join(path))
            .output()
            .unwrap();
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            output.status.success(),
            "{path}: {}\n{text}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(text.contains("Coverage: exhausted"), "{path}: {text}");
        let expected = &case["answer"];
        let expected_models: BTreeSet<BTreeSet<&str>> = expected["models"]
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
            .collect();
        let expected_cost: Option<Vec<i64>> = expected["cost"]
            .as_array()
            .map(|cost| cost.iter().map(|x| x.as_i64().unwrap()).collect());
        let mut models = BTreeSet::new();
        let mut count = 0u64;
        let mut cost_count = 0u64;
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            if line.starts_with("Answer:") {
                // This pinned subset displays only symbolic/numeric atoms without spaces.
                let atoms = lines.next().unwrap();
                assert!(!atoms.contains('"'));
                models.insert(atoms.split_whitespace().collect::<BTreeSet<_>>());
                count += 1;
            } else if let Some(cost) = line.strip_prefix("Optimization:") {
                assert_eq!(
                    Some(
                        cost.split_whitespace()
                            .map(|x| x.parse::<i64>().unwrap())
                            .collect()
                    ),
                    expected_cost,
                    "{path}"
                );
                cost_count += 1;
            }
        }
        assert_eq!(models, expected_models, "{path}");
        assert_eq!(count, expected["model_count"].as_u64().unwrap(), "{path}");
        assert_eq!(
            cost_count,
            if expected_cost.is_some() { count } else { 0 },
            "{path}"
        );
        assert!(text.contains(&format!("Models: {count};")), "{path}");
        let status = if !expected["satisfiable"].as_bool().unwrap() {
            "UNSATISFIABLE"
        } else if expected_cost.is_some() {
            "OPTIMUM FOUND"
        } else {
            "SATISFIABLE"
        };
        assert_eq!(
            text.lines()
                .filter(|line| matches!(*line, "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND"))
                .collect::<Vec<_>>(),
            [status],
            "{path}"
        );
    }
}
