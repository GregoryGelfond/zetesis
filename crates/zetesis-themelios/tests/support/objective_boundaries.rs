//! Exact scored contracts for objectives composed with finite language features.
#[path = "objective_contract.rs"]
mod objective_contract;

pub(super) fn check(source: &str) {
    objective_contract::check(
        include_str!("../fixtures/objective-boundaries.jsonl"),
        source,
    );
}
