//! Complete source families, raw priority reports and aligned objective orders.
use super::{source_cases, source_oracle, source_records};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use zetesis_themelios::FormulaLimits;

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

pub fn check(fixture: &str) {
    let cases = source_cases::cases(fixture);
    for (case, line) in cases.into_iter().zip(fixture.lines()) {
        let row: Json = serde_json::from_str(line).unwrap();
        let input = source_records::admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
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

pub fn fresh(fixture: &str) {
    for line in fixture.lines() {
        let row: Json = serde_json::from_str(line).unwrap();
        assert_eq!(
            source_oracle::records(row["source"].as_str().unwrap()),
            reference(&row)
        );
    }
}
