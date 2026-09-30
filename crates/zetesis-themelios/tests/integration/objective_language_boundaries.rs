//! Exact witnesses for the remaining aggregate-head and objective boundaries.
//!
//! The fixture retains complete clingo 5.8.2 byte captures from the unchanged
//! sources. Native expectations separately specify the empty extremum-head
//! families and cyclic objective scores. Complete objective-free families remain
//! checked independently; reference observations do not define native semantics.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::time::Duration;

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_clingo_support as oracle;
use zetesis_reference_support::{admit, exhaustive};
use zetesis_test_support::records::Records;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const CASES: &str = include_str!("../fixtures/objective-language-boundaries.jsonl");
const SOURCE: SourceId = SourceId::new(173);

fn cases() -> Vec<Json> {
    let cases: Vec<Json> = CASES
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(cases.len(), 7);
    let mut names = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for case in &cases {
        assert!(names.insert(case["name"].as_str().unwrap()));
        assert!(sources.insert(case["source"].as_str().unwrap()));
    }
    cases
}

#[test]
fn missing_extremum_witnesses_have_no_answers() {
    let mut admitted = 0;
    for case in cases() {
        if !matches!(
            case["name"].as_str().unwrap(),
            "missing-min" | "missing-max"
        ) {
            continue;
        }
        let source = case["source"].as_str().unwrap();
        assert!(matches!(source, "#min{:a}=0." | "#max{:a}=0."));
        let input = admit_formula(
            source.into(),
            AdmissionOptions {
                source_id: SOURCE,
                ..AdmissionOptions::default()
            },
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        assert_eq!(input.source().id(), SOURCE);
        assert_eq!(input.source().text(), source);
        assert!(exhaustive(&input).is_empty());
        let mut search = zetesis_sat::StableModels::new(
            input.theory(),
            zetesis_sat::Limits::default(),
            zetesis_cpu::Cancellation::default(),
        )
        .unwrap();
        assert!(search.next().is_none());
        assert!(search.exhausted());
        admitted += 1;
    }
    assert_eq!(admitted, 2);
}

#[test]
fn boundary_sources_have_declared_scored_answers() {
    let mut migrated = 0;
    for case in cases() {
        let (priorities, records) = match case["name"].as_str().unwrap() {
            "missing-min" | "missing-max" => (vec![], serde_json::json!([])),
            "pooled-weak" => (
                vec![0],
                serde_json::json!([[["d(1)", "d(2)", "p(1)", "p(3)"], [1]]]),
            ),
            "cyclic-aggregate" => (vec![0], serde_json::json!([[["p"], [1]]])),
            "cyclic-conditional" => (vec![0], serde_json::json!([[["d"], [0]]])),
            // The nonnegative count guard establishes p even in its cycle.
            // The complete count assignment therefore excludes n(0), so its
            // redundant zero-cost priority is absent. The original family and
            // priority-1 cost agree with the retained independent capture.
            "cyclic-priority" => (vec![1], serde_json::json!([[["n(1)", "p"], [1]]])),
            "cyclic-multiple-observer" => (vec![1], serde_json::json!([[["n(1,2)", "p"], [2]]])),
            name => panic!("unclassified boundary source {name}"),
        };
        let input = admit(case["source"].as_str().unwrap(), &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let expected = records
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (oracle::atoms(&row[0]), oracle::costs(&row[1])))
            .collect();
        assert_eq!(exhaustive(&input), expected, "{}", case["name"]);
        assert_eq!(
            input.objectives().priorities(),
            priorities,
            "{}",
            case["name"]
        );
        migrated += 1;
    }
    assert_eq!(migrated, 7);
}

#[test]
fn cyclic_objectives_preserve_original_answers() {
    let mut originals = 0;
    for case in cases() {
        let Some(original) = case["original_source"].as_str() else {
            continue;
        };
        assert_eq!(
            case["source"]
                .as_str()
                .unwrap()
                .split_once("#minimize")
                .unwrap()
                .0,
            original
        );
        let input = admit(original, &FormulaLimits::default()).unwrap();
        assert!(input.objectives().templates().is_empty());
        let expected: Records = case["original_records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| (oracle::atoms(&record[0]), oracle::costs(&record[1])))
            .collect();
        assert_eq!(exhaustive(&input), expected, "{original}");
        originals += 1;
    }
    assert_eq!(originals, 4);
}

#[test]
#[ignore = "requires clingo: original boundary sources retain reference outcomes"]
fn original_boundary_sources_retain_reference_outcomes() {
    for case in cases() {
        let source = case["source"].as_str().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.lp");
        std::fs::write(&input, source).unwrap();
        let arguments = [
            input.clone().into_os_string(),
            OsString::from("0"),
            OsString::from("--outf=2"),
            OsString::from("--opt-mode=enum"),
        ];
        assert_eq!(
            case["reference_arguments"],
            serde_json::json!(["-", "0", "--outf=2", "--opt-mode=enum"])
        );
        let run = oracle::run_in(
            directory.path(),
            &arguments,
            &oracle::DECIDED,
            oracle::Limits {
                timeout: Duration::from_secs(5),
                max_output_bytes: 65_536,
            },
        );
        assert_eq!(
            i64::from(run.code()),
            case["reference_exit"].as_i64().unwrap(),
            "{source}"
        );
        let reference: Json =
            serde_json::from_str(case["reference_stdout"].as_str().unwrap()).unwrap();
        let actual = oracle::json(&run);
        assert_eq!(reference["Solver"], "clingo version 5.8.2");
        assert_eq!(actual["Solver"], reference["Solver"], "{source}");
        assert_eq!(reference["Input"], serde_json::json!(["-"]));
        assert_eq!(actual["Input"], serde_json::json!([input]), "{source}");
        for field in ["Result", "Models", "Calls"] {
            assert_eq!(actual[field], reference[field], "{source}: {field}");
        }
        assert_eq!(
            oracle::model_records(&actual),
            oracle::model_records(&reference),
            "{source}"
        );
        // The original capture used stdin. Normalize only the corresponding
        // input filename; retain every diagnostic byte apart from that name.
        let diagnostics = std::str::from_utf8(run.stderr())
            .unwrap()
            .replace(input.to_str().unwrap(), "-");
        assert_eq!(
            diagnostics,
            case["reference_stderr"].as_str().unwrap(),
            "{source}"
        );
    }
}
