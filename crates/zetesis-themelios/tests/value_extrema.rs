//! Complete-value extrema against full source models and an independent reduct.
#[path = "support/finite_bindings.rs"]
mod reference;

use reference::{Models, exhaustive, native};
use serde_json::Value as Json;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn cases() -> Vec<Json> {
    include_str!("fixtures/value-extrema.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn models(value: &Json) -> Models {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .map(|atom| atom.as_str().unwrap().to_owned())
                .collect()
        })
        .collect()
}

#[test]
fn retained_upstream_bytes_and_both_full_model_contracts_are_preserved() {
    let fixtures = cases();
    for line in include_str!("../../../validation/upstream/clingo-5.8.2/cases.jsonl").lines() {
        let row: Json = serde_json::from_str(line).unwrap();
        let name = match row["id"].as_str().unwrap() {
            "lparse/assign/04" => "upstream-assign-min",
            "lparse/assign/05" => "upstream-assign-max",
            _ => continue,
        };
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture["name"] == name)
            .unwrap();
        assert_eq!(fixture["source"], row["source"]);
        assert_eq!(models(&fixture["models"]), models(&row["models"]));
        assert_eq!(
            models(&fixture["models"]),
            models(&row["expected_helper_models"])
        );
    }
}

#[test]
fn complete_values_preserve_source_models_and_original_reduct_minimality() {
    let rows = cases();
    assert_eq!(rows.len(), 218);
    for row in rows {
        let source = row["source"].as_str().unwrap();
        let input = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {source}: {error}", row["name"]));
        assert_eq!(input.source().text(), source);
        assert_eq!(native(&input), models(&row["models"]), "{}", row["name"]);
        assert_eq!(
            exhaustive(&input),
            models(&row["models"]),
            "{}",
            row["name"]
        );
    }
}

#[test]
fn value_carrier_and_cache_limits_refuse_without_a_partial_formula() {
    let source = "{p;q}.r(M):-M=#min{f(1),a:p;f(1),b:q;\"z\",c:p}.";
    for limits in [
        FormulaLimits {
            max_assignment_values: 1,
            ..FormulaLimits::default()
        },
        FormulaLimits {
            max_aggregate_cache_rows: 0,
            ..FormulaLimits::default()
        },
        FormulaLimits {
            max_aggregate_cache_elements: 2,
            ..FormulaLimits::default()
        },
        FormulaLimits {
            max_aggregate_cache_roots: 2,
            ..FormulaLimits::default()
        },
        FormulaLimits {
            max_aggregate_cache_key_bytes: 0,
            ..FormulaLimits::default()
        },
        FormulaLimits {
            max_work: 0,
            ..FormulaLimits::default()
        },
        FormulaLimits {
            max_substitutions: 0,
            ..FormulaLimits::default()
        },
    ] {
        let error = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .unwrap_err();
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
    let input = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
    )
    .unwrap();
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
        assert!(
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                limits
            )
            .is_err()
        );
    }
}

#[test]
#[ignore = "requires independent clingo; complete value-extrema models"]
fn recorded_sources_match_complete_fresh_clingo_enumeration() {
    for row in cases() {
        let json = reference::external(row["source"].as_str().unwrap(), true);
        assert_eq!(json["Models"]["More"], "no");
        let mut actual = Models::new();
        let mut count = 0;
        for call in json["Call"].as_array().unwrap() {
            if let Some(witnesses) = call["Witnesses"].as_array() {
                for witness in witnesses {
                    count += 1;
                    let wrapped = serde_json::json!([witness["Value"]]);
                    assert!(actual.insert(models(&wrapped).into_iter().next().unwrap()));
                }
            }
        }
        assert_eq!(json["Models"]["Number"].as_u64().unwrap(), count);
        assert_eq!(actual, models(&row["models"]), "{}", row["name"]);
    }
}
