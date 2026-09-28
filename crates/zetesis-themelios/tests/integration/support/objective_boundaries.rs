//! Exact scored contracts for objectives composed with finite language features.
use crate::support::objective_contract;

pub(crate) fn check(source: &str) {
    objective_contract::check(
        include_str!("../../fixtures/objective-boundaries.jsonl"),
        source,
    );
}
