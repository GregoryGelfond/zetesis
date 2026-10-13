//! Complete-value extrema against full source models and an independent reduct.
use crate::support::clingo_reports;
use crate::support::finite_bindings as reference;
use crate::support::upstream;
use zetesis_clingo_support as oracle;
use zetesis_reference_support::{admit, resource_limits};

use reference::{exhaustive, native};
use serde_json::Value as Json;
use zetesis_themelios::{FormulaLimits, FormulaResource};

fn cases() -> Vec<Json> {
    include_str!("../fixtures/value-extrema.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn retained_upstream_sources_are_unchanged() {
    for (fixture, case) in upstream_cases() {
        assert_eq!(fixture["source"].as_str().unwrap(), case.source());
    }
}

#[test]
fn retained_upstream_models_preserve_the_full_contract() {
    for (fixture, case) in upstream_cases() {
        assert_eq!(
            model_records(&fixture["models"]),
            sorted_records(case.contract().full_models().to_vec())
        );
    }
}

#[test]
fn retained_upstream_models_preserve_the_helper_contract() {
    for (fixture, case) in upstream_cases() {
        assert_eq!(
            model_records(&fixture["models"]),
            sorted_records(case.contract().helper_models().to_vec())
        );
    }
}

fn upstream_cases() -> Vec<(Json, &'static zetesis_validation::curated::Case)> {
    let fixtures = cases();
    [
        ("lparse/assign/04", "upstream-assign-min"),
        ("lparse/assign/05", "upstream-assign-max"),
    ]
    .into_iter()
    .map(|(id, name)| {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture["name"] == name)
            .unwrap()
            .clone();
        let case = upstream::corpus()
            .cases()
            .iter()
            .find(|case| case.id() == id)
            .unwrap();
        (fixture, case)
    })
    .collect()
}

fn model_records(value: &Json) -> Vec<Vec<String>> {
    sorted_records(serde_json::from_value(value.clone()).unwrap())
}

fn sorted_records(mut records: Vec<Vec<String>>) -> Vec<Vec<String>> {
    for record in &mut records {
        record.sort_unstable();
    }
    records.sort_unstable();
    records
}

#[test]
fn complete_values_preserve_source_models_and_original_reduct_minimality() {
    let rows = cases();
    assert_eq!(rows.len(), 218);
    for row in rows {
        let source = row["source"].as_str().unwrap();
        let input = admit(source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {source}: {error}", row["name"]));
        assert_eq!(input.source().expect("source input").text(), source);
        assert_eq!(
            native(&input),
            clingo_reports::expected(&row),
            "{}",
            row["name"]
        );
        assert_eq!(
            exhaustive(&input),
            clingo_reports::expected(&row),
            "{}",
            row["name"]
        );
    }
}

#[test]
fn value_carrier_and_cache_limits_refuse_without_a_partial_formula() {
    let source = "{p;q}.r(M):-M=#min{f(1),a:p;f(1),b:q;\"z\",c:p}.";
    for limits in [
        resource_limits::formula(FormulaResource::AssignmentValues, 1),
        resource_limits::formula(FormulaResource::AggregateCacheRows, 0),
        resource_limits::formula(FormulaResource::AggregateCacheElements, 2),
        resource_limits::formula(FormulaResource::AggregateCacheRoots, 2),
        resource_limits::formula(FormulaResource::AggregateCacheKeys, 0),
        resource_limits::formula(FormulaResource::Work, 0),
        resource_limits::formula(FormulaResource::Substitutions, 0),
    ] {
        let error = admit(source, &limits).unwrap_err();
        assert!(!error.diagnostics().is_empty(), "{error}");
    }
}

#[test]
fn equal_first_values_share_proposals_but_retain_complete_tuple_limit_counts() {
    let source = "{p;q}.r(M):-M=#min{f(1),a:p;f(1),b:q}.";
    let limits = FormulaLimits {
        max_assignment_values: 2,
        max_aggregate_cache_elements: 2,
        ..FormulaLimits::default()
    };
    let input = admit(source, &limits).unwrap();
    assert_eq!(native(&input), exhaustive(&input));
    for limits in [
        FormulaLimits {
            max_assignment_values: 1,
            ..limits
        },
        FormulaLimits {
            max_aggregate_cache_elements: 1,
            ..limits
        },
    ] {
        assert!(admit(source, &limits).is_err());
    }
}

#[test]
#[ignore = "requires clingo: recorded sources match complete fresh clingo enumeration; complete value-extrema models"]
fn recorded_sources_match_complete_fresh_clingo_enumeration() {
    for row in cases() {
        let json = reference::external(row["source"].as_str().unwrap(), true);
        assert_eq!(
            oracle::full_models(&json),
            clingo_reports::expected(&row),
            "{}",
            row["name"]
        );
    }
}
