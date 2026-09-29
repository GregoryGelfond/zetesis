//! A one-atom catalog, and a step budget that fails once spent, for the
//! canonical-form propositions.

use zetesis_core::{Atom, AtomCatalog, Predicate, Value};

/// Spends one step of `remaining`, failing once none is left.
pub fn before(remaining: &mut usize) -> Result<(), ()> {
    if *remaining == 0 {
        return Err(());
    }
    *remaining -= 1;
    Ok(())
}

/// The catalog of the one atom `p(value)`.
pub fn catalog(value: Value) -> AtomCatalog {
    AtomCatalog::new(vec![
        Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap(),
    ])
    .unwrap()
}
