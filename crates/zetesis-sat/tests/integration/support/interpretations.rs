//! Interpretations chosen by bit mask, a model's atoms as a list, and a
//! theory's stable models so listed, found by enumeration or by a search.

use std::collections::BTreeSet;

use zetesis_ferraris::{Interpretation, Theory};
use zetesis_sat::{Cancellation, StableModels};

pub use zetesis_theory_support::theories::interpretation;

/// The atoms `model` holds, in order.
pub fn key(model: &Interpretation) -> Vec<usize> {
    model.atoms().collect()
}

/// Every stable model of `theory`: each interpretation the reduct check
/// accepts, as its atoms.
pub fn stable_models(theory: &Theory) -> BTreeSet<Vec<usize>> {
    (0..1_usize << theory.atom_count())
        .filter_map(|mask| {
            let model = Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
            )
            .unwrap();
            zetesis_ferraris::check(
                theory,
                &model,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
            .then(|| model.atoms().collect())
        })
        .collect()
}

/// The models `search` returns, each as its atoms, in its order.
pub fn models(search: &mut StableModels) -> Vec<Vec<usize>> {
    search
        .by_ref()
        .map(|model| model.unwrap().atoms().collect())
        .collect()
}
