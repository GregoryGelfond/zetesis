//! Rich finite producer cones preserve original model truth and objective order.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

#[path = "support/priority_contracts.rs"]
mod priority_contracts;

const CASES: &str = include_str!("fixtures/objective-rich-producers.jsonl");

#[test]
fn rich_producers_preserve_full_scored_answers() {
    assert_eq!(CASES.lines().count(), 50);
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires independent clingo 5.8.2 for raw priority reporting"]
fn rich_producers_match_fresh_raw_clingo() {
    priority_contracts::fresh(CASES);
}

#[test]
fn rich_producers_keep_the_original_reduct_subject() {
    for case in source_cases::cases(CASES) {
        let program = case.source.split("#minimize").next().unwrap();
        let original =
            source_records::admit(program, &zetesis_themelios::FormulaLimits::default()).unwrap();
        let observed =
            source_records::admit(&case.source, &zetesis_themelios::FormulaLimits::default())
                .unwrap();
        assert_eq!(original.atoms(), observed.atoms(), "{}", case.name);
        assert_eq!(
            original.theory().nodes(),
            observed.theory().nodes(),
            "{}",
            case.name
        );
        assert_eq!(
            original.theory().roots(),
            observed.theory().roots(),
            "{}",
            case.name
        );
        assert_eq!(
            original.formula_origins(),
            observed.formula_origins(),
            "{}",
            case.name
        );
    }
}
