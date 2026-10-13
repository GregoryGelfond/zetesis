//! Default-negated heads preserve original formulas and only positive support.

use std::collections::BTreeSet;

use crate::support::finite_bindings::{Models, holds, values};
use crate::support::head_models::{
    Names, clingo, complete, expected, limited, manual_holds, selected, truth,
};
use crate::support::objective_boundaries;
use crate::support::resource_limits;
use serde_json::Value as Json;
use zetesis_clingo_support as oracle;
use zetesis_reference_support::{BundleFixture, admit, canonical};
use zetesis_themelios::{
    AdmissionOptions, BundleAdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_bundle_formula,
};

fn cases() -> Vec<Json> {
    serde_json::from_str(zetesis_test_support::fixtures::NEGATIVE_HEADS).unwrap()
}
fn singleton_cases() -> Vec<Json> {
    serde_json::from_str(zetesis_test_support::fixtures::SINGLETON_HEADS).unwrap()
}
// Independent topological evaluation (support::head_models): every subtree
// false in M is falsum in F^M. No production evaluator, reduct mask, SAT search
// or subset enumeration helper is used to decide these finite stable models.

fn supported(case: &Json, model: &Names) -> bool {
    model.iter().all(|atom| {
        case["support"]
            .get(atom)
            .is_some_and(|antecedent| truth(antecedent, model, None))
    })
}

