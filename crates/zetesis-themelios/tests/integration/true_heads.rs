//! Explicitly true/empty conditions retain signed whole-rule expansion semantics.

use std::collections::BTreeSet;
use std::fs::{self};

use crate::support::finite_bindings::{holds, remap, values};
use crate::support::head_models::{clingo, complete, expected, limited, manual_holds, selected};
use crate::support::objective_boundaries;
use crate::support::thresholds::first_success;
use serde_json::{Value as Json, json};
use zetesis_reference_support::admit;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionOptions, BundleLimits, ExpansionFailure,
    ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, InputLimit,
    SourceBundle, admit_bundle_formula, admit_extended,
};

fn cases() -> Vec<Json> {
    serde_json::from_str(include_str!("../fixtures/true-heads.json")).unwrap()
}
// Independent topological evaluation: every subtree false in M is falsum in
// F^M. No production evaluator, reduct mask, SAT search or subset enumeration
// helper is used to decide these finite stable models.

#[test]
fn complete_models_match_explicit_families_and_recorded_reference_expectations() {
    let cases = cases();
    assert_eq!(cases.len(), 25);
    for case in cases {
        let source = admit(case["source"].as_str().unwrap(), &FormulaLimits::default()).unwrap();
        let expanded = admit(
            case["expanded"].as_str().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap();
        let predicted = expected(&case["models"]);
        assert_eq!(complete(&source), predicted, "{}: source", case["name"]);
        assert_eq!(complete(&expanded), predicted, "{}: expanded", case["name"]);
    }
}

#[test]
fn source_expansions_preserve_every_original_and_frozen_pair() {
    let mut pairs = 0;
    for case in cases() {
        let source = admit(case["source"].as_str().unwrap(), &FormulaLimits::default()).unwrap();
        let expanded = admit(
            case["expanded"].as_str().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap();
        assert_eq!(
            source.atoms().iter().collect::<BTreeSet<_>>(),
            expanded.atoms().iter().collect::<BTreeSet<_>>(),
            "{}: exact carrier",
            case["name"]
        );
        assert!(source.atoms().len() <= 6);
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
                pairs += 1;
            }
        }
    }
    assert!(pairs > 1_000);
}
// Every M-false subtree becomes falsum, including non-atomic implications.
// This evaluates JSON trees directly, without a production DAG or compiler.
#[test]
fn true_head_range_products_match_handwritten_frozen_formulas() {
    for (source, manual) in [
        (
            "p(1..2):#true;q:#true.",
            json!({"roots": [["or", ["and", "p(1)", "p(2)"], "q"]]}),
        ),
        (
            "p(1..2):#true;q(1..2):#true.",
            json!({"roots": [["or", ["and", "p(1)", "p(2)"], ["and", "q(1)", "q(2)"]]]}),
        ),
        ("p(2..1):#true;q:#true.", json!({"roots": []})),
        ("a|b:.", json!({"roots": [["or", "a", "b"]]})),
    ] {
        let admitted = admit(source, &FormulaLimits::default()).unwrap();
        for outer in 0..1_usize << admitted.atoms().len() {
            let candidate = selected(&admitted, outer);
            let frozen = values(admitted.theory(), outer, None);
            assert_eq!(
                holds(admitted.theory(), &frozen),
                manual_holds(&manual, &candidate, None)
            );
            for inner in 0..1_usize << admitted.atoms().len() {
                assert_eq!(
                    holds(
                        admitted.theory(),
                        &values(admitted.theory(), inner, Some(&frozen))
                    ),
                    manual_holds(&manual, &selected(&admitted, inner), Some(&candidate)),
                    "{source}: M={outer}, J={inner}",
                );
            }
        }
    }
}

#[test]
fn conditional_disjunctions_preserve_finite_families() {
    for (source, models) in [
        ("p:#false;q:#true.", json!([["q"]])),
        ("p:not #true;q:#true.", json!([["q"]])),
        ("p:not not #false;q:#true.", json!([["q"]])),
        ("p:a;q:#true.", json!([["q"]])),
        ("p:not a;q:#true.", json!([["p"], ["q"]])),
        ("p:not not a;q:#true.", json!([["q"]])),
        ("p:1=1;q:#true.", json!([["p"], ["q"]])),
        ("p:#true,a;q:#true.", json!([["q"]])),
        ("p(X):X=1..2;q:#true.", json!([["p(1)"], ["p(2)"], ["q"]])),
    ] {
        assert_eq!(
            complete(&admit(source, &FormulaLimits::default()).unwrap()),
            expected(&models),
            "{source}"
        );
    }
}

#[test]
fn true_disjuncts_preserve_scored_answers() {
    let source = "p:#true;q:#true.#minimize{1:q}.";
    objective_boundaries::check(source);
}

