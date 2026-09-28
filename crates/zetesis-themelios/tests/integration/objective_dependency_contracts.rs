//! Preserved source refusals now have full scored-family and reporting contracts.

use crate::support::priority_contracts;

const CASES: &str = include_str!("../fixtures/objective-dependency-contracts.jsonl");

#[test]
fn dependency_sources_preserve_every_optimum_tie() {
    assert_eq!(CASES.lines().count(), 41);
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires independent clingo 5.8.2 for complete raw priority records"]
fn dependency_sources_match_fresh_reference_records() {
    priority_contracts::fresh(CASES);
}
