//! Source coverage preserves costs while raw retained zero slots can differ.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

#[path = "support/priority_contracts.rs"]
mod priority_contracts;

const CASES: &str = include_str!("fixtures/objective-priority-reporting.jsonl");

#[test]
fn zero_slot_reporting_preserves_full_scored_answers() {
    assert_eq!(CASES.lines().count(), 7);
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires independent clingo 5.8.2 for raw priority reporting"]
fn fresh_clingo_preserves_raw_reporting_differences() {
    priority_contracts::fresh(CASES);
}
