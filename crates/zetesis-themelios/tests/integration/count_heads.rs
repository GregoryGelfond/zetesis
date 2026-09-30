//! Checked finite tuple/atom count heads preserve complete models and frozen reducts.

use crate::support::finite_bindings::{Models, holds, remap, values};
use crate::support::head_models::{complete, expected, limited, manual_holds, names, selected};
use crate::support::objective_dependency_records as objective_dependencies;
use crate::support::thresholds::first_success;

use std::collections::BTreeSet;
use std::fs::{self};
use std::time::Duration;

use serde_json::{Value as Json, json};
use zetesis_clingo_support as oracle;
use zetesis_reference_support::{admit, canonical};
use zetesis_themelios::{
    AdmissionOptions, BundleAdmissionOptions, BundleLimits, ExpansionFailure, ExpansionLimits,
    ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, SourceBundle,
    admit_bundle_formula, admit_extended,
};

fn cases() -> Vec<Json> {
    let mut cases: Vec<Json> =
        serde_json::from_str(include_str!("../fixtures/count-heads.json")).unwrap();
    cases.extend(
        serde_json::from_str::<Vec<Json>>(include_str!("../fixtures/count-head-eligibility.json"))
            .unwrap(),
    );
    cases
}
// Independent topological evaluation: every subtree false in M is falsum in
// F^M. No production evaluator, reduct mask, SAT search or subset enumeration
// helper is used to decide these finite stable models.

#[test]
fn count_heads_preserve_stable_models() {
    assert_eq!(cases().len(), 46);
    for case in cases() {
        let source = admit(case["source"].as_str().unwrap(), &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let expanded = admit(
            case["expanded"].as_str().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap();
        assert_eq!(
            complete(&source),
            expected(&case["models"]),
            "{}: source",
            case["name"]
        );
        assert_eq!(
            complete(&expanded),
            expected(&case["models"]),
            "{}: choice",
            case["name"]
        );
    }
}

#[test]
fn native_count_search_preserves_full_models() {
    for case in cases() {
        let admitted = admit(case["source"].as_str().unwrap(), &FormulaLimits::default()).unwrap();
        let mut search = zetesis_sat::StableModels::new(
            admitted.theory(),
            zetesis_sat::Limits::default(),
            zetesis_cpu::Cancellation::default(),
        )
        .unwrap();
        let mut models = Models::new();
        for model in search.by_ref() {
            assert!(
                models.insert(
                    model
                        .unwrap()
                        .atoms()
                        .map(|index| canonical(admitted.atoms().at(index).unwrap()))
                        .collect()
                )
            );
        }
        assert!(
            search.exhausted(),
            "{}: incomplete enumeration",
            case["name"]
        );
        assert_eq!(models, expected(&case["models"]), "{}", case["name"]);
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
    assert_eq!(pairs, 1024);
    println!("count_sources={} frozen_pairs={pairs}", cases().len());
}
// Every M-false subtree becomes falsum, including non-atomic implications.
// This evaluates JSON trees directly, without a production DAG or compiler.
#[test]
fn count_bounds_preserve_frozen_formulas() {
    for (source, manual) in [
        (
            "{q}.1#count{1:p:q}1.",
            json!({"roots": [
                ["or","q",["imp","q",false]],
                ["imp","q",["or","p",["imp","p",false]]],
                ["imp",["imp",["and","q","p"],false],false]
            ]}),
        ),
        (
            "{b;c}.1#count{1:a:b;1:a:c}1.",
            json!({"roots": [
                ["or","b",["imp","b",false]],
                ["or","c",["imp","c",false]],
                ["imp",["or","b","c"],["or","a",["imp","a",false]]],
                ["imp",["imp",["and","a",["or","b","c"]],false],false]
            ]}),
        ),
        (
            "{d}.N{a}N:-N=#count{1:d}.",
            json!({"roots": [
                ["or","d",["imp","d",false]],
                ["imp","d",["or","a",["imp","a",false]]],
                ["imp",["imp","d",false],["or","a",["imp","a",false]]],
                ["imp",["and","d",["imp","a",false]],false],
                ["imp",["and",["imp","d",false],"a"],false]
            ]}),
        ),
        (
            "1#count{1:p;2:q}1.",
            json!({"roots": [
                ["or","p",["imp","p",false]], ["or","q",["imp","q",false]],
                ["imp",["imp",["or","p","q"],false],false], ["imp",["and","p","q"],false]
            ]}),
        ),
        ("1#count{}1.", json!({"roots": [false]})),
        ("0#count{}0.", json!({"roots": []})),
        ("1#count{1:a}1:-a.", json!({"roots": []})),
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
                    "{source}: M={outer}, J={inner}"
                );
            }
        }
    }
}

