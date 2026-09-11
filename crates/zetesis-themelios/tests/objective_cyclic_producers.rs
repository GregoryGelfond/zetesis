//! Finite ordinary cyclic producers retain sound possible source rows.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use zetesis_themelios::FormulaLimits;

const CASES: &str = include_str!("fixtures/objective-cyclic-producers.jsonl");

#[test]
fn cyclic_producers_preserves_full_scored_answers() {
    let cases = source_cases::cases(CASES);
    assert_eq!(cases.len(), 15);
    for (case, row) in cases.into_iter().zip(CASES.lines()) {
        let expected: serde_json::Value = serde_json::from_str(row).unwrap();
        let input = source_records::admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(
            source_records::exhaustive(&input),
            case.records,
            "{}",
            case.name
        );
        assert_eq!(
            serde_json::to_value(input.objectives().priorities()).unwrap(),
            expected["priorities"],
            "{}",
            case.name
        );
    }
}

#[test]
fn cyclic_producers_keeps_the_original_reduct_subject() {
    for case in source_cases::cases(CASES) {
        let program = case
            .source
            .split("#minimize")
            .next()
            .unwrap()
            .split("#maximize")
            .next()
            .unwrap()
            .split(":~")
            .next()
            .unwrap();
        let original = source_records::admit(program, &FormulaLimits::default()).unwrap();
        let observed = source_records::admit(&case.source, &FormulaLimits::default()).unwrap();
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

#[test]
#[ignore = "requires independent clingo for 15 original cyclic sources"]
fn cyclic_producers_matches_fresh_clingo() {
    for case in source_cases::cases(CASES) {
        assert_eq!(
            source_oracle::records(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}

#[test]
fn cyclic_aggregate_generators_require_a_certificate() {
    let source = "n(N):-a,N=#count{1:a}.a:-n(0).#minimize{1:not a}.";
    let error = source_records::admit(source, &FormulaLimits::default()).unwrap_err();
    assert!(
        matches!(
            error,
            zetesis_themelios::FormulaFailure::Expansion(
                zetesis_themelios::ExpansionFailure::Admission(
                    zetesis_themelios::AdmissionFailure::Profile {
                        feature: zetesis_themelios::ProfileFeature::ObjectiveSourceEligibility,
                        ..
                    }
                )
            )
        ),
        "{error}"
    );
}
