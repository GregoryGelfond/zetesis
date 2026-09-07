//! Upstream semantic gaps must not be mislabeled as frontend parsing failures.

use serde_json::Value;
use themelios_base::source::{Source, SourceId};
use themelios_syntax::{dialect::Dialect, parse::parse};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[test]
fn every_selected_upstream_program_admits() {
    let mut count = 0;
    for line in include_str!("../../../validation/upstream/clingo-5.8.2/cases.jsonl").lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        assert_eq!(case["native"], "admit");
        admit_formula(
            case["source"].as_str().unwrap().to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
        count += 1;
    }
    assert_eq!(count, 24);
}

#[test]
fn every_selected_upstream_program_parses_and_raises_without_diagnostics() {
    let mut count = 0;
    for line in include_str!("../../../validation/upstream/clingo-5.8.2/cases.jsonl").lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        let source = Source::new(
            SourceId::new(0),
            case["source"].as_str().unwrap().to_owned(),
        )
        .unwrap();
        let parsed = parse(&source, Dialect::Clingo);
        assert!(
            parsed.diagnostics().is_empty(),
            "{}: {:?}",
            case["id"],
            parsed.diagnostics()
        );
        let raised = themelios_program::raise::raise(&parsed);
        assert!(
            raised.diagnostics().is_empty(),
            "{}: {:?}",
            case["id"],
            raised.diagnostics()
        );
        count += 1;
    }
    assert_eq!(count, 24);
}
