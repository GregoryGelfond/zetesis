//! Source projection declarations and their separately completed fixed domain.

use super::selection::{SignatureSet, Signatures};
use super::{MetadataVocabulary, Predicate};
use std::sync::Arc;
use zetesis_core::AtomCatalog;
use zetesis_core::catalog::{AtomRef, Atoms};

/// Source declarations for explicit projected enumeration. This is not a
/// completed projection domain and does not alter ordinary full-family APIs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectSelection {
    pub(crate) explicit: bool,
    signatures: SignatureSet,
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
    pub fn signatures(&self) -> Signatures<'_> {
        self.signatures.view()
    }
    /// Whether atom/body declarations require complete source support grounding.
    #[must_use]
    pub fn has_conditional_atoms(&self) -> bool {
        self.conditional_atoms
    }
}

#[derive(Default)]
pub(super) struct Builder {
    explicit: bool,
    signatures: Vec<Predicate>,
    conditional_atoms: bool,
}
impl Builder {
    pub(crate) fn signature(&mut self, predicate: Predicate) {
        self.explicit = true;
        self.signatures.push(predicate);
    }
    pub(crate) fn atom(&mut self) {
        self.explicit = true;
        self.conditional_atoms = true;
    }
    pub(super) fn finish(self, vocabulary: Option<Arc<MetadataVocabulary>>) -> ProjectSelection {
        ProjectSelection {
            explicit: self.explicit,
            signatures: SignatureSet::publish(vocabulary, self.signatures),
            conditional_atoms: self.conditional_atoms,
        }
    }
}

/// Fixed original-atom domain completed against one prepared source owner.
/// Conditional declarations are grounded once; conditions are not reevaluated
/// against individual answers. A projected result remains a full answer set.
#[derive(Clone, Debug, Default)]
pub struct PreparedProjection {
    pub(crate) explicit: bool,
    pub(crate) atoms: AtomCatalog,
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
    pub fn atoms(&self) -> Atoms<'_> {
        self.atoms.atoms()
    }
    /// Whether this original atom belongs to the fixed domain.
    #[must_use]
    pub fn contains<'a>(&self, atom: impl Into<AtomRef<'a>>) -> bool {
        self.atoms.atoms().binary_search(atom.into()).is_ok()
    }
}

impl PartialEq for PreparedProjection {
    fn eq(&self, other: &Self) -> bool {
        self.explicit == other.explicit && self.atoms() == other.atoms()
    }
}
impl Eq for PreparedProjection {}
