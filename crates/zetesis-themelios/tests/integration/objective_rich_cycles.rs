//! Completed possible support covers rich cyclic objective producer families.

use crate::support::priority_contracts;
use crate::support::source_cases;
use crate::support::source_oracle;
use crate::support::source_records;

use serde_json::Value as Json;
use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

const BOUNDARIES: &str = include_str!("../fixtures/objective-language-boundaries.jsonl");
const CASES: &str = include_str!("../fixtures/objective-rich-cycles.jsonl");

fn boundaries() -> Vec<Json> {
    let cases: Vec<Json> = BOUNDARIES
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|row: &Json| row.get("original_source").is_some())
        .collect();
    assert_eq!(cases.len(), 4);
    cases
}

#[test]
fn cyclic_objectives_keep_the_original_reduct_subject() {
    for case in boundaries() {
        let original = source_records::admit(
            case["original_source"].as_str().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap();
        let observed =
            source_records::admit(case["source"].as_str().unwrap(), &FormulaLimits::default())
                .unwrap();
        assert_eq!(original.atoms(), observed.atoms(), "{}", case["name"]);
        assert_eq!(
            original.theory().nodes(),
            observed.theory().nodes(),
            "{}",
            case["name"]
        );
        assert_eq!(
            original.theory().roots(),
            observed.theory().roots(),
            "{}",
            case["name"]
        );
        assert_eq!(
            original.formula_origins(),
            observed.formula_origins(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn rich_cycles_preserve_complete_objective_order() {
    assert_eq!(CASES.lines().count(), 15);
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires independent clingo 5.8.2 for complete cyclic families"]
fn rich_cycles_match_fresh_clingo() {
    priority_contracts::fresh(CASES);
    for case in boundaries() {
        let recorded: Json =
            serde_json::from_str(case["reference_stdout"].as_str().unwrap()).unwrap();
        assert_eq!(
            source_oracle::records(case["source"].as_str().unwrap()),
            source_oracle::model_records(&recorded),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn cyclic_completion_limits_are_inclusive() {
    let source = "{n(1)}.n(N):-N=#count{1:n(1)}.#minimize{N@N:n(N)}.";
    let expected = source_records::exhaustive(
        &source_records::admit(source, &FormulaLimits::default()).unwrap(),
    );
    for resource in [
        FormulaResource::SupportRounds,
        FormulaResource::AssignmentValues,
        FormulaResource::Substitutions,
        FormulaResource::ObjectivePresenceEntries,
        FormulaResource::Work,
    ] {
        let attempt = |maximum| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::SupportRounds => limits.max_support_rounds = maximum,
                FormulaResource::AssignmentValues => {
                    limits.max_assignment_values = usize::try_from(maximum).unwrap();
                }
                FormulaResource::Substitutions => limits.max_substitutions = maximum,
                FormulaResource::ObjectivePresenceEntries => {
                    limits.max_objective_presence_entries = usize::try_from(maximum).unwrap();
                }
                FormulaResource::Work => limits.max_work = maximum,
                _ => unreachable!("selected completion resources"),
            }
            source_records::admit(source, &limits)
        };
        let (mut lower, mut upper) = (0, 65_536);
        assert!(attempt(upper).is_ok());
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            match attempt(middle) {
                Ok(_) => upper = middle,
                Err(FormulaFailure::Limit {
                    resource: actual, ..
                }) if actual == resource => {
                    lower = middle;
                }
                Err(FormulaFailure::Aggregate { error, .. })
                    if resource == FormulaResource::Work
                        && error.kind() == zetesis_ferraris::AggregateErrorKind::WorkLimit =>
                {
                    lower = middle;
                }
                Err(error) => panic!("{resource:?}: unexpected refusal {error}"),
            }
        }
        assert_eq!(
            source_records::exhaustive(&attempt(upper).unwrap()),
            expected
        );
        let error = attempt(upper - 1).unwrap_err();
        assert!(
            matches!(error,
            FormulaFailure::Limit { resource: actual, observed, limit, .. }
            if actual == resource && observed == u128::from(upper)
                && limit == u128::from(upper - 1)),
            "{resource:?}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn growing_value_feedback_never_yields_a_program() {
    let source = "n(0).n(N):-N=#count{X:n(X)}.#minimize{N:n(N)}.";
    for maximum in [1, 2, 4] {
        let limits = FormulaLimits {
            max_support_rounds: maximum,
            ..FormulaLimits::default()
        };
        let error = source_records::admit(source, &limits).unwrap_err();
        assert!(
            matches!(error,
            FormulaFailure::Limit { resource: FormulaResource::SupportRounds, limit, observed, .. }
            if limit == u128::from(maximum) && observed == u128::from(maximum) + 1),
            "{error}"
        );
    }
}

#[test]
fn recursive_mixed_arithmetic_retains_the_complete_empty_family() {
    for (source, priorities) in [
        (
            "n(N):-N=#count{1:p}.p:-n(0).r(V):-n(N),V=1/N.#minimize{symbol:r(V)}.",
            &[][..],
        ),
        (
            "r(V):-n(N),V=1/N.p:-n(0).n(N):-N=#count{1:p}.#minimize{1:r(V)}.",
            &[0][..],
        ),
    ] {
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(input.warnings().len(), 1, "{source}");
        assert_eq!(input.objectives().priorities(), priorities, "{source}");
        assert!(
            input
                .atoms()
                .iter()
                .any(|atom| source_records::canonical(atom) == "r(1)")
        );
        // n(0) holds exactly when p does not, while p requires n(0):
        // there is no stable model, including after the undefined r row is omitted.
        assert!(source_records::exhaustive(&input).is_empty(), "{source}");
        let guarded = source.replace("V=1/N", "N!=0,V=1/N");
        let guarded = source_records::admit(&guarded, &FormulaLimits::default()).unwrap();
        assert!(guarded.warnings().is_empty());
        assert_eq!(
            source_records::exhaustive(&input),
            source_records::exhaustive(&guarded)
        );
    }
}

#[test]
fn source_order_preserves_cyclic_objective_families() {
    let cases = source_cases::cases(CASES);
    for (name, source) in [
        (
            "count_feedback",
            "#minimize{N@N:n(N)}.n(N):-N=#count{1:n(1)}.{n(1)}.",
        ),
        (
            "universal_feedback",
            "#minimize{1:p;2,q(1):q(1)}.q(1):-p.p:-q(X):d(X).{q(1);q(2)}.d(1;2).",
        ),
        (
            "signed_sum_feedback",
            "#minimize{N@1,k(N):n(N)}.n(N):-N=#sum{-1,j:n(1);2,k:n(1);2,k:a}.{a}.",
        ),
    ] {
        let expected = &cases.iter().find(|case| case.name == name).unwrap().records;
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(&source_records::exhaustive(&input), expected, "{name}");
    }
}

#[test]
fn rich_cycles_preserve_independent_carrier_precision() {
    for source in [
        "n(N):-N=#max{1;word}.p(N):-n(N),N=1.r:-#count{1:r}>=0.#minimize{N:n(N);1@3:p(N);2@5:r}.",
        "#minimize{2@5:r;1@3:p(N);N:n(N)}.r:-#count{1:r}>=0.p(N):-n(N),N=1.n(N):-N=#max{word;1}.",
    ] {
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        // The shared source fold proves p(1) absent: the required symbolic
        // maximum prevents n(1). Its redundant priority 3 is not published;
        // the independent required cyclic producer still contributes at 5.
        assert_eq!(input.objectives().priorities(), [5]);
        let records = serde_json::json!([[["n(word)", "r"], [2]]]);
        let expected = records
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    source_records::atoms(&row[0]),
                    source_records::costs(&row[1]),
                )
            })
            .collect();
        assert_eq!(source_records::exhaustive(&input), expected);
    }
}
