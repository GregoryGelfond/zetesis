//! Invariant aggregate values discharge generated priority obligations.

use crate::support::source_cases;
use crate::support::source_oracle;
use crate::support::source_records;

use source_records::{admit, exhaustive};
use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

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
{"name":"symbol_minimum","source":"a.{b}.n(N):-N=#min{z:a;\"s\":b}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["a","n(z)"],null],[["a","n(z)","b"],null]]}
{"name":"string_maximum","source":"a.{b}.n(N):-N=#max{\"s\":a;z:b}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["a","n(\"s\")"],null],[["a","n(\"s\")","b"],null]]}
{"name":"required_symbol_minimum","source":"a.b.{c}.n(N):-N=#min{\"s\":a;z:b;\"a\":c}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["a","b","n(z)"],null],[["a","b","n(z)","c"],null]]}
{"name":"required_string_maximum","source":"a.b.{c}.n(N):-N=#max{\"a\":a;z:b;zz:c}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["a","b","n(\"a\")"],null],[["a","b","n(\"a\")","c"],null]]}
{"name":"arity_minimum","source":"a.{b}.n(N):-N=#min{g(1):a;f(1,2):b}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["a","n(g(1))"],null],[["a","n(g(1))","b"],null]]}
{"name":"arity_maximum","source":"a.{b}.n(N):-N=#max{f(1,2):a;g(1):b}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["a","n(f(1,2))"],null],[["a","n(f(1,2))","b"],null]]}
{"name":"required_arity_minimum","source":"a.b.{c}.n(N):-N=#min{f(1,2):a;g(1):b;a(1,2):c}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["a","b","n(g(1))"],null],[["a","b","n(g(1))","c"],null]]}
{"name":"required_arity_maximum","source":"a.b.{c}.n(N):-N=#max{f(1,2):a;g(1):b;z(1):c}.#minimize{1@N:n(N)}.","priorities":[],"records":[[["a","b","n(f(1,2))"],null],[["a","b","n(f(1,2))","c"],null]]}
"#;

// Symbols precede strings, and constructor arity precedes name in ASP order.
// These answer families change the aggregate value. Their completed source
// carriers still contain only nonnumeric priorities and contribute no cost.
const CHANGING_ORDER: &str = r#"{"name":"changing_symbol_minimum","source":"a.{b}.n(N):-N=#min{\"s\":a;z:b}.#minimize{1@N:n(N)}.","records":[[["a","n(\"s\")"],null],[["a","b","n(z)"],null]]}
{"name":"changing_string_maximum","source":"a.{b}.n(N):-N=#max{z:a;\"s\":b}.#minimize{1@N:n(N)}.","records":[[["a","n(z)"],null],[["a","b","n(\"s\")"],null]]}
{"name":"changing_arity_minimum","source":"a.{b}.n(N):-N=#min{f(1,2):a;g(1):b}.#minimize{1@N:n(N)}.","records":[[["a","n(f(1,2))"],null],[["a","b","n(g(1))"],null]]}
{"name":"changing_arity_maximum","source":"a.{b}.n(N):-N=#max{g(1):a;f(1,2):b}.#minimize{1@N:n(N)}.","records":[[["a","n(g(1))"],null],[["a","b","n(f(1,2))"],null]]}
"#;

#[test]
fn changing_logical_extrema_preserve_scored_answers() {
    for case in source_cases::cases(CHANGING_ORDER.trim()) {
        let input = admit(&case.source, &FormulaLimits::default()).unwrap();
        assert!(input.objectives().priorities().is_empty());
        assert_eq!(exhaustive(&input), case.records, "{}", case.name);
    }
}

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
fn certificate_storage_has_an_inclusive_limit() {
    // One request, two tuple sets, two cone predicates, one transported name
    // and one completed carrier value coexist during certification.
    const CERTIFICATE_ENTRIES: usize = 7;
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
fn original_sources_match_complete_clingo_records() {
    for case in source_cases::cases(CASES.trim())
        .into_iter()
        .chain(source_cases::cases(CHANGING_ORDER.trim()))
    {
        assert_eq!(
            source_oracle::records(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}
