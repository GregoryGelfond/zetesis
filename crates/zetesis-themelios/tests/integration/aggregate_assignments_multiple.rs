//! Independent aggregate proposals form a bounded product; each equality stays
//! in the original formula. The reference evaluator implements the finite
//! reduct definition without the production reduct masks or countermodel search.

use crate::support::finite_bindings::{Models, exhaustive, native};
use crate::support::objective_dependency_records as objective_dependencies;

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_clingo_support as oracle;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula,
};

const SOURCE: SourceId = SourceId::new(83);

fn options() -> AdmissionOptions {
    AdmissionOptions {
        source_id: SOURCE,
        ..Default::default()
    }
}

fn input(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}

fn cases() -> Vec<Json> {
    include_str!("../fixtures/aggregate-assignments-multiple.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn expected(row: &Json) -> Models {
    row[2].as_array().unwrap().iter().map(json_model).collect()
}

fn json_model(values: &Json) -> BTreeSet<String> {
    let atoms = values.as_array().unwrap();
    let result: BTreeSet<_> = atoms
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(result.len(), atoms.len(), "full atom identities are unique");
    result
}

#[test]
fn all_function_pairs_correlations_and_recursive_equalities_match_the_finite_reduct() {
    let cases = cases();
    assert_eq!(cases.len(), 54);
    for row in cases {
        let source = row[1].as_str().unwrap();
        let admitted = input(source).unwrap_or_else(|error| panic!("{}: {error}", row[0]));
        assert_eq!(admitted.source().text(), source);
        assert_eq!(exhaustive(&admitted), expected(&row), "{}", row[0]);
        assert_eq!(native(&admitted), expected(&row), "{}", row[0]);
    }
}

#[test]
fn independent_generator_order_preserves_scopes_and_full_models() {
    let aggregates = ["N=#count{X:p(X)}", "M=#sum{X:q(X)}", "K=#sum+{-2:p(1)}"];
    let mut reference = None;
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let body = order.map(|index| aggregates[index]).join(",");
        let source = format!("p(1).q(2).r(N,M,K):-{body}.");
        let admitted = input(&source).unwrap();
        let actual = native(&admitted);
        assert_eq!(actual, exhaustive(&admitted));
        assert_eq!(
            actual,
            BTreeSet::from([BTreeSet::from([
                "p(1)".into(),
                "q(2)".into(),
                "r(1,2,0)".into()
            ])])
        );
        if let Some(reference) = &reference {
            assert_eq!(&actual, reference);
        } else {
            reference = Some(actual);
        }
    }
}

#[test]
fn extra_aggregate_guards_test_completed_values() {
    for source in [
        "r(N,M):-N=#count{},M=#count{},M<=#count{}.",
        "r(N,M):-N=#count{},M=#count{},M=#sum{}.",
    ] {
        assert_eq!(
            native(&input(source).unwrap()),
            Models::from([BTreeSet::from(["r(0,0)".into()])]),
        );
    }
}

