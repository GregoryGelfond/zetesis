//! Constructor descriptions refer to canonical text, without invented values.

use super::{
    CatalogRead, ReadError,
    storage::{Read, TextId, VocabularyScope},
};
use crate::{Sign, ValueNodeRef};

#[derive(Clone, Copy, Debug)]
pub(crate) enum ConstructorData {
    Function {
        name: TextId,
        sign: Sign,
        arity: usize,
    },
    Tuple {
        arity: usize,
    },
}
impl ConstructorData {
    pub(crate) fn text(self) -> Option<TextId> {
        match self {
            Self::Function { name, .. } => Some(name),
            Self::Tuple { .. } => None,
        }
    }
    pub(crate) fn descriptor(self, read: Read<'_>) -> ValueNodeRef<'_> {
        match self {
            Self::Function { name, sign, arity } => ValueNodeRef::Function {
                name: read.text(name),
                sign,
                arity,
            },
            Self::Tuple { arity } => ValueNodeRef::Tuple { arity },
        }
    }
}

/// A constructor shape declared in a canonical vocabulary, without a ground
/// term, predicate, atom or logical membership. The witness owns no text payload.
/// Resolution requires a compatible reader covering its name's exact prefix.
/// Retained component owners store this description under one shared witness.
#[derive(Clone, Debug)]
pub struct DeclaredConstructor {
    pub(super) scope: VocabularyScope,
    pub(super) data: ConstructorData,
}
impl DeclaredConstructor {
    pub(crate) fn new(read: Read<'_>, data: ConstructorData) -> Self {
        Self {
            scope: read.vocabulary_scope(),
            data,
        }
    }

    /// Replace a function's sign while retaining the same name identity, arity
    /// and vocabulary witness. This consumes the handle without reading or
    /// copying text. Tuples have no sign and return `None`.
    #[must_use]
    pub fn with_sign(mut self, sign: Sign) -> Option<Self> {
        let ConstructorData::Function { name, arity, .. } = self.data else {
            return None;
        };
        self.data = ConstructorData::Function { name, sign, arity };
        Some(self)
    }
}
impl<'a> CatalogRead<'a> {
    /// Borrow a declared function or tuple shape. Nullary positive functions
    /// remain constructor descriptions here; constructing a term applies the
    /// ordinary normalization to a symbol.
    ///
    /// # Errors
    /// Refuses a foreign vocabulary or a prefix preceding the constructor name.
    pub fn constructor(
        self,
        constructor: &DeclaredConstructor,
    ) -> Result<ValueNodeRef<'a>, ReadError> {
        self.selected_constructor(constructor)
            .map(|data| data.descriptor(self.0))
    }

    pub(crate) fn selected_constructor(
        self,
        constructor: &DeclaredConstructor,
    ) -> Result<ConstructorData, ReadError> {
        if !self.0.accepts_vocabulary_scope(&constructor.scope) {
            return Err(ReadError::ForeignCatalog);
        }
        if constructor
            .data
            .text()
            .is_some_and(|id| !self.0.contains_text(id))
        {
            return Err(ReadError::OutsidePrefix);
        }
        Ok(constructor.data)
    }
}

#[cfg(test)]
mod tests;

impl CatalogRead<'_> {
    /// Borrow the constructor shape of an admitted predicate without copying its name.
    /// # Errors
    /// Refuses ingress, foreign predicates, or inaccessible newer predicates.
    pub fn constructor_of_predicate(
        self,
        predicate: super::PredicateRef<'_>,
    ) -> Result<DeclaredConstructor, ReadError> {
        let id = self.selected_predicate(predicate)?;
        let data = self
            .0
            .predicate_constructor(id)
            .ok_or(ReadError::OutsidePrefix)?;
        Ok(DeclaredConstructor::new(self.0, data))
    }
}
