//! Closed choice ranges preserve one group, local products and every reduct.

use crate::support::answer_records::Record;
use crate::support::clingo_reports::optimal;
use crate::support::finite_bindings::{holds, remap, values};
use crate::support::objective_dependency_records as objective_dependencies;
use crate::support::thresholds::first_success;

use std::collections::BTreeSet;
use std::fs::{self};

use serde_json::Value as Json;
use themelios_program::term::EvalError;
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_reference_support::admit;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits,
    FormulaResource, ProfileFeature, SourceBundle, admit_bundle_formula, admit_extended,
    admit_formula,
};

fn cases() -> Vec<Json> {
    include_str!("../fixtures/choice-intervals.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

// Independent topological evaluation: every subtree false in M is falsum in
// F^M. No production evaluator, reduct mask, SAT search or subset enumeration
// helper is used to decide these finite stable models.
fn record(admitted: &AdmittedFormula, mask: usize) -> Record {
    let model = Model::from_positions(
        admitted.atom_catalog(),
        (0..admitted.atoms().len()).filter(|index| mask & (1 << index) != 0),
    )
    .unwrap();
    let rendered = admitted
        .metadata()
        .observations()
        .render(
            &model,
            admitted.metadata().output(),
            zetesis_themelios::observation::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    // These fixtures have no whitespace inside a displayed atom.
    let mut atoms: Vec<_> = rendered
        .text()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    atoms.sort();
    let evaluated = zetesis_objective::evaluate(
        admitted.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    (
        atoms,
        evaluated.score().is_present().then(|| {
            evaluated
                .score()
                .costs()
                .iter()
                .map(|&(_, cost)| cost)
                .collect()
        }),
    )
}
fn complete(admitted: &AdmittedFormula) -> Vec<Record> {
    assert!(admitted.atoms().len() <= 12, "bounded exhaustive oracle");
    let theory = admitted.theory();
    let mut found = Vec::new();
    for mask in 0..1_usize << admitted.atoms().len() {
        let outer = values(theory, mask, None);
        if !holds(theory, &outer) {
            continue;
        }
        let mut subset = mask;
        let mut countermodel = false;
        while subset != 0 {
            subset = (subset - 1) & mask;
            if holds(theory, &values(theory, subset, Some(&outer))) {
                countermodel = true;
                break;
            }
        }
        if !countermodel {
            found.push(record(admitted, mask));
        }
    }
    found.sort();
    found
}
fn optimum(mut records: Vec<Record>) -> Vec<Record> {
    if let Some(best) = records.iter().map(|(_, cost)| cost).min().cloned() {
        records.retain(|(_, cost)| cost == &best);
    }
    records
}
fn reference(raw: &Json) -> Vec<Record> {
    assert_eq!(raw["Models"]["More"], "no");
    assert!(matches!(
        raw["Result"].as_str().unwrap(),
        "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND"
    ));
    let mut records: Vec<Record> = raw["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
        .map(|witness| {
            let mut atoms: Vec<_> = witness["Value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|atom| atom.as_str().unwrap().to_owned())
                .collect();
            atoms.sort();
            let costs = witness["Costs"]
                .as_array()
                .map(|costs| costs.iter().map(|cost| cost.as_i64().unwrap()).collect());
            (atoms, costs)
        })
        .collect();
    assert_eq!(
        raw["Models"]["Number"].as_u64().unwrap(),
        records.len() as u64
    );
    if raw["Result"] == "OPTIMUM FOUND" {
        let best: Vec<_> = raw["Models"]["Costs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cost| cost.as_i64().unwrap())
            .collect();
        let first = records
            .iter()
            .position(|(_, cost)| cost.as_ref() == Some(&best))
            .unwrap();
        assert!(
            records[first + 1..].contains(&records[first]),
            "optN incumbent recurrence"
        );
        records = records.into_iter().skip(first + 1).collect();
        assert!(records.iter().all(|(_, cost)| cost.as_ref() == Some(&best)));
        assert_eq!(
            raw["Models"]["Optimal"].as_u64().unwrap(),
            records.len() as u64
        );
    }
    records.sort();
    records
}

#[test]
fn explicit_single_group_expansions_match_complete_models_and_cost_presence() {
    let mut admitted_count = 0;
    let mut reference_count = 0;
    let mut model_count = 0;
    let mut optimum_count = 0;
    for case in cases().iter().filter(|case| case["native"] == "admit") {
        let label = case["name"].as_str().unwrap();
        let admitted = admit(case["source"].as_str().unwrap(), &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        let expanded = admit(
            case["expanded"].as_str().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{label} explicit expansion: {error}"));
        model_count += complete(&admitted).len();
        optimum_count += optimum(complete(&admitted)).len();
        assert_eq!(
            complete(&admitted),
            complete(&expanded),
            "{label}: every model/cost"
        );
        if !case["reference"].is_null() {
            assert_eq!(
                optimum(complete(&admitted)),
                reference(&case["reference"]),
                "{label}"
            );
            reference_count += 1;
        }
        assert_eq!(
            admitted.formula_origins().len(),
            admitted.theory().roots().len()
        );
        assert!(
            admitted
                .formula_origins()
                .iter()
                .all(|origins| !origins.is_empty())
        );
        admitted_count += 1;
    }
    assert_eq!((admitted_count, reference_count), (33, 26));
    assert_eq!((model_count, optimum_count), (80, 79));
    assert_ne!(
        complete(&admit("1{p(1..2)}1.", &FormulaLimits::default()).unwrap()),
        complete(&admit("1{p(1)}1.1{p(2)}1.", &FormulaLimits::default()).unwrap()),
        "bounds cannot be distributed"
    );
}

#[test]
fn tiny_expansions_match_every_frozen_pair_including_non_subsets() {
    let mut worlds = 0;
    for case in cases().iter().filter(|case| case["native"] == "admit") {
        let source = admit(case["source"].as_str().unwrap(), &FormulaLimits::default()).unwrap();
        let expanded = admit(
            case["expanded"].as_str().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap();
        assert_eq!(
            source.atoms().iter().collect::<BTreeSet<_>>(),
            expanded.atoms().iter().collect::<BTreeSet<_>>(),
            "{}: exact semantic atoms",
            case["name"]
        );
        if source.atoms().len() > 6 {
            continue;
        }
        for mask in 0..1_usize << source.atoms().len() {
            let outer = values(source.theory(), mask, None);
            let other = values(
                expanded.theory(),
                remap(mask, source.atoms(), expanded.atoms()),
                None,
            );
            assert_eq!(
                holds(source.theory(), &outer),
                holds(expanded.theory(), &other)
            );
            for inner in 0..1_usize << source.atoms().len() {
                assert_eq!(
                    holds(
                        source.theory(),
                        &values(source.theory(), inner, Some(&outer))
                    ),
                    holds(
                        expanded.theory(),
                        &values(
                            expanded.theory(),
                            remap(inner, source.atoms(), expanded.atoms()),
                            Some(&other)
                        )
                    ),
                    "{}: M={mask}, J={inner}",
                    case["name"]
                );
                worlds += 1;
            }
        }
    }
    assert_eq!(worlds, 10_439);
}

#[test]
fn nested_choice_values_retain_one_group() {
    // Keep the historical record untouched, but exercise its newly admitted
    // source against an independently written complete group.
    let cases = cases();
    let case = cases
        .iter()
        .find(|case| case["name"] == "nested_interval_still_refused")
        .unwrap();
    let source = admit(case["source"].as_str().unwrap(), &FormulaLimits::default()).unwrap();
    let expanded = admit("{p(f(1));p(f(2))}.", &FormulaLimits::default()).unwrap();
    assert_eq!(
        source.atoms().iter().collect::<BTreeSet<_>>(),
        expanded.atoms().iter().collect()
    );
    let expected: Vec<Record> = vec![
        (vec![], None),
        (vec!["p(f(1))".into()], None),
        (vec!["p(f(1))".into(), "p(f(2))".into()], None),
        (vec!["p(f(2))".into()], None),
    ];
    assert_eq!(complete(&source), expected);
    assert_eq!(complete(&source), complete(&expanded));
    assert_eq!(source.atoms().len(), 2);
    for outer in 0..4 {
        let frozen = values(source.theory(), outer, None);
        let other = values(
            expanded.theory(),
            remap(outer, source.atoms(), expanded.atoms()),
            None,
        );
        assert_eq!(
            holds(source.theory(), &frozen),
            holds(expanded.theory(), &other)
        );
        for inner in 0..4 {
            assert_eq!(
                holds(
                    source.theory(),
                    &values(source.theory(), inner, Some(&frozen))
                ),
                holds(
                    expanded.theory(),
                    &values(
                        expanded.theory(),
                        remap(inner, source.atoms(), expanded.atoms()),
                        Some(&other)
                    )
                ),
                "M={outer}, J={inner}",
            );
        }
    }
}

fn refused(error: &FormulaFailure, expected: &str) -> bool {
    match (expected, error) {
        ("UnsafeVariable", FormulaFailure::UnsafeVariable { .. })
        | (
            "Undefined",
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Undefined,
                ..
            }),
        )
        | (
            "Overflow",
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Overflow,
                ..
            }),
        )
        | (
            "Raise",
            FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Raise {
                ..
            })),
        ) => true,
        (
            _,
            FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                feature,
                ..
            })),
        ) => {
            let predicted = match expected {
                "Term" => ProfileFeature::Term,
                "PooledArguments" => ProfileFeature::PooledArguments,
                _ => return false,
            };
            *feature == predicted
        }
        _ => false,
    }
}
#[test]
fn excluded_endpoints_syntax_and_unsafe_scopes_remain_typed_refusals() {
    let mut count = 0;
    // Historical reference records stay immutable. These former refusals are
    // covered by evaluated_heads and finite_pools with explicit expansions.
    for case in cases().iter().filter(|case| {
        case["native"] != "admit"
            && case["native"] != "ObjectiveNegativeDependency"
            && !matches!(
                case["name"].as_str().unwrap(),
                "dependent_global_endpoint"
                    | "dependent_local_endpoint"
                    | "dependent_repeated_bound_and_argument"
                    | "closed_and_dependent_endpoints"
                    | "ordinary_head_expression_still_refused"
                    | "pooled_interval_argument"
                    | "pooled_argument_rows"
                    | "nested_interval_still_refused"
            )
    }) {
        let Err(error) = admit(case["source"].as_str().unwrap(), &FormulaLimits::default()) else {
            panic!("unexpected admission: {}", case["name"]);
        };
        assert!(
            refused(
                &error,
                if case["name"] == "dependent_unsafe_endpoint" {
                    "UnsafeVariable"
                } else if case["name"] == "supremum_endpoint" {
                    // Extremal values are admitted; a closed nonnumeric range
                    // endpoint remains outside the finite integer generator.
                    "Term"
                } else {
                    case["native"].as_str().unwrap()
                }
            ),
            "{}: expected {}, got {error:?}",
            case["name"],
            case["native"]
        );
        assert!(!error.diagnostics().is_empty());
        count += 1;
    }
    assert_eq!(count, 15);
    assert!(
        admit_extended(
            "1{p(1..2)}1.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err(),
        "explicit closure profile is unchanged"
    );
}

#[test]
fn interval_slots_obey_the_variable_limit() {
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 2;
    let admit = |source: &str| {
        admit_formula(
            source.into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
    };
    assert!(admit("1{p(1..2,1..2)}1.").is_ok());
    assert!(matches!(
        admit("d(3).1{p(X,1..2,1..2):d(X)}1."),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Variables,
            observed: 3,
            ..
        })
    ));
}

#[test]
fn generated_choice_values_obey_the_cumulative_limit() {
    let source = "{p(1..2)}.{q(3..4)}.";
    let limits = FormulaLimits {
        max_generated_values: 4,
        ..FormulaLimits::default()
    };
    assert!(admit(source, &limits).is_ok());
    assert!(matches!(
        admit(
            source,
            &FormulaLimits {
                max_generated_values: 3,
                ..limits
            }
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::GeneratedValues,
            observed: 4,
            ..
        })
    ));
}