#[test]
fn extended_profile_refuses_true_disjunctions() {
    assert!(
        admit_extended(
            "p:#true;q:#true.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err()
    );
}

#[test]
fn erased_conditions_do_not_bind_or_hide_unsafe_head_arguments() {
    for source in [
        "p(X):#true;q:#true.",
        "p(_):#true;q:#true.",
        "p(2..1,X):#true;q:#true.",
    ] {
        assert!(
            matches!(
                admit(source, &FormulaLimits::default()),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn condition_and_head_counts_have_inclusive_limits() {
    let source = "p:#true,#true;q:#true.";
    let mut options = AdmissionOptions {
        max_body_elements: 2,
        ..AdmissionOptions::default()
    };
    assert!(
        limited(
            source,
            options,
            ExpansionLimits::default(),
            &FormulaLimits::default()
        )
        .is_ok()
    );
    options.max_body_elements = 1;
    assert!(matches!(
        limited(
            source,
            options,
            ExpansionLimits::default(),
            &FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Limit {
                resource: InputLimit::BodyElements,
                limit: 1,
                observed: 2,
                ..
            }
        )))
    ));
    let limits = FormulaLimits {
        max_disjunction_elements: 2,
        ..FormulaLimits::default()
    };
    assert!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &limits
        )
        .is_ok()
    );
    assert!(matches!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &FormulaLimits {
                max_disjunction_elements: 1,
                ..limits
            }
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::DisjunctionElements,
            limit: 1,
            observed: 2,
            ..
        })
    ));
}

#[test]
fn erased_conditions_do_not_consume_synthetic_variable_slots() {
    let source = "d(1).p(X+1):#true;q:#true:-d(X).";
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 2;
    assert!(
        limited(
            source,
            options,
            ExpansionLimits::default(),
            &FormulaLimits::default()
        )
        .is_ok()
    );
    options.core_limits.max_variables_per_template = 1;
    assert!(matches!(
        limited(
            source,
            options,
            ExpansionLimits::default(),
            &FormulaLimits::default()
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Variables,
            limit: 1,
            observed: 2,
            ..
        })
    ));
}

#[test]
fn construction_work_substitutions_and_nodes_fail_at_exact_boundaries() {
    let source = "d(1).p(X+1):#true;q:#true:-d(X).";
    for resource in [FormulaResource::Work, FormulaResource::Substitutions] {
        let attempt = |limit| {
            let mut limits = FormulaLimits::default();
            if resource == FormulaResource::Work {
                limits.max_work = limit;
            } else {
                limits.max_substitutions = limit;
            }
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                &limits,
            )
        };
        let threshold = first_success(|limit| attempt(limit).is_ok());
        assert!(
            matches!(attempt(threshold-1), Err(FormulaFailure::Limit { resource: actual, observed, .. })
            if actual == resource && observed == u128::from(threshold))
        );
        assert_eq!(
            complete(&attempt(threshold).unwrap()),
            complete(&admit(source, &FormulaLimits::default()).unwrap())
        );
    }
    let attempt = |limit| {
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits {
                max_term_work: usize::try_from(limit).unwrap(),
                ..ExpansionLimits::default()
            },
            &FormulaLimits::default(),
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
    let mut limits = FormulaLimits::default();
    limits.theory.max_nodes = admitted.theory().nodes().len();
    assert!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &limits
        )
        .is_ok()
    );
    limits.theory.max_nodes -= 1;
    assert!(matches!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &limits
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Nodes,
            ..
        })
    ));
}

#[test]
fn duplicate_included_true_heads_retain_each_source_origin() {
    let directory = tempfile::tempdir().unwrap();
    let rule = "p(1..2):#true;q:#true.";
    fs::write(
        directory.path().join("entry.lp"),
        format!("#include \"other.lp\".\n{rule}"),
    )
    .unwrap();
    fs::write(directory.path().join("other.lp"), rule).unwrap();
    let bundle =
        SourceBundle::load(directory.path().join("entry.lp"), BundleLimits::default()).unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert!(!admitted.formula_origins().is_empty());
    for origins in admitted.formula_origins() {
        assert_eq!(
            origins
                .iter()
                .map(|origin| origin.source)
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
        for origin in origins {
            assert_eq!(
                admitted
                    .bundle()
                    .get(origin.source)
                    .unwrap()
                    .source()
                    .slice(origin.span)
                    .unwrap(),
                rule
            );
        }
    }
}

#[test]
#[ignore = "requires external clingo 5.8; each original and expansion has a bounded complete capture"]
fn fresh_clingo_original_and_expanded_sources_match_complete_models() {
    for case in cases() {
        let predicted = expected(&case["models"]);
        for field in ["source", "expanded"] {
            assert_eq!(
                clingo(case[field].as_str().unwrap()),
                predicted,
                "{}: {field}",
                case["name"]
            );
        }
    }
}
