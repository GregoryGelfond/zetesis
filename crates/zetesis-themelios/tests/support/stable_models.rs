//! The complete family of stable models of an admitted formula, as atom sets.

use std::collections::BTreeSet;

use zetesis_core::Atom;
use zetesis_cpu::Cancellation;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::AdmittedFormula;

/// Every stable model of the formula, each as its atoms: the search must
/// exhaust the family and return each model once.
pub fn stable(input: &AdmittedFormula) -> BTreeSet<BTreeSet<Atom>> {
    let mut search =
        StableModels::new(input.theory(), Limits::default(), Cancellation::default()).unwrap();
    let mut result = BTreeSet::new();
    for model in search.by_ref() {
        let atoms = model
            .unwrap()
            .atoms()
            .map(|index| input.atoms()[index].clone())
            .collect();
        assert!(result.insert(atoms), "a stable model returned twice");
    }
    assert!(search.exhausted());
    result
}