#[test]
fn oversized_choice_range_is_refused_before_generation() {
    let limits = FormulaLimits::default();
    assert!(
        matches!(
            admit(
                "{p((-2147483647-1)..2147483647)}.",
                &FormulaLimits {
                    max_assignment_values: 3,
                    ..limits
                }
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::AssignmentValues,
                observed: 4_294_967_296,
                ..
            })
        ),
        "the full i32 range width refuses before allocating its denotation"
    );
}

#[test]
fn empty_joins_do_not_enumerate_choice_ranges() {
    assert!(
        admit(
            "{p((-2147483647-1)..2147483647)}:-missing.",
            &FormulaLimits {
                max_assignment_values: 0,
                ..FormulaLimits::default()
            }
        )
        .is_ok(),
        "a closed empty relational join never enumerates the range"
    );
}

#[test]
fn construction_and_streamed_substitution_work_refuse_before_partial_admission() {
    let source = "1{p(1..2,1..2)}1.";
    for resource in [FormulaResource::Substitutions, FormulaResource::Work] {
        let attempt = |limit| {
            let mut limits = FormulaLimits::default();
            if resource == FormulaResource::Work {
                limits.max_work = limit;
            } else {
                limits.max_substitutions = limit;
            }
            admit(source, &limits)
        };
        let threshold = first_success(|limit| attempt(limit).is_ok());
        assert!(matches!(attempt(threshold - 1), Err(FormulaFailure::Limit {
            resource: actual, observed, .. }) if actual == resource && observed == u128::from(threshold)));
        assert_eq!(complete(&attempt(threshold).unwrap()).len(), 4);
    }
    let attempt = |limit| {
        admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_term_work: usize::try_from(limit).unwrap(),
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
    };
    let threshold = first_success(|limit| attempt(limit).is_ok());
    assert!(matches!(
        attempt(threshold - 1),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::TermWork,
            ..
        }))
    ));
    let admitted = admit(source, &FormulaLimits::default()).unwrap();
    for (resource, limit) in [
        (FormulaResource::Atoms, admitted.atoms().len()),
        (FormulaResource::Nodes, admitted.theory().nodes().len()),
    ] {
        let mut limits = FormulaLimits::default();
        if resource == FormulaResource::Atoms {
            limits.theory.max_atoms = limit - 1;
        } else {
            limits.theory.max_nodes = limit - 1;
        }
        assert!(matches!(admit(source, &limits), Err(FormulaFailure::Limit {
            resource: actual, .. }) if actual == resource));
    }
}

