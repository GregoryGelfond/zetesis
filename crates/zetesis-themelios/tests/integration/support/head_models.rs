//! The head propositions' reference: clingo's models and the models an exhaustive
//! reduct check finds, as sets of atom spellings, and the admission they share.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value as Json;
use zetesis_clingo_support as oracle;
use zetesis_reference_support::{admit, canonical};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

use super::finite_bindings::{Models, holds, remap, values};

/// A model as the set of its atoms' spellings.
pub(crate) type Names = BTreeSet<String>;

/// The models a fixture records, each a list of atom spellings.
pub(crate) fn expected(value: &Json) -> Models {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(oracle::atoms)
        .collect()
}

/// `source` admitted with the given options and limits.
pub(crate) fn limited(
    source: &str,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(source.into(), options, expansion, *limits)
}

/// Whether the fixture formula `formula` holds in `tested`: in the reduct by
/// `frozen` when one is given.
pub(crate) fn truth(formula: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    if frozen.is_some_and(|outer| !truth(formula, outer, None)) {
        return false;
    }
    if let Some(atom) = formula.as_str() {
        return tested.contains(atom);
    }
    if formula == &Json::Bool(false) {
        return false;
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

/// clingo's models of `source`, from a decided run of clingo 5.8 that
/// enumerated them all without projection or costs.
pub(crate) fn clingo(source: &str) -> Models {
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
    oracle::objective_free_models(&raw)
}

/// Every answer set of `admitted` (at most six atoms), by the exhaustive reduct
/// check.
pub(crate) fn complete(admitted: &AdmittedFormula) -> Models {
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

/// Whether every root of the fixture theory `theory` holds in `tested`: in the
/// reduct by `frozen` when one is given.
pub(crate) fn manual_holds(theory: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    theory["roots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|root| truth(root, tested, frozen))
}

/// The spellings of `admitted`'s atoms whose bits `mask` sets.
pub(crate) fn selected(admitted: &AdmittedFormula, mask: usize) -> Names {
    admitted
        .atoms()
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, atom)| canonical(atom))
        .collect()
}

/// Every case's source and its explicit expansion (`source` and `expanded`)
/// have the same atoms and agree on satisfaction in each interpretation M and,
/// for each J, on whether J satisfies the reduct by M; the fixture checks more
/// than a thousand such pairs.
pub(crate) fn expansions_preserve_every_original_and_frozen_pair(cases: &[Json]) {
    let mut pairs = 0;
    for case in cases {
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

/// Clingo's complete models of every case's source and of its expansion are
/// the ones the case records.
pub(crate) fn clingo_matches_every_source_and_expansion(cases: &[Json]) {
    for case in cases {
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
