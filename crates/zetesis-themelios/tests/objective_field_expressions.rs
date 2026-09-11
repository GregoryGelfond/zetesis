//! Evaluated objective fields share one complete source binding.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use zetesis_themelios::FormulaLimits;

const CASES: &str = include_str!("fixtures/objective-field-expressions.jsonl");

#[test]
fn evaluated_fields_preserves_full_scored_answers() {
    let cases = source_cases::cases(CASES);
    assert_eq!(cases.len(), 18);
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
fn evaluated_fields_keeps_the_original_reduct_subject() {
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
#[ignore = "requires independent clingo for 18 original expression sources"]
fn evaluated_fields_matches_fresh_clingo() {
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
fn eligible_field_arithmetic_keeps_located_failures() {
    for source in [
        "d(0).#minimize{1/X:d(X)}.",
        "d(0).#minimize{1,1/X:d(X)}.",
        "d(2147483647).#minimize{X+1:d(X)}.",
        "{a}.n(N):-N=#count{1:a;2:a}.#minimize{1/(N-1):n(N)}.",
    ] {
        let error = source_records::admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(
                error,
                zetesis_themelios::FormulaFailure::Expansion(
                    zetesis_themelios::ExpansionFailure::Evaluation { .. }
                )
            ),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn source_exclusion_precedes_field_evaluation() {
    for source in [
        "#minimize{1/X:d(X)}.",
        "a.n(N):-N=#count{1:a}.#minimize{1/N:n(N)}.",
        "a.n(N):-N=#count{1:a;2:a}.#minimize{1/(N-1):n(N)}.",
    ] {
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        let records = source_records::exhaustive(&input);
        assert_eq!(records.len(), 1);
        assert_eq!(
            records.iter().next().unwrap().1,
            if source.starts_with("#minimize") {
                None
            } else {
                Some(vec![1])
            }
        );
        assert_eq!(
            input.objectives().priorities(),
            if source.starts_with("#minimize") {
                &[][..]
            } else {
                &[0][..]
            }
        );
    }
}

#[test]
fn resolved_fields_share_template_and_tuple_ceilings() {
    let source = "p(1;2).#minimize{X+1,X+1:p(X)}.";
    let mut limits = FormulaLimits::default();
    limits.objective.max_templates = 2;
    limits.objective.max_tuple_width = 1;
    let input = source_records::admit(source, &limits).unwrap();
    assert_eq!(input.objectives().templates().len(), 2);
    assert_eq!(
        source_records::exhaustive(&input).iter().next().unwrap().1,
        Some(vec![5])
    );
    limits.objective.max_templates = 1;
    let error = source_records::admit(source, &limits).unwrap_err();
    assert!(matches!(
        error,
        zetesis_themelios::FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::ObjectiveElements,
            observed: 2,
            limit: 1,
            ..
        }
    ));
    limits.objective.max_templates = 2;
    limits.objective.max_tuple_width = 0;
    assert!(matches!(
        source_records::admit(source, &limits).unwrap_err(),
        zetesis_themelios::FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::Limit {
                resource: zetesis_objective::AdmissionResource::TupleWidth,
                actual: 1,
                limit: 0,
                ..
            },
            ..
        }
    ));
}
