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
    assert_eq!(cases.len(), 36);
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
fn source_completion_limits_are_inclusive() {
    for source in [
        "p(1;2).{q(1);q(2)}.#minimize{X@X:p(X),not q(X)}.",
        "{a;b}.n(N):-N=#count{1:a;2:b}.#minimize{1@N:n(N);1@3:not a}.",
        "a:-not b.b:-not a.#minimize{1:not a}.",
        "d(1;2).{q(1);q(2)}.p:-q(X):d(X).#minimize{1:not p}.",
        "d(1;2).{p(1);p(2)}.n(K,N):-d(K),N=#count{X:p(X),X=K}.#minimize{N@K:n(K,N),not not n(K,N)}.",
    ] {
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
}

#[test]
#[ignore = "requires independent clingo for 36 original completion sources"]
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
fn producer_activity_does_not_consume_query_capacity() {
    for source in [
        "a.b.c.p:-a,b,c.#minimize{1:not not p}.",
        "a.b.c.{p:a,b,c}.#minimize{1:not not p}.",
    ] {
        let mut limits = FormulaLimits::default();
        limits.objective.max_condition_nodes = 5;
        let input = source_records::admit(source, &limits).unwrap();
        assert_eq!(input.objectives().templates().len(), 1);
        assert_eq!(
            input.objectives().templates()[0].condition().nodes().len(),
            5
        );
        let expected = source_records::exhaustive(
            &source_records::admit(source, &FormulaLimits::default()).unwrap(),
        );
        assert_eq!(source_records::exhaustive(&input), expected);
        limits.objective.max_condition_nodes = 4;
        assert!(matches!(
            source_records::admit(source, &limits).unwrap_err(),
            FormulaFailure::Objective {
                error: zetesis_objective::AdmissionError::Limit {
                    resource: zetesis_objective::AdmissionResource::ConditionNodes,
                    actual: 5,
                    limit: 4,
                    ..
                },
                ..
            }
        ));
    }
}

#[test]
fn absent_rows_retain_no_query_capacity() {
    let mut limits = FormulaLimits::default();
    limits.objective.max_condition_nodes = 0;
    let input = source_records::admit("a.b.c.p:-a,b,c.#minimize{1:not p}.", &limits).unwrap();
    assert!(input.objectives().templates().is_empty());
    assert!(input.objectives().priorities().is_empty());
    let records = source_records::exhaustive(&input);
    assert_eq!(records.len(), 1);
    assert_eq!(records.iter().next().unwrap().1, None);
}

#[test]
fn ignored_values_do_not_retain_query_capacity() {
    for source in [
        "{a}.#minimize{symbol:not not a}.",
        "w(symbol).{a}.#minimize{W:w(W),not not a}.",
        "{a}.#minimize{#inf:not not a}.",
        "{a}.#minimize{1@symbol:not not a}.",
        "p(symbol).{a}.#minimize{1@P:p(P),not not a}.",
    ] {
        let mut limits = FormulaLimits::default();
        limits.objective.max_condition_nodes = 0;
        let input = source_records::admit(source, &limits).unwrap();
        assert!(input.objectives().templates().is_empty(), "{source}");
        let records = source_records::exhaustive(&input);
        assert_eq!(records.len(), 2, "{source}");
        assert!(records.iter().all(|(_, costs)| costs.is_none()));
    }
}

#[test]
fn numeric_zero_retains_its_model_query() {
    let mut limits = FormulaLimits::default();
    limits.objective.max_condition_nodes = 5;
    let source = "{a}.#minimize{symbol:not not a;0:not not a;1@symbol:not not a}.";
    let input = source_records::admit(source, &limits).unwrap();
    assert_eq!(input.objectives().templates().len(), 1);
    assert_eq!(
        input.objectives().templates()[0].condition().nodes().len(),
        5
    );
    assert_eq!(input.objectives().priorities(), &[0]);
    let records = source_records::exhaustive(&input);
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|(_, costs)| *costs == Some(vec![0])));
    limits.objective.max_condition_nodes = 0;
    assert!(matches!(
        source_records::admit(source, &limits).unwrap_err(),
        FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::Limit {
                resource: zetesis_objective::AdmissionResource::ConditionNodes,
                ..
            },
            ..
        }
    ));
}

#[test]
fn source_arithmetic_precedes_query_materialization() {
    for source in [
        "d(0).{a}.#minimize{1/X:d(X),not not a}.",
        "d(0).{a}.#minimize{1@1/X:d(X),not not a}.",
        "d(0).{a}.#minimize{1,1/X:d(X),not not a}.",
    ] {
        let mut limits = FormulaLimits::default();
        limits.objective.max_condition_nodes = 0;
        assert!(
            matches!(
                source_records::admit(source, &limits).unwrap_err(),
                FormulaFailure::Expansion(zetesis_themelios::ExpansionFailure::Evaluation { .. })
            ),
            "{source}"
        );
    }
}
