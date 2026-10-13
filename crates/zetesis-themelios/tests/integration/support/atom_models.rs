//! Models as sets of atoms, as the formula propositions compare them.

use std::collections::BTreeSet;

use zetesis_core::{Atom, Program, ValueLimits};
use zetesis_reference_support::normal_answer_sets;

/// Models as sets of atoms.
pub(crate) type Models = BTreeSet<BTreeSet<Atom>>;

/// Every answer set of the normal program `program`, as sets of atoms; no two
/// candidates yield the same one.
pub(crate) fn answer_sets(program: &Program) -> Models {
    let mut models = Models::new();
    for model in normal_answer_sets(program) {
        let atoms = model
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
            .collect();
        assert!(models.insert(atoms), "one candidate per answer set");
    }
    models
}
