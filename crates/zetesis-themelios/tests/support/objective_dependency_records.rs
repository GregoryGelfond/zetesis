//! Complete scored contracts for exact sources whose objective refusal was removed.

#[path = "source_records.rs"]
mod source_records;

use serde_json::Value as Json;
use zetesis_themelios::FormulaLimits;

pub fn check(source: &str) {
    let rows: Vec<Json> = include_str!("../fixtures/objective-dependency-contracts.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<Json>(line).unwrap())
        .filter(|row| row["source"] == source)
        .collect();
    let [row] = rows.as_slice() else {
        panic!("one preserved complete contract for {source}");
    };
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
    let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(input.source().text(), source);
    assert_eq!(
        source_records::exhaustive(&input),
        expected,
        "{}",
        row["name"]
    );
    assert_eq!(
        serde_json::to_value(input.objectives().priorities()).unwrap(),
        row["priorities"]
    );
    // An objective reads complete models of this exact original theory.
    let original_source = source.split("#minimize").next().unwrap();
    let original = source_records::admit(original_source, &FormulaLimits::default()).unwrap();
    assert_eq!(input.atoms(), original.atoms());
    assert_eq!(input.theory().nodes(), original.theory().nodes());
    assert_eq!(input.theory().roots(), original.theory().roots());
    assert_eq!(input.formula_origins(), original.formula_origins());
}