#[test]
fn own_target_and_unbound_variables_keep_their_safety_refusal() {
    for source in [
        "r(N,M):-N=#count{N:p},M=#sum{}.",
        "r(N,M):-N=#count{},M=#count{X:p(M,X)}.",
        "r(X,N,M):-N=#count{X:p(X)},M=#sum{}.",
        "r(N,M):-N=#count{},M=#count{X:not p(X)}.",
        "r(N,M):-not N=#count{},M=#count{}.",
    ] {
        let error = input(source).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn multiple_assignments_preserve_scored_answers() {
    objective_dependencies::check("{p}.r(N,M):-N=#count{1:p},M=#sum{2:p}.#minimize{N@3,M:r(N,M)}.");
    let unrelated = "{p}.r(N,M):-N=#count{1:p},M=#sum{2:p}.#minimize{1@3:p}.";
    assert_eq!(input(unrelated).unwrap().objectives().priorities(), &[3]);
}

#[test]
fn product_limits_refuse_the_whole_source_and_preserve_original_locations() {
    let rule = "r(N,M):-N=#count{1:p},M=#count{1:q}.";
    let source = format!("{{p;q}}.\n{rule}");
    let reference = input(&source).unwrap();
    assert_eq!(native(&reference).len(), 4);
    for (limits, expected) in [
        (
            FormulaLimits {
                max_substitutions: 8,
                ..Default::default()
            },
            FormulaResource::Substitutions,
        ),
        (
            FormulaLimits {
                max_assignment_values: 1,
                ..Default::default()
            },
            FormulaResource::AssignmentValues,
        ),
        (
            FormulaLimits {
                max_aggregate_cache_rows: 1,
                ..Default::default()
            },
            FormulaResource::AggregateCacheRows,
        ),
        (
            FormulaLimits {
                max_aggregate_cache_roots: 1,
                ..Default::default()
            },
            FormulaResource::AggregateCacheRoots,
        ),
    ] {
        let error = admit_formula(
            source.clone(),
            options(),
            ExpansionLimits::default(),
            limits,
        )
        .unwrap_err();
        let FormulaFailure::Limit {
            resource,
            limit,
            observed,
            location,
        } = error
        else {
            panic!("expected formula product refusal: {error}");
        };
        assert_eq!(resource, expected);
        assert!(observed > limit);
        assert_eq!(location.source, SOURCE);
        assert_eq!(reference.source().slice(location.span).unwrap(), rule);
        assert_eq!(native(&input(&source).unwrap()), native(&reference));
    }
    let origins: BTreeSet<_> = reference
        .formula_origins()
        .iter()
        .flatten()
        .map(|location| reference.source().slice(location.span).unwrap())
        .collect();
    assert!(origins.contains(rule));
    // Keep upstream ASP-Core-2 safety facts intact; the frontend independently
    // establishes the clingo equality-binder extension.
    assert!(!reference.source_analysis().safety().is_safe());
}

#[test]
fn scope_work_is_bounded_and_can_be_retried_without_partial_admission() {
    let source = "r(N,M):-N=#count{},M=#sum{}.";
    let error = admit_formula(
        source.into(),
        options(),
        ExpansionLimits {
            max_term_work: 1,
            ..Default::default()
        },
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(error,
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::TermWork, observed, limit, location
        }) if observed > limit && location.source == SOURCE
    ));
    assert_eq!(
        native(&input(source).unwrap()),
        BTreeSet::from([BTreeSet::from(["r(0,0)".into()])])
    );
}

fn external(source: &str) -> Json {
    oracle::json(&oracle::run(
        source,
        &["--models=0", "--outf=2"],
        oracle::Limits {
            timeout: Duration::from_secs(3),
            max_output_bytes: 2 * 65_536,
        },
    ))
}

#[test]
#[ignore = "requires external clingo; exact original sources and complete full-model replay"]
fn multiple_aggregate_assignments_match_clingo() {
    for row in cases() {
        let source = row[1].as_str().unwrap();
        let result = external(source);
        assert!(
            result["Solver"]
                .as_str()
                .unwrap()
                .starts_with("clingo version ")
        );
        assert_eq!(result["Models"]["More"], "no", "{}", row[0]);
        let witnesses: Vec<_> = result["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .collect();
        let actual: Models = witnesses
            .iter()
            .map(|witness| json_model(&witness["Value"]))
            .collect();
        assert_eq!(
            actual.len(),
            witnesses.len(),
            "complete semantic multiplicity"
        );
        assert_eq!(
            result["Models"]["Number"].as_u64().unwrap(),
            u64::try_from(actual.len()).unwrap()
        );
        assert_eq!(actual, expected(&row), "{}: {source}", row[0]);
        assert_eq!(actual, native(&input(source).unwrap()), "{}", row[0]);
        assert_eq!(
            result["Result"],
            if actual.is_empty() {
                "UNSATISFIABLE"
            } else {
                "SATISFIABLE"
            }
        );
    }
}