#[test]
fn count_aliases_admit_complete_groups() {
    for source in [
        "1#count{1:a;1:b}1.",
        "1#count{1:a;2:a}1.",
        "1#count{X:p:X=1..2}1.",
        "1#count{1:p(1..2)}1.",
        "1#count{1:p;1:q;2:r}1.",
        "1#count{1:p;2:q;3:p}1.",
        "{b;c}.1#count{1:a:b;1:d:c}1.",
        "{b;c}.1#count{1:a:b;2:a:c}1.",
        "1#count{1:a:b;1:d:c}1.b.c:-b.",
        "d(1).d(2).1#count{X:a:d(X)}1.",
    ] {
        assert!(admit(source, &FormulaLimits::default()).is_ok(), "{source}");
    }
}

#[test]
fn count_heads_preserve_scored_answers() {
    objective_dependencies::check("1#count{1:a}1.#minimize{1:a}.");
}

#[test]
fn extended_profile_refuses_count_heads() {
    assert!(
        admit_extended(
            "1#count{X:p(X):X=1..4}2.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err()
    );
}

#[test]
fn tuples_and_derived_atoms_never_supply_safety() {
    for source in [
        "1#count{X:p(X)}1.",
        "1#count{X:p:X=X}1.",
        "1#count{_:p}1.",
        "N#count{X:p(X):X=1..4}N.",
        "1#count{X:p(X):X=2..1,Y=Y}1.",
    ] {
        let error = admit(source, &FormulaLimits::default()).expect_err(source);
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
    }
}

#[test]
fn checked_group_size_and_tuple_storage_obey_inclusive_limits() {
    let source = "1#count{X:p(X):X=1..4}2.";
    for source in [
        source,
        "#count{X:p(X):X=1..4}.",
        "d(1..4).1#count{X:p(X):d(X)}2.",
    ] {
        let mut limits = FormulaLimits::default();
        limits.aggregate.max_elements = 4;
        assert!(
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                &limits
            )
            .is_ok()
        );
        limits.aggregate.max_elements = 3;
        assert!(matches!(
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                &limits
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::AggregateElements,
                limit: 3,
                observed: 4,
                ..
            })
        ));
    }
    let attempt = |limit| {
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits {
                max_scalar_bytes: usize::try_from(limit).unwrap(),
                ..ExpansionLimits::default()
            },
            &FormulaLimits::default(),
        )
    };
    let threshold = first_success(|limit| attempt(limit).is_ok());
    assert!(matches!(
        attempt(threshold - 1),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            ..
        }))
    ));
    assert_eq!(
        complete(&attempt(threshold).unwrap()),
        complete(&admit(source, &FormulaLimits::default()).unwrap())
    );
}

#[test]
fn validation_limits_fail_before_partial_publication() {
    validation_limits("1#count{X:p(X):X=1..4}2.");
}

#[test]
fn eligibility_limits_fail_before_partial_publication() {
    validation_limits("d(1..2).1#count{X:p(X):d(X)}1.");
}

fn validation_limits(source: &str) {
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
        println!("source={source} inclusive_{resource:?}={threshold}");
    }
}

#[test]
fn duplicate_included_count_groups_retain_each_source_origin() {
    let directory = tempfile::tempdir().unwrap();
    let rule = "1#count{X:p(X):X=1..4}2.";
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

fn clingo(source: &str) -> Models {
    let run = oracle::run(
        source,
        &["--models=0", "--outf=2"],
        oracle::Limits {
            timeout: Duration::from_secs(5),
            max_output_bytes: 1_048_576 + 65_536,
        },
    );
    let stdout = String::from_utf8_lossy(run.stdout());
    let stderr = String::from_utf8_lossy(run.stderr());
    println!(
        "{}",
        json!({"source":source,"exit":run.code(),"stdout":stdout,"stderr":stderr})
    );
    let raw = oracle::json(&run);
    assert!(
        raw["Solver"]
            .as_str()
            .unwrap()
            .starts_with("clingo version 5.8.")
    );
    assert!(matches!(
        raw["Result"].as_str().unwrap(),
        "SATISFIABLE" | "UNSATISFIABLE"
    ));
    assert_eq!(raw["Models"]["More"], "no");
    let witnesses: Vec<_> = raw["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
        .collect();
    assert_eq!(
        raw["Models"]["Number"].as_u64().unwrap(),
        witnesses.len() as u64
    );
    let models: Models = witnesses
        .iter()
        .map(|witness| {
            assert!(witness["Costs"].is_null());
            names(&witness["Value"])
        })
        .collect();
    assert_eq!(
        models.len(),
        witnesses.len(),
        "fixtures show complete models without projection"
    );
    models
}
#[test]
#[ignore = "requires clingo: fresh clingo original and expanded sources match complete models; each original and expansion has a bounded complete capture"]
fn fresh_clingo_original_and_expanded_sources_match_complete_models() {
    let mut models = 0;
    for case in cases() {
        let predicted = expected(&case["models"]);
        models += predicted.len();
        for field in ["source", "expanded"] {
            assert_eq!(
                clingo(case[field].as_str().unwrap()),
                predicted,
                "{}: {field}",
                case["name"]
            );
        }
    }
    println!(
        "count_sources={} full_models={models} source_and_expansion_runs={}",
        cases().len(),
        2 * cases().len()
    );
}
