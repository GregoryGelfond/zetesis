//! Models as sets of atoms, as the formula propositions compare them.

use std::collections::BTreeSet;

use zetesis_core::Atom;

/// Models as sets of atoms.
pub type Models = BTreeSet<BTreeSet<Atom>>;
