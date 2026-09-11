//! Source grounding eligibility remains separate from complete-model truth.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

const CASES: &str = include_str!("fixtures/objective-source-completion.jsonl");

#[test]
fn source_completion_preserves_full_scored_answers() {
    let cases = source_cases::cases(CASES);
    assert_eq!(cases.len(), 29);
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
fn source_completion_keeps_the_original_reduct_subject() {
    for case in source_cases::cases(CASES) {
        let program = case
            .source
            .split("#minimize")
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
fn source_completion_limits_are_inclusive() {
    let source = "p(1;2).{q(1);q(2)}.#minimize{X@X:p(X),not q(X)}.";
    let expected = source_records::exhaustive(
        &source_records::admit(source, &FormulaLimits::default()).unwrap(),
    );
    for resource in [
        FormulaResource::Work,
        FormulaResource::Substitutions,
        FormulaResource::ObjectivePresenceEntries,
    ] {
        let attempt = |maximum| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::Work => limits.max_work = maximum,
                FormulaResource::Substitutions => limits.max_substitutions = maximum,
                FormulaResource::ObjectivePresenceEntries => {
                    limits.max_objective_presence_entries = usize::try_from(maximum).unwrap();
                }
                _ => unreachable!("selected source resources"),
            }
            source_records::admit(source, &limits)
        };
        let (mut lower, mut upper) = (0, 65_536);
        assert!(attempt(upper).is_ok());
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            if attempt(middle).is_ok() {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        assert_eq!(
            source_records::exhaustive(&attempt(upper).unwrap()),
            expected
        );
        let failure = attempt(upper - 1).unwrap_err();
        assert!(
            matches!(failure, FormulaFailure::Limit { resource: actual, observed, limit, .. } if actual == resource && observed == u128::from(upper) && limit == u128::from(upper - 1))
        );
        assert!(!failure.diagnostics().is_empty());
        assert_eq!(
            source_records::exhaustive(&attempt(upper).unwrap()),
            expected
        );
    }
}

#[test]
#[ignore = "requires independent clingo for 29 original completion sources"]
fn source_completion_matches_fresh_clingo() {
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
fn extended_cycles_keep_a_located_refusal() {
    let source = "{a}.b:-a.c:-b.b:-c.#minimize{1:not b}.";
    let error = source_records::admit(source, &FormulaLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(zetesis_themelios::ExpansionFailure::Admission(
            zetesis_themelios::AdmissionFailure::Profile {
                feature: zetesis_themelios::ProfileFeature::ObjectiveSourceEligibility,
                ..
            }
        ))
    ));
    assert!(!error.diagnostics().is_empty());
}
