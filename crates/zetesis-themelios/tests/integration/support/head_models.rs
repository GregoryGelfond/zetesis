//! The head propositions' reference: clingo's models and the models an exhaustive
//! reduct check finds, as sets of atom spellings, and the admission they share.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value as Json;
use zetesis_clingo_support as oracle;
use zetesis_reference_support::canonical;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

use super::finite_bindings::{Models, holds, values};

/// A model as the set of its atoms' spellings.
pub type Names = BTreeSet<String>;

/// The atom spellings of one clingo witness value.
pub fn names(value: &Json) -> Names {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect()
}

/// The models a fixture records, each a list of atom spellings.
pub fn expected(value: &Json) -> Models {
    value.as_array().unwrap().iter().map(names).collect()
}

/// `source` admitted with the given options and limits.
pub fn limited(
    source: &str,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(source.into(), options, expansion, *limits)
}

/// Whether the fixture formula `formula` holds in `tested`: in the reduct by
/// `frozen` when one is given.
pub fn truth(formula: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
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
pub fn clingo(source: &str) -> Models {
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

/// Every answer set of `admitted` (at most six atoms), by the exhaustive reduct
/// check.
pub fn complete(admitted: &AdmittedFormula) -> Models {
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
pub fn manual_holds(theory: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    theory["roots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|root| truth(root, tested, frozen))
}

/// The spellings of `admitted`'s atoms whose bits `mask` sets.
pub fn selected(admitted: &AdmittedFormula, mask: usize) -> Names {
    admitted
        .atoms()
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, atom)| canonical(atom))
        .collect()
}
