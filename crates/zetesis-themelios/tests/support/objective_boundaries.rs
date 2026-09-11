//! Exact source contracts shared by the affected language-feature tests.
use super::source_records;

pub(super) const CASES: &str = include_str!("../fixtures/objective-boundaries.jsonl");

pub(super) fn check(source: &str) {
    let row = CASES
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| row["source"] == source)
        .unwrap_or_else(|| panic!("missing complete objective contract: {source}"));
    let input =
        source_records::admit(source, &zetesis_themelios::FormulaLimits::default()).unwrap();
    let expected: source_records::Records = row["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            (
                source_records::atoms(&record[0]),
                source_records::costs(&record[1]),
            )
        })
        .collect();
    assert_eq!(source_records::exhaustive(&input), expected, "{source}");
    assert_eq!(
        serde_json::to_value(input.objectives().priorities()).unwrap(),
        row["priorities"],
        "{source}"
    );
    assert_eq!(input.source().text(), source);
}
