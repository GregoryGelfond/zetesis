//! Exact scored contracts for sources whose objective dependency refusal was removed.
use crate::support::objective_contract;

pub(crate) fn check(source: &str) {
    objective_contract::check(
        include_str!("../../fixtures/objective-dependency-contracts.jsonl"),
        source,
    );
}
