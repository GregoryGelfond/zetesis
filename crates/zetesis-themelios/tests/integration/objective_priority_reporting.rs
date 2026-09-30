//! Source coverage preserves costs while raw retained zero slots can differ.

use crate::support::priority_contracts;

const CASES: &str = include_str!("../fixtures/objective-priority-reporting.jsonl");

#[test]
fn zero_slot_reporting_preserves_full_scored_answers() {
    assert_eq!(CASES.lines().count(), 7);
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires clingo: fresh clingo preserves raw reporting differences"]
fn fresh_clingo_preserves_raw_reporting_differences() {
    priority_contracts::fresh(CASES);
}
