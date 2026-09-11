//! Exact scored contracts for sources whose objective dependency refusal was removed.
#[path = "objective_contract.rs"]
mod objective_contract;

pub(super) fn check(source: &str) {
    objective_contract::check(
        include_str!("../fixtures/objective-dependency-contracts.jsonl"),
        source,
    );
}