#[test]
fn original_support_augmentation_and_frozen_formulas_are_distinct_contracts() {
    let mut pairs = 0;
    for case in cases() {
        let source = case["source"].as_str().unwrap();
        let admitted = admit(source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let actual_atoms: Names = admitted.atoms().iter().map(canonical).collect();
        // A positive join with no possible rows may erase a vacuous source rule;
        // compare it separately through full models, not a fabricated DAG carrier.
        if case["name"] == "recursive-body" {
            assert_eq!(complete(&admitted), expected(&case["models"]));
            continue;
        }
        assert_eq!(
            actual_atoms,
            oracle::atoms(&case["atoms"]),
            "{source}: carrier"
        );
        for mask in 0..1_usize << admitted.atoms().len() {
            let model = selected(&admitted, mask);
            let outer = values(admitted.theory(), mask, None);
            let has_support = supported(&case, &model);
            assert_eq!(
                holds(admitted.theory(), &outer),
                manual_holds(&case, &model, None) && has_support,
                "{source}: original M={mask}"
            );
            if !has_support {
                continue;
            }
            // The added double-negated support guards are true in every J when
            // they hold in M. This compares arbitrary J, not just proper subsets.
            for inner in 0..1_usize << admitted.atoms().len() {
                assert_eq!(
                    holds(
                        admitted.theory(),
                        &values(admitted.theory(), inner, Some(&outer))
                    ),
                    manual_holds(&case, &selected(&admitted, inner), Some(&model)),
                    "{source}: frozen M={mask}, J={inner}"
                );
                pairs += 1;
            }
        }
    }
    assert!(pairs > 300);
}

#[test]
fn complete_signed_models_match_handwritten_theories_and_explicit_expansions() {
    let mut count = 0;
    for case in cases() {
        let predicted = expected(&case["models"]);
        for field in ["source", "expanded"] {
            let admitted = admit(case[field].as_str().unwrap(), &FormulaLimits::default()).unwrap();
            assert_eq!(complete(&admitted), predicted, "{}: {field}", case["name"]);
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
        }
        count += predicted.len();
    }
    assert_eq!(cases().len(), 27);
    assert_eq!(count, 49);
}

#[test]
#[ignore = "requires clingo: fresh clingo preserves every complete negative head contract; bounded original and explicit-expansion captures"]
fn fresh_clingo_preserves_every_complete_negative_head_contract() {
    for case in cases() {
        for field in ["source", "expanded"] {
            assert_eq!(
                clingo(case[field].as_str().unwrap()),
                expected(&case["models"]),
                "{}: {field}",
                case["name"]
            );
        }
    }
}

#[test]
fn singleton_heads_preserve_original_formulas() {
    for case in singleton_cases() {
        let source = case["source"].as_str().unwrap();
        let admitted = admit(source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(
            admitted.atoms().iter().map(canonical).collect::<Names>(),
            oracle::atoms(&case["atoms"])
        );
        for mask in 0..1_usize << admitted.atoms().len() {
            let model = selected(&admitted, mask);
            assert_eq!(
                holds(admitted.theory(), &values(admitted.theory(), mask, None)),
                manual_holds(&case, &model, None) && supported(&case, &model),
                "{source}: M={model:?}"
            );
        }
    }
}

#[test]
fn singleton_heads_preserve_frozen_formulas() {
    for case in singleton_cases() {
        let source = case["source"].as_str().unwrap();
        let admitted = admit(source, &FormulaLimits::default()).unwrap();
        for mask in 0..1_usize << admitted.atoms().len() {
            let model = selected(&admitted, mask);
            // Added double-negated support guards impose only the outer test.
            if !supported(&case, &model) {
                continue;
            }
            let outer = values(admitted.theory(), mask, None);
            for inner in 0..1_usize << admitted.atoms().len() {
                assert_eq!(
                    holds(
                        admitted.theory(),
                        &values(admitted.theory(), inner, Some(&outer))
                    ),
                    manual_holds(&case, &selected(&admitted, inner), Some(&model)),
                    "{source}: M={mask}, J={inner}"
                );
            }
        }
    }
}

#[test]
fn singleton_heads_preserve_complete_models() {
    let cases = singleton_cases();
    let mut records = 0;
    for case in &cases {
        let source = case["source"].as_str().unwrap();
        let actual = complete(&admit(source, &FormulaLimits::default()).unwrap());
        assert_eq!(actual, expected(&case["models"]), "{source}");
        records += actual.len();
    }
    assert_eq!((cases.len(), records), (12, 19));
}

#[test]
#[ignore = "requires clingo: singleton references match clingo; bounded complete captures"]
fn singleton_references_match_clingo() {
    for case in singleton_cases() {
        assert_eq!(
            clingo(case["source"].as_str().unwrap()),
            expected(&case["models"]),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn negative_heads_do_not_bind_variables() {
    for source in [
        "not a(X)|b.",
        "not not a(X)|b.",
        "not a(X+1)|b.",
        "not a(2..1,X)|b.",
        "d(1).not a(Y)|b(X):-d(X).",
        "not a(X).",
        "not not a(X).",
        "not a(X+1).",
        "d(1).not a(Y):-d(X).",
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
fn conditional_negative_disjuncts_require_eligibility() {
    let source = "not a:b|c.";
    assert_eq!(
        complete(&admit(source, &FormulaLimits::default()).unwrap()),
        Models::from([Names::from(["c".into()])])
    );
}

#[test]
fn negative_head_producers_preserve_scored_answers() {
    for source in [
        "not a|b.#minimize{1:a}.",
        "not not a|b.c:-a.#minimize{1:c}.",
    ] {
        objective_boundaries::check(source);
    }
}

#[test]
fn negation_nodes_and_occurrences_obey_exact_resource_boundaries() {
    for source in [
        "not a|not not a|a.",
        "d(1).not a(X+1)|b:-d(X).",
        "not not p(1..2)|q.",
        "not a.",
        "not not a.",
        "d(1).not a(X+1):-d(X).",
        "not not p(1..2).",
    ] {
        let admitted = admit(source, &FormulaLimits::default()).unwrap();
        let nodes = u64::try_from(admitted.theory().nodes().len()).unwrap();
        let limits = |maximum| resource_limits::formula(FormulaResource::Nodes, maximum);
        assert_eq!(
            complete(
                &limited(
                    source,
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    &limits(nodes)
                )
                .unwrap()
            ),
            complete(&admitted)
        );
        assert!(matches!(
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                &limits(nodes - 1)
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Nodes,
                ..
            })
        ));
        for (work, expansion_work) in [(0, u64::MAX), (u64::MAX, 0)] {
            let limits = resource_limits::formula(FormulaResource::Work, work);
            let expansion = resource_limits::expansion(ExpansionResource::TermWork, expansion_work);
            assert!(matches!(
                limited(source, AdmissionOptions::default(), expansion, &limits),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                } | FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::TermWork,
                    ..
                }))
            ));
        }
    }
    let limits = resource_limits::formula(FormulaResource::DisjunctionElements, 2);
    assert!(
        limited(
            "a|not a.",
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &limits
        )
        .is_ok()
    );
    assert!(matches!(
        limited(
            "a|not a|not not a.",
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &limits
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::DisjunctionElements,
            observed: 3,
            ..
        })
    ));
}

#[test]
fn singleton_head_limit_counts_one_literal() {
    for source in ["not a.", "not not a."] {
        let limits = resource_limits::formula(FormulaResource::DisjunctionElements, 1);
        assert!(
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                &limits
            )
            .is_ok()
        );
        let limits = resource_limits::formula(FormulaResource::DisjunctionElements, 0);
        assert!(matches!(
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                &limits
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::DisjunctionElements,
                observed: 1,
                ..
            })
        ));
    }
}

#[test]
fn negative_heads_keep_original_duplicate_bundle_origins() {
    let fixture = BundleFixture::new();
    let rule = "not not a(1..2)|b.";
    fixture.write("entry.lp", &format!("#include \"other.lp\".\n{rule}"));
    fixture.write("other.lp", rule);
    let bundle = fixture.bundle();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    for origins in admitted.formula_origins() {
        assert_eq!(
            origins
                .iter()
                .map(|origin| origin.location().expect("parsed source").source)
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
        for origin in origins {
            assert_eq!(
                admitted
                    .bundle()
                    .get(origin.location().expect("parsed source").source)
                    .unwrap()
                    .source()
                    .slice(origin.location().expect("parsed source").span)
                    .unwrap(),
                rule
            );
        }
    }
}
