//! Interpretations chosen by bit mask, and a model's atoms as a list.

use zetesis_ferraris::{Interpretation, Theory};

/// The interpretation of `theory` holding the atoms whose bits `mask` sets.
pub fn interpretation(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

/// The atoms `model` holds, in order.
pub fn key(model: &Interpretation) -> Vec<usize> {
    model.atoms().collect()
}
