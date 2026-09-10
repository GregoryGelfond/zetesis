//! Invariant aggregate values discharge generated priority obligations.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use source_records::{admit, exhaustive};
use zetesis_themelios::{
    AdmissionFailure, ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource,
    ProfileFeature,
};

const CASES: &str = r#"{"name":"mandatory_count","source":"a.n(N):-N=#count{1:a}.#minimize{1@N:n(N)}.","priorities":[1],"records":[[["a","n(1)"],[1]]]}
{"name":"required_duplicate_key","source":"a.{b}.n(N):-N=#count{1:a;1:b}.#minimize{1@N:n(N)}.","priorities":[1],"records":[[["a","n(1)"],[1]],[["a","b","n(1)"],[1]]]}
{"name":"zero_optional_sum","source":"a.{b}.n(N):-N=#sum{2:a;0:b}.#minimize{1@N:n(N)}.","priorities":[2],"records":[[["a","n(2)"],[1]],[["a","b","n(2)"],[1]]]}
{"name":"ignored_optional_sum_plus","source":"a.{b}.n(N):-N=#sum+{2:a;-7:b}.#minimize{1@N:n(N)}.","priorities":[2],"records":[[["a","n(2)"],[1]],[["a","b","n(2)"],[1]]]}
{"name":"dominating_symbol","source":"b.{a}.n(N):-N=#max{2:a;foo:b}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["b","n(foo)"],null],[["a","b","n(foo)"],null]]}
{"name":"dominating_minimum","source":"a.{b}.n(N):-N=#min{2:a;5:b}.#minimize{1@N:n(N)}.","priorities":[2],"records":[[["a","n(2)"],[1]],[["a","b","n(2)"],[1]]]}
{"name":"dominating_maximum","source":"a.{b}.n(N):-N=#max{5:a;2:b}.#minimize{1@N:n(N)}.","priorities":[5],"records":[[["a","n(5)"],[1]],[["a","b","n(5)"],[1]]]}
{"name":"empty_maximum","source":"n(N):-N=#max{}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["n(#inf)"],null]]}
{"name":"empty_minimum","source":"n(N):-N=#min{}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["n(#sup)"],null]]}
{"name":"empty_sum","source":"n(N):-N=#sum{}.#minimize{N@N:n(N)}.","priorities":[0],"records":[[["n(0)"],[0]]]}
{"name":"forwarded_priority","source":"a.n(N):-N=#count{1:a}.copied(X):-n(X).#minimize{1@X:copied(X)}.","priorities":[1],"records":[[["a","n(1)","copied(1)"],[1]]]}
{"name":"correlated_weight_priority","source":"a.n(N):-N=#sum{2:a}.#minimize{N@N:n(N)}.","priorities":[2],"records":[[["a","n(2)"],[2]]]}
{"name":"distinct_full_keys","source":"a.n(N):-N=#count{1,k:a;1,l:a}.#minimize{1@N:n(N)}.","priorities":[2],"records":[[["a","n(2)"],[1]]]}
{"name":"ignored_sum_value","source":"a.n(N):-N=#sum{foo:a;2:a}.#minimize{1@N:n(N)}.","priorities":[2],"records":[[["a","n(2)"],[1]]]}
{"name":"impossible_expression_input","source":"a.n(N):-N=#count{1:a}.#minimize{1@(1/N):n(N)}.","priorities":[1],"records":[[["a","n(1)"],[1]]]}
{"name":"absent_tuple","source":"n(N):-N=#count{1:missing}.#minimize{1@N:n(N)}.","priorities":[0],"records":[[["n(0)"],[1]]]}
{"name":"closed_tuple","source":"n(N):-N=#sum{2}.#minimize{1@N:n(N)}.","priorities":[2],"records":[[["n(2)"],[1]]]}
"#;

#[test]
fn invariant_priorities_preserve_scored_answers() {
    for (case, row) in source_cases::cases(CASES.trim())
        .into_iter()
        .zip(CASES.lines())
    {
        let raw: serde_json::Value = serde_json::from_str(row).unwrap();
        let input = admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(exhaustive(&input), case.records, "{}", case.name);
        assert_eq!(
            serde_json::to_value(input.objectives().priorities()).unwrap(),
            raw["priorities"],
            "{}",
            case.name,
        );
    }
}

#[test]
fn changing_carriers_require_further_evidence() {
    for source in [
        "{a}.n(N):-N=#count{1:a}.#minimize{1@N:n(N)}.",
        "a.{b}.n(N):-N=#sum{2:a;3:b}.#minimize{1@N:n(N)}.",
        "a.{b}.n(N):-N=#min{2:a;1:b}.#minimize{1@N:n(N)}.",
        "a.{b}.n(N):-N=#max{2:a;3:b}.#minimize{1@N:n(N)}.",
        "a.n(N):-N=#count{1:a}.n(7).#minimize{1@N:n(N)}.",
        "a.n(N):-N=#count{1:a}.copied(X):-n(X).{copied(7)}.#minimize{1@X:copied(X)}.",
    ] {
        let error = admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::ObjectiveAggregateDependency,
                    ..
                }))
            ),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn certificate_storage_has_an_inclusive_limit() {
    // One request, two tuple sets, two cone predicates and one transported name.
    const CERTIFICATE_ENTRIES: usize = 6;
    let source = "a.n(N):-N=#count{1:a}.#minimize{1@N:n(N)}.";
    let mut limits = FormulaLimits {
        max_objective_presence_entries: CERTIFICATE_ENTRIES - 1,
        ..FormulaLimits::default()
    };
    let error = admit(source, &limits).unwrap_err();
    assert!(matches!(error, FormulaFailure::Limit {
        resource: FormulaResource::ObjectivePresenceEntries,
        observed,
        ..
    } if observed == CERTIFICATE_ENTRIES as u128));
    limits.max_objective_presence_entries = CERTIFICATE_ENTRIES;
    assert!(admit(source, &limits).is_ok());
}

#[test]
fn certificates_preserve_original_equalities() {
    let source = "a.n(N):-N=#count{1:a}.";
    let ordinary = admit(source, &FormulaLimits::default()).unwrap();
    let optimized = admit(
        &format!("{source}#minimize{{1@N:n(N)}}."),
        &FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(optimized.atoms(), ordinary.atoms());
    assert_eq!(optimized.theory().nodes(), ordinary.theory().nodes());
    assert_eq!(optimized.theory().roots(), ordinary.theory().roots());
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn certified_sources_match_complete_clingo_records() {
    for case in source_cases::cases(CASES.trim()) {
        assert_eq!(
            source_oracle::records(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}
