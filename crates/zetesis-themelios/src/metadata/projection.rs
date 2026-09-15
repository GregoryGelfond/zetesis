//! Source projection declarations and their separately completed fixed domain.

use zetesis_core::{Atom, Predicate};

/// Source declarations for explicit projected enumeration. This is not a
/// completed projection domain and does not alter ordinary full-family APIs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectSelection {
    pub(crate) explicit: bool,
    pub(crate) signatures: Vec<Predicate>,
    pub(crate) conditional_atoms: bool,
}
impl ProjectSelection {
    /// Whether any `#project` directive was authored, including an empty domain.
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.explicit
    }
    /// Sorted distinct signed predicate selectors.
    #[must_use]
    pub fn signatures(&self) -> &[Predicate] {
        &self.signatures
    }
    /// Whether atom/body declarations require complete source support grounding.
    #[must_use]
    pub fn has_conditional_atoms(&self) -> bool {
        self.conditional_atoms
    }
    pub(crate) fn signature(&mut self, predicate: Predicate) {
        self.explicit = true;
        self.signatures.push(predicate);
    }
    pub(crate) fn atom(&mut self) {
        self.explicit = true;
        self.conditional_atoms = true;
    }
    pub(crate) fn finish(mut self) -> Self {
        self.signatures.sort();
        self.signatures.dedup();
        self
    }
}

/// Fixed original-atom domain completed against one prepared source owner.
/// Conditional declarations are grounded once; conditions are not reevaluated
/// against individual answers. A projected result remains a full answer set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparedProjection {
    pub(crate) explicit: bool,
    pub(crate) atoms: Vec<Atom>,
}
impl PreparedProjection {
    /// Whether source directives explicitly requested this domain.
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.explicit
    }
    /// Sorted unique full typed atoms defining projection identity. Positions
    /// belong to this fixed domain, not to a solver's atom catalog.
    #[must_use]
    pub fn atoms(&self) -> &[Atom] {
        &self.atoms
    }
    /// Whether this original atom belongs to the fixed domain.
    #[must_use]
    pub fn contains(&self, atom: &Atom) -> bool {
        self.atoms.binary_search(atom).is_ok()
    }
}
