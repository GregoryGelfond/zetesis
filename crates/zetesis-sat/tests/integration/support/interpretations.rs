//! Interpretations chosen by bit mask, and a model's atoms as a list.

use zetesis_ferraris::Interpretation;

pub use zetesis_theory_support::theories::interpretation;

/// The atoms `model` holds, in order.
pub fn key(model: &Interpretation) -> Vec<usize> {
    model.atoms().collect()
}
