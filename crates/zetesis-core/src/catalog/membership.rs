//! Predicate-local membership over an explicit canonical read scope.
//!
//! This metadata owns no logical payload or snapshot directory. Binding checks
//! both the atom authority and the accessible prefix before exposing local rows.

use std::fmt;

use super::{AtomRef, Atoms, PredicateRef, storage};

/// A borrowed canonical resolver. It carries an exact accessible prefix, not
/// merely an identity token. Retaining it borrows its writer or sealed snapshot;
/// it never owns a second term or tuple representation.
#[derive(Clone, Copy, Debug)]
pub struct CatalogRead<'a>(pub(super) storage::Read<'a>);

/// A declared canonical predicate, independent of any extensional membership.
/// The identity witness retains no payload. Resolve it using a compatible read
/// prefix; an empty relation needs no fabricated representative atom.
#[derive(Clone, Debug)]
pub struct DeclaredPredicate {
    scope: storage::VocabularyScope,
    id: storage::PredicateId,
}

/// A canonical metadata view could not bind to the supplied reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// An identity belongs to another canonical authority.
    ForeignCatalog,
    /// The reader predates an identity required by the metadata.
    OutsidePrefix,
    /// Owned ingress has not been interned into this authority.
    Uninterned,
    /// The supplied atom has a different signed predicate.
    Predicate,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ForeignCatalog => "canonical identity belongs to another catalog",
            Self::OutsidePrefix => "canonical read prefix does not cover this identity",
            Self::Uninterned => "canonical metadata requires an interned identity",
            Self::Predicate => "atom does not have the relation's predicate",
        })
    }
}
impl std::error::Error for ReadError {}

impl<'a> CatalogRead<'a> {
    pub(crate) fn storage(self) -> storage::Read<'a> {
        self.0
    }

    pub(crate) fn selected_term(
        self,
        term: super::TermRef<'_>,
    ) -> Result<storage::TermId, ReadError> {
        self.term_key(term).map(|key| key.id)
    }

    pub(crate) fn selected_predicate(
        self,
        predicate: PredicateRef<'_>,
    ) -> Result<storage::PredicateId, ReadError> {
        self.declare_existing(predicate).map(|key| key.id)
    }

    /// Retain a declaration for a predicate already present in this read prefix.
    /// This validates canonical scope without importing or comparing payload.
    ///
    /// # Errors
    /// Refuses owned ingress, a foreign vocabulary, or an inaccessible prefix.
    pub fn declare_existing(
        self,
        predicate: PredicateRef<'_>,
    ) -> Result<DeclaredPredicate, ReadError> {
        let (read, id) = predicate.canonical().ok_or(ReadError::Uninterned)?;
        if !self.0.accepts_vocabulary_scope(&read.vocabulary_scope()) {
            return Err(ReadError::ForeignCatalog);
        }
        if !self.0.contains_predicate(id) {
            return Err(ReadError::OutsidePrefix);
        }
        Ok(DeclaredPredicate::new(self.0, id))
    }

    /// Resolve a declared predicate without borrowing its declaration writer.
    ///
    /// # Errors
    /// Refuses a foreign vocabulary or an older prefix missing the predicate.
    pub fn predicate(self, key: &DeclaredPredicate) -> Result<PredicateRef<'a>, ReadError> {
        if !self.0.accepts_vocabulary_scope(&key.scope) {
            return Err(ReadError::ForeignCatalog);
        }
        PredicateRef::new(self.0, key.id).ok_or(ReadError::OutsidePrefix)
    }
}

impl DeclaredPredicate {
    pub(super) fn new(read: storage::Read<'_>, id: storage::PredicateId) -> Self {
        Self {
            scope: read.vocabulary_scope(),
            id,
        }
    }
}

/// Insertion rows and one maintained upper bound for constant-time prefix
/// validation. Equality IDs and source occurrence positions remain separate.
pub(crate) struct Membership {
    scope: storage::AtomScope,
    predicate: DeclaredPredicate,
    pub(crate) ids: Vec<storage::AtomId>,
    last: Option<storage::AtomId>,
}

/// A complete atom checked for this membership's authority and predicate.
/// Only this module constructs it; relation planning cannot adopt a raw ID.
#[derive(Clone, Copy)]
pub(crate) struct Member<'a> {
    read: CatalogRead<'a>,
    id: storage::AtomId,
}

