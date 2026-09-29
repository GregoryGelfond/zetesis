//! Signed atoms, models of nullary atoms, and families of models, as the
//! session propositions build and compare them.

use std::collections::BTreeSet;

use zetesis_core::{Atom, Model, Predicate, Sign, Value};

/// The atom `name(values…)` with `sign`; its arity is the number of values.
pub fn atom(name: &str, sign: Sign, values: Vec<Value>) -> Atom {
    Atom::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values,
    )
    .unwrap()
}

/// The model holding the nullary atoms `names`.
pub fn model(names: &[&str]) -> Model {
    Model::new(
        names
            .iter()
            .map(|name| Atom::new(Predicate::new(*name, 0).unwrap(), vec![]).unwrap()),
    )
    .unwrap()
}

/// A family of models, compared as a set.
pub type Family = BTreeSet<Model>;