#[test]
fn normalized_bundle_constants_and_duplicate_rules_keep_original_origins() {
    let directory = tempfile::tempdir().unwrap();
    let rule = "1{p(l..u)}1.";
    fs::write(
        directory.path().join("entry.lp"),
        format!("#include \"data.lp\".\n{rule}"),
    )
    .unwrap();
    fs::write(
        directory.path().join("data.lp"),
        format!("#const l=1.#const u=2.\n{rule}"),
    )
    .unwrap();
    let bundle =
        SourceBundle::load(directory.path().join("entry.lp"), BundleLimits::default()).unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(admitted.atoms().len(), 2);
    assert!(!admitted.formula_origins().is_empty());
    for origins in admitted.formula_origins() {
        let sources: BTreeSet<_> = origins.iter().map(|origin| origin.source).collect();
        assert_eq!(
            sources.len(),
            2,
            "canonical duplicate retains both parsed rule locations"
        );
        for origin in origins {
            let text = admitted
                .bundle()
                .get(origin.source)
                .unwrap()
                .source()
                .slice(origin.span)
                .unwrap();
            assert_eq!(text, rule, "synthetic slots have no fabricated source span");
        }
    }
}

#[test]
#[ignore = "requires clingo: fresh bounded clingo replays complete contracts and explicit diagnostics; excludes the recorded i32::MAX deadline"]
fn fresh_bounded_clingo_replays_complete_contracts_and_explicit_diagnostics() {
    let mut runs = 0;
    for case in cases().iter().filter(|case| !case["reference"].is_null()) {
        let fresh = optimal(case["source"].as_str().unwrap());
        assert_eq!(
            fresh["Result"], case["reference"]["Result"],
            "{}",
            case["name"]
        );
        if fresh["Result"] == "UNKNOWN" {
            assert_eq!(
                fresh["Models"]["Number"], 0,
                "unsafe input is no model evidence"
            );
        } else {
            assert_eq!(
                reference(&fresh),
                reference(&case["reference"]),
                "{}",
                case["name"]
            );
            if case["native"] == "admit" {
                assert_eq!(
                    optimum(complete(
                        &admit(case["source"].as_str().unwrap(), &FormulaLimits::default())
                            .unwrap()
                    )),
                    reference(&fresh),
                    "{}",
                    case["name"]
                );
            }
        }
        runs += 1;
    }
    assert_eq!(runs, 41);
}

#[test]
fn negative_choice_conditions_preserve_scored_answers() {
    let sources: Vec<_> = cases()
        .into_iter()
        .filter(|case| case["native"] == "ObjectiveNegativeDependency")
        .collect();
    assert_eq!(sources.len(), 1);
    for case in sources {
        objective_dependencies::check(case["source"].as_str().unwrap());
    }
}