impl<'a> Member<'a> {
    pub(crate) const fn read(&self) -> CatalogRead<'a> {
        self.read
    }
    pub(crate) fn atom(&self) -> AtomRef<'a> {
        AtomRef::new(self.read.0, self.id).expect("checked member belongs to its read prefix")
    }
}

impl Membership {
    pub(crate) fn new(
        read: CatalogRead<'_>,
        predicate: DeclaredPredicate,
    ) -> Result<Self, ReadError> {
        read.predicate(&predicate)?;
        let scope = read.0.atom_scope().ok_or(ReadError::ForeignCatalog)?;
        Ok(Self {
            scope,
            predicate,
            ids: Vec::new(),
            last: None,
        })
    }

    pub(crate) fn predicate<'a>(
        &self,
        read: CatalogRead<'a>,
    ) -> Result<PredicateRef<'a>, ReadError> {
        self.check(read)?;
        read.predicate(&self.predicate)
    }

    fn check(&self, read: CatalogRead<'_>) -> Result<(), ReadError> {
        if !read.0.accepts_atom_scope(&self.scope)
            || !read.0.accepts_vocabulary_scope(&self.predicate.scope)
        {
            return Err(ReadError::ForeignCatalog);
        }
        if !read.0.contains_predicate(self.predicate.id)
            || self.last.is_some_and(|id| !read.0.contains_atom(id))
        {
            return Err(ReadError::OutsidePrefix);
        }
        Ok(())
    }

    pub(crate) fn bind<'a>(&'a self, read: CatalogRead<'a>) -> Result<Atoms<'a>, ReadError> {
        self.check(read)?;
        Ok(Atoms::new(read.0, &self.ids))
    }

    pub(crate) fn member<'a>(&self, atom: AtomRef<'a>) -> Result<Member<'a>, ReadError> {
        let (read, id) = atom.canonical().ok_or(ReadError::Uninterned)?;
        let read = CatalogRead(read);
        self.check(read)?;
        let (_, predicate) = atom.predicate().canonical().ok_or(ReadError::Uninterned)?;
        if predicate != self.predicate.id {
            return Err(ReadError::Predicate);
        }
        Ok(Member { read, id })
    }

    /// Caller reserves the ID cell and admits publication before this write.
    pub(crate) fn publish(&mut self, member: Member<'_>) {
        self.last = Some(self.last.map_or(member.id, |old| old.max(member.id)));
        self.ids.push(member.id);
    }

    pub(crate) fn clear(&mut self) {
        self.ids.clear();
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Predicate;
    use std::convert::Infallible;

    fn predicate(store: &mut storage::Store, name: &str) -> storage::PredicateId {
        store
            .import_predicate_with((&Predicate::new(name, 0).unwrap()).into(), || {
                Ok::<(), Infallible>(())
            })
            .unwrap()
    }

    #[test]
    fn an_existing_declaration_round_trips_without_payload_import() {
        let mut store = storage::Store::new(usize::MAX);
        let id = predicate(&mut store, "p");
        let read = CatalogRead(storage::Read::from(&store));
        let value = PredicateRef::new(read.0, id).unwrap();
        let declaration = read.declare_existing(value).unwrap();
        assert_eq!(read.predicate(&declaration).unwrap(), value);
    }

    #[test]
    fn an_existing_declaration_rejects_foreign_vocabulary() {
        let mut left = storage::Store::new(usize::MAX);
        let mut right = storage::Store::new(usize::MAX);
        predicate(&mut left, "p");
        let id = predicate(&mut right, "p");
        let value = PredicateRef::new(storage::Read::from(&right), id).unwrap();
        assert!(matches!(
            CatalogRead(storage::Read::from(&left)).declare_existing(value),
            Err(ReadError::ForeignCatalog)
        ));
    }

    #[test]
    fn an_existing_declaration_rejects_a_newer_prefix() {
        let mut store = storage::Store::new(usize::MAX);
        predicate(&mut store, "p");
        let prefix = store.snapshot(0).unwrap();
        let id = predicate(&mut store, "q");
        let value = PredicateRef::new(storage::Read::from(&store), id).unwrap();
        assert!(matches!(
            CatalogRead(storage::Read::from(&prefix)).declare_existing(value),
            Err(ReadError::OutsidePrefix)
        ));
    }

    #[test]
    fn an_existing_declaration_rejects_owned_ingress() {
        let store = storage::Store::new(usize::MAX);
        let value = Predicate::new("p", 0).unwrap();
        assert!(matches!(
            CatalogRead(storage::Read::from(&store)).declare_existing((&value).into()),
            Err(ReadError::Uninterned)
        ));
    }
}
