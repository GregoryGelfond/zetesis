//! Complete answer families for objectives composed with finite language features.
#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;
#[path = "support/priority_contracts.rs"]
mod priority_contracts;
#[path = "support/objective_boundaries.rs"]
mod objective_boundaries;

#[test]
fn objective_consumers_preserve_complete_scored_families() {
    assert_eq!(objective_boundaries::CASES.lines().count(), 43);
    for case in source_cases::cases(objective_boundaries::CASES) {
        objective_boundaries::check(&case.source);
    }
    // Compare costs at named priorities and all optimal ties, retaining exact
    // versioned differences in always-zero slot reporting.
    priority_contracts::check(objective_boundaries::CASES);
}

#[test]
#[ignore = "requires independent clingo 5.8.2 for complete original source records"]
fn original_sources_match_declared_clingo_records() {
    priority_contracts::fresh(objective_boundaries::CASES);
}
