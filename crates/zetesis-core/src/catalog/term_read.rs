//! A term-only resolver shared by ordinary vocabularies and derived arenas.

use super::{CatalogRead, DeclaredConstructor, ReadError, TermKey, TermRef, storage};

/// A borrowed, exact-prefix term resolver. It owns no payload and cannot outlive
/// the immutable borrow of its catalog or derived arena. Atom and predicate
/// membership remains a separate [`CatalogRead`] capability.
#[derive(Clone, Copy, Debug)]
pub struct TermRead<'a>(Source<'a>);

#[derive(Clone, Copy, Debug)]
enum Source<'a> {
    Canonical(storage::Read<'a>),
    Derived(&'a storage::DerivedTerms<'a>),
}
impl<'a> From<CatalogRead<'a>> for TermRead<'a> {
    fn from(read: CatalogRead<'a>) -> Self {
        Self(Source::Canonical(read.0))
    }
}
impl<'a> TermRead<'a> {
    pub(super) fn derived(arena: &'a storage::DerivedTerms<'a>) -> Self {
        Self(Source::Derived(arena))
    }
    pub(super) fn scope(self) -> storage::VocabularyScope {
        match self.0 {
            Source::Canonical(read) => read.vocabulary_scope(),
            Source::Derived(arena) => arena.scope().clone(),
        }
    }
    pub(super) fn accepts(self, scope: &storage::VocabularyScope) -> bool {
        match self.0 {
            Source::Canonical(read) => read.accepts_vocabulary_scope(scope),
            Source::Derived(arena) => arena.scope().same(scope),
        }
    }
    pub(super) fn same(self, other: Self) -> bool {
        match (self.0, other.0) {
            (Source::Canonical(left), Source::Canonical(right)) => left.same_vocabulary(right),
            (Source::Derived(left), Source::Derived(right)) => left.scope().same(right.scope()),
            _ => false,
        }
    }
    pub(super) fn contains(self, id: storage::TermId) -> bool {
        match self.0 {
            Source::Canonical(read) => read.contains_term(id),
            Source::Derived(arena) => arena.contains(id),
        }
    }
    pub(crate) fn resolve(self, id: storage::TermId) -> Option<TermRef<'a>> {
        match self.0 {
            Source::Canonical(read) => TermRef::new(read, id),
            Source::Derived(arena) => TermRef::derived(arena, id),
        }
    }
    /// Borrow the shape of a function, symbol, or tuple without copying text.
    /// Scalar numbers, strings and extrema have no constructor shape.
    /// # Errors
    /// Refuses foreign keys and keys outside this exact read prefix.
    pub fn constructor_of(self, key: &TermKey) -> Result<Option<DeclaredConstructor>, ReadError> {
        if !self.accepts(&key.scope) {
            return Err(ReadError::ForeignCatalog);
        }
        if !self.contains(key.id) {
            return Err(ReadError::OutsidePrefix);
        }
        Ok(match self.0 {
            Source::Canonical(read) => read
                .term_constructor(key.id)
                .map(|data| DeclaredConstructor::new(read, data)),
            Source::Derived(arena) => arena.constructor(key.id),
        })
    }
}
