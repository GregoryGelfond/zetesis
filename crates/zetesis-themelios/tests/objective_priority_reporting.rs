//! Source coverage preserves costs while raw retained zero slots can differ.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use zetesis_themelios::FormulaLimits;

const CASES: &str = include_str!("fixtures/objective-priority-reporting.jsonl");

fn reference(row: &Json) -> source_records::Records {
    row["reference_records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            (
                source_records::atoms(&record[0]),
                source_records::costs(&record[1]),
            )
        })
        .collect()
}

#[test]
fn zero_slot_reporting_preserves_full_scored_answers() {
    let cases = source_cases::cases(CASES);
    assert_eq!(cases.len(), 7);
    for (case, line) in cases.into_iter().zip(CASES.lines()) {
        let row: Json = serde_json::from_str(line).unwrap();
        let input = source_records::admit(&case.source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&input),
            case.records,
            "{}",
            case.name
        );
        assert_eq!(
            serde_json::to_value(input.objectives().priorities()).unwrap(),
            row["priorities"]
        );
        let native: BTreeMap<_, _> = case.records.into_iter().collect();
        let reference: BTreeMap<_, _> = reference(&row).into_iter().collect();
        assert_eq!(
            native.keys().collect::<Vec<_>>(),
            reference.keys().collect::<Vec<_>>()
        );
        let native_priorities: Vec<i32> =
            serde_json::from_value(row["priorities"].clone()).unwrap();
        let reference_priorities: Vec<i32> =
            serde_json::from_value(row["reference_priorities"].clone()).unwrap();
        let priorities: BTreeSet<_> = native_priorities
            .iter()
            .chain(&reference_priorities)
            .copied()
            .collect();
        for (model, costs) in &native {
            assert_eq!(costs.as_ref().map_or(0, Vec::len), native_priorities.len());
            assert!(costs.is_some() || native_priorities.is_empty());
            let expected_costs = &reference[model];
            assert_eq!(
                expected_costs.as_ref().map_or(0, Vec::len),
                reference_priorities.len()
            );
            assert!(expected_costs.is_some() || reference_priorities.is_empty());
            let actual: BTreeMap<_, _> = native_priorities
                .iter()
                .copied()
                .zip(costs.iter().flatten().copied())
                .collect();
            let expected: BTreeMap<_, _> = reference_priorities
                .iter()
                .copied()
                .zip(expected_costs.iter().flatten().copied())
                .collect();
            for priority in &priorities {
                assert_eq!(
                    actual.get(priority).copied().unwrap_or(0),
                    expected.get(priority).copied().unwrap_or(0),
                    "{}: {model:?}",
                    case.name
                );
            }
        }
        for first in native.keys() {
            for second in native.keys() {
                assert_eq!(
                    native[first].cmp(&native[second]),
                    reference[first].cmp(&reference[second]),
                    "{}: complete ordering",
                    case.name
                );
            }
        }
        let optimum = |records: &BTreeMap<BTreeSet<String>, Option<Vec<i64>>>| {
            let best = records.values().min().unwrap();
            records
                .iter()
                .filter(|(_, costs)| *costs == best)
                .map(|(model, _)| model.clone())
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            optimum(&native),
            optimum(&reference),
            "{}: every optimum tie",
            case.name
        );
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2 for raw priority reporting"]
fn fresh_clingo_preserves_raw_reporting_differences() {
    for line in CASES.lines() {
        let row: Json = serde_json::from_str(line).unwrap();
        assert_eq!(
            source_oracle::records(row["source"].as_str().unwrap()),
            reference(&row)
        );
    }
}
