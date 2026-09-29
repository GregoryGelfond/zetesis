//! Default-negated heads preserve original formulas and only positive support.

use std::collections::BTreeSet;
use std::fs::{self};
use std::time::Duration;

use serde_json::Value as Json;
use zetesis_core::Sign;
use zetesis_reference_support::admit;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits, ExpansionFailure,
    ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource,
    SourceBundle, admit_bundle_formula, admit_formula,
};

type Names = BTreeSet<String>;
type Models = BTreeSet<Names>;

fn cases() -> Vec<Json> {
    serde_json::from_str(zetesis_test_support::fixtures::NEGATIVE_HEADS).unwrap()
}
fn singleton_cases() -> Vec<Json> {
    serde_json::from_str(zetesis_test_support::fixtures::SINGLETON_HEADS).unwrap()
}
fn limited(
    source: &str,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(source.into(), options, expansion, *limits)
}
fn name<'a>(atom: impl Into<zetesis_core::catalog::AtomRef<'a>>) -> String {
    let atom = atom.into();
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let base = format!("{sign}{}", atom.predicate().name());
    match (
        atom.values().len(),
        atom.values()
            .at(0)
            .map(zetesis_core::catalog::TermRef::descriptor),
    ) {
        (0, _) => base,
        (1, Some(zetesis_core::ValueNodeRef::Number(number))) => format!("{base}({number})"),
        _ => panic!("fixture has nullary or numeric unary atoms: {atom:?}"),
    }
}
fn names(value: &Json) -> Names {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect()
}
fn expected(value: &Json) -> Models {
    value.as_array().unwrap().iter().map(names).collect()
}
// Independent topological evaluation: every subtree false in M is falsum in
// F^M. No production evaluator, reduct mask, SAT search or subset enumeration
// helper is used to decide these finite stable models.
fn selected(admitted: &AdmittedFormula, mask: usize) -> Names {
    admitted
        .atoms()
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, atom)| name(atom))
        .collect()
}
fn complete(admitted: &AdmittedFormula) -> Models {
    assert!(admitted.atoms().len() <= 6, "tiny exhaustive carrier");
    let mut result = Models::new();
    for mask in 0..1_usize << admitted.atoms().len() {
        let outer = values(admitted.theory(), mask, None);
        if !holds(admitted.theory(), &outer) {
            continue;
        }
        let mut subset = mask;
        let mut countermodel = false;
        while subset != 0 {
            subset = (subset - 1) & mask;
            if holds(
                admitted.theory(),
                &values(admitted.theory(), subset, Some(&outer)),
            ) {
                countermodel = true;
                break;
            }
        }
        if !countermodel {
            assert!(result.insert(selected(admitted, mask)));
        }
    }
    result
}

fn truth(formula: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    if frozen.is_some_and(|outer| !truth(formula, outer, None)) {
        return false;
    }
    if let Some(atom) = formula.as_str() {
        return tested.contains(atom);
    }
    if let Json::Bool(value) = formula {
        return *value;
    }
    let left = truth(&formula[1], tested, frozen);
    let right = truth(&formula[2], tested, frozen);
    match formula[0].as_str().unwrap() {
        "and" => left && right,
        "or" => left || right,
        "imp" => !left || right,
        other => panic!("unknown manual formula {other}"),
    }
}
fn manual_holds(theory: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    theory["roots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|root| truth(root, tested, frozen))
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
        let actual_atoms: Names = admitted.atoms().iter().map(name).collect();
        // A positive join with no possible rows may erase a vacuous source rule;
        // compare it separately through full models, not a fabricated DAG carrier.
        if case["name"] == "recursive-body" {
            assert_eq!(complete(&admitted), expected(&case["models"]));
            continue;
        }
        assert_eq!(actual_atoms, names(&case["atoms"]), "{source}: carrier");
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
#[ignore = "requires external clingo 5.8; bounded original and explicit-expansion captures"]
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
            admitted.atoms().iter().map(name).collect::<Names>(),
            names(&case["atoms"])
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
#[ignore = "requires external clingo 5.8; bounded complete captures"]
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
use crate::support::finite_bindings::{holds, values};
use crate::support::objective_boundaries;
use zetesis_clingo_support as oracle;

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
        let mut limits = FormulaLimits::default();
        limits.theory.max_nodes = admitted.theory().nodes().len();
        assert_eq!(
            complete(
                &limited(
                    source,
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    &limits
                )
                .unwrap()
            ),
            complete(&admitted)
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
        for (work, expansion_work) in [(0, usize::MAX), (u64::MAX, 0)] {
            let limits = FormulaLimits {
                max_work: work,
                ..FormulaLimits::default()
            };
            let expansion = ExpansionLimits {
                max_term_work: expansion_work,
                ..ExpansionLimits::default()
            };
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
    let limits = FormulaLimits {
        max_disjunction_elements: 2,
        ..FormulaLimits::default()
    };
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
        let limits = FormulaLimits {
            max_disjunction_elements: 1,
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
        let limits = FormulaLimits {
            max_disjunction_elements: 0,
            ..limits
        };
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
    let directory = tempfile::tempdir().unwrap();
    let rule = "not not a(1..2)|b.";
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
