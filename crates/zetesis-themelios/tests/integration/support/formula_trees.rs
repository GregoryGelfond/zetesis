//! Formulas as explicit trees, independent of the production DAG, and the worlds
//! they are evaluated in.

use std::collections::BTreeSet;

use zetesis_reference_support::canonical;

/// A separate tree definition, with no production DAG construction or reduct API.
#[derive(Clone)]
pub enum Formula {
    False,
    Atom(String),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Implies(Box<Self>, Box<Self>),
}
impl Formula {
    pub fn atom(name: impl Into<String>) -> Self {
        Self::Atom(name.into())
    }
    pub fn implies(left: Self, right: Self) -> Self {
        Self::Implies(Box::new(left), Box::new(right))
    }
    pub fn and(left: Self, right: Self) -> Self {
        Self::And(Box::new(left), Box::new(right))
    }
    pub fn truth() -> Self {
        Self::implies(Self::False, Self::False)
    }
    pub fn sign(self, count: usize) -> Self {
        (0..count).fold(self, |value, _| Self::implies(value, Self::False))
    }
    pub fn original(&self, world: &BTreeSet<String>) -> bool {
        match self {
            Self::False => false,
            Self::Atom(atom) => world.contains(atom),
            Self::And(left, right) => left.original(world) && right.original(world),
            Self::Or(left, right) => left.original(world) || right.original(world),
            Self::Implies(left, right) => !left.original(world) || right.original(world),
        }
    }
    pub fn frozen(&self, outer: &BTreeSet<String>, inner: &BTreeSet<String>) -> bool {
        if !self.original(outer) {
            return false;
        }
        match self {
            Self::False => false,
            Self::Atom(atom) => inner.contains(atom),
            Self::And(left, right) => left.frozen(outer, inner) && right.frozen(outer, inner),
            Self::Or(left, right) => left.frozen(outer, inner) || right.frozen(outer, inner),
            Self::Implies(left, right) => !left.frozen(outer, inner) || right.frozen(outer, inner),
        }
    }
}

/// The spellings of the atoms of `atoms` whose bits `mask` sets.
pub fn world(atoms: zetesis_core::catalog::Atoms<'_>, mask: usize) -> BTreeSet<String> {
    atoms
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, atom)| canonical(atom))
        .collect()
}
