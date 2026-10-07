//! Separate vocabulary and atom scopes with exact immutable-prefix extents.
//! Shared closed vocabularies justify term IDs, never another writer's atom IDs.

use std::sync::Arc;

use super::segments::{self, Atom, Counts, Predicate, RowSegment, Term, VocabularySegment};
use super::{
    AtomId, Closed, FrozenVocabulary, Owner, PredicateId, Snapshot, Store, TermId, TextId, locate,
};

#[derive(Clone, Debug)]
pub(crate) struct AtomScope(Arc<Owner>);
impl Store {
    /// A live writer always has an atom owner, unlike a vocabulary-only view.
    pub(crate) fn atom_scope(&self) -> AtomScope {
        AtomScope(Arc::clone(&self.atom_owner))
    }
}

impl AtomScope {
    /// Whether anything besides this handle refers to the owner. A count of
    /// one cannot rise again: a handle is made only by cloning another.
    pub(crate) fn held_elsewhere(&self) -> bool {
        Arc::strong_count(&self.0) > 1
    }
    /// The owner's address: unique among live owners, and stable while any
    /// handle, this one included, keeps the owner alive.
    pub(crate) fn key(&self) -> usize {
        Arc::as_ptr(&self.0).addr()
    }
}
#[derive(Clone, Debug)]
pub(crate) struct VocabularyScope(Arc<Owner>);
impl VocabularyScope {
    pub(super) fn fresh() -> Self {
        Self(Arc::new(Owner))
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Read<'a> {
    Snapshot(&'a Snapshot),
    Writer(&'a Store),
    Frozen(&'a FrozenVocabulary),
    /// A closed original writer: its own scopes and every canonical row.
    Closed(&'a Closed),
}
impl<'a> From<&'a Snapshot> for Read<'a> {
    fn from(snapshot: &'a Snapshot) -> Self {
        Self::Snapshot(snapshot)
    }
}
impl<'a> From<&'a Store> for Read<'a> {
    fn from(store: &'a Store) -> Self {
        Self::Writer(store)
    }
}
impl<'a> From<&'a Closed> for Read<'a> {
    fn from(closed: &'a Closed) -> Self {
        Self::Closed(closed)
    }
}
impl<'a> From<&'a FrozenVocabulary> for Read<'a> {
    fn from(base: &'a FrozenVocabulary) -> Self {
        Self::Frozen(base)
    }
}

impl<'a> Read<'a> {
    fn vocabulary_owner(self) -> &'a Arc<Owner> {
        match self {
            Self::Snapshot(snapshot) => &snapshot.data.vocabulary.owner,
            Self::Writer(store) => store.vocabulary.owner(),
            Self::Frozen(base) => &base.data.owner,
            Self::Closed(closed) => &closed.vocabulary.data.owner,
        }
    }
    fn atom_owner(self) -> Option<&'a Arc<Owner>> {
        match self {
            Self::Snapshot(snapshot) => Some(&snapshot.atom_owner),
            Self::Writer(store) => Some(&store.atom_owner),
            Self::Frozen(_) => None,
            Self::Closed(closed) => Some(closed.rows.source_owner()),
        }
    }
    /// The atom owner's address, as `AtomScope::key` gives it.
    pub(crate) fn atom_owner_key(self) -> Option<usize> {
        self.atom_owner().map(|owner| Arc::as_ptr(owner).addr())
    }
    pub(crate) fn atom_scope(self) -> Option<AtomScope> {
        self.atom_owner().map(|owner| AtomScope(Arc::clone(owner)))
    }
    pub(crate) fn vocabulary_scope(self) -> VocabularyScope {
        VocabularyScope(Arc::clone(self.vocabulary_owner()))
    }
    pub(crate) fn accepts_atom_scope(self, scope: &AtomScope) -> bool {
        self.atom_owner()
            .is_some_and(|owner| Arc::ptr_eq(owner, &scope.0))
    }
    pub(crate) fn accepts_vocabulary_scope(self, scope: &VocabularyScope) -> bool {
        Arc::ptr_eq(self.vocabulary_owner(), &scope.0)
    }
    pub(crate) fn same_vocabulary(self, other: Self) -> bool {
        Arc::ptr_eq(self.vocabulary_owner(), other.vocabulary_owner())
    }
    pub(crate) fn same_atoms(self, other: Self) -> bool {
        match (self.atom_owner(), other.atom_owner()) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
    pub(super) fn vocabulary_belongs_to(self, store: &Store) -> bool {
        Arc::ptr_eq(self.vocabulary_owner(), store.vocabulary.owner())
    }
    pub(super) fn atoms_belong_to(self, store: &Store) -> bool {
        self.atom_owner()
            .is_some_and(|owner| Arc::ptr_eq(owner, &store.atom_owner))
    }

    fn counts(self) -> Counts {
        match self {
            Self::Snapshot(snapshot) => Counts {
                atoms: snapshot.data.atoms,
                ..snapshot.data.vocabulary.counts
            },
            Self::Writer(store) => store.counts(),
            Self::Frozen(base) => base.data.counts,
            Self::Closed(closed) => Counts {
                atoms: closed.rows.payload.atoms,
                ..closed.vocabulary.data.counts
            },
        }
    }
    // Segments are contiguous from zero and the counts end with the last one
    // (a writer answers its unpublished tail directly), so an identity below
    // a count is exactly one that resolves. Views rely on this to check
    // membership without resolving.
    pub(crate) fn contains_atom(self, id: AtomId) -> bool {
        (id.0 as usize) < self.counts().atoms
    }
    pub(crate) fn contains_predicate(self, id: PredicateId) -> bool {
        (id.0 as usize) < self.counts().predicates
    }
    /// Every predicate identity of this prefix, in identity order.
    pub(crate) fn predicate_ids(self) -> impl ExactSizeIterator<Item = PredicateId> {
        (0..self.counts().predicates).map(|position| {
            PredicateId(u32::try_from(position).expect("admitted predicate identities fit u32"))
        })
    }
    pub(crate) fn contains_term(self, id: TermId) -> bool {
        (id.0 as usize) < self.counts().terms
    }
    pub(crate) fn contains_text(self, id: TextId) -> bool {
        (id.0 as usize) < self.counts().texts
    }

    fn vocabulary_segment(
        self,
        id: usize,
        start: impl Fn(Counts) -> usize,
    ) -> Option<&'a VocabularySegment> {
        match self {
            Self::Snapshot(snapshot) => locate(&snapshot.data.vocabulary.segments, id, |segment| {
                start(segment.start)
            }),
            Self::Writer(store) => store.vocabulary_segment(id, start),
            Self::Frozen(base) => locate(&base.data.segments, id, |segment| start(segment.start)),
            Self::Closed(closed) => locate(&closed.vocabulary.data.segments, id, |segment| {
                start(segment.start)
            }),
        }
    }
    fn row_segment(self, id: usize) -> Option<&'a RowSegment> {
        match self {
            Self::Snapshot(snapshot) => snapshot.rows().segment(id),
            Self::Writer(store) => store.row_segment(id),
            Self::Frozen(_) => None,
            Self::Closed(closed) => {
                locate(&closed.rows.payload.segments, id, |segment| segment.start)
            }
        }
    }
    pub(crate) fn text(self, id: TextId) -> &'a str {
        let segment = self
            .vocabulary_segment(id.0 as usize, |counts| counts.texts)
            .expect("admitted text ID");
        segments::text(segment, id)
    }
    pub(super) fn text_measures(self, id: TextId) -> (usize, usize) {
        let segment = self
            .vocabulary_segment(id.0 as usize, |counts| counts.texts)
            .expect("admitted text ID");
        segments::text_measures(segment, id)
    }
    pub(crate) fn term(self, id: TermId) -> Option<Term<'a>> {
        if !self.contains_term(id) {
            return None;
        }
        let segment = self.vocabulary_segment(id.0 as usize, |counts| counts.terms)?;
        Some(Term {
            read: self,
            segment,
            local: id.0 as usize - segment.start.terms,
        })
    }
    pub(crate) fn term_constructor(self, id: TermId) -> Option<crate::catalog::ConstructorData> {
        use super::nodes::Kind;
        use crate::catalog::ConstructorData;
        let term = self.term(id)?;
        match term.segment.terms.kinds[term.local] {
            Kind::Symbol => Some(ConstructorData::Function {
                name: TextId(term.segment.terms.payloads[term.local]),
                sign: crate::Sign::Positive,
                arity: 0,
            }),
            Kind::Function | Kind::Tuple => {
                let compound = term.segment.terms.compound(term.local)?;
                Some(compound.name.map_or(
                    ConstructorData::Tuple {
                        arity: compound.children.len(),
                    },
                    |name| ConstructorData::Function {
                        name,
                        sign: compound.sign,
                        arity: compound.children.len(),
                    },
                ))
            }
            _ => None,
        }
    }
    pub(crate) fn predicate_constructor(
        self,
        id: PredicateId,
    ) -> Option<crate::catalog::ConstructorData> {
        if !self.contains_predicate(id) {
            return None;
        }
        let segment = self.vocabulary_segment(id.0 as usize, |counts| counts.predicates)?;
        let signature = segment.predicates[id.0 as usize - segment.start.predicates];
        Some(crate::catalog::ConstructorData::Function {
            name: signature.name,
            sign: signature.sign,
            arity: signature.arity,
        })
    }
    pub(crate) fn predicate(self, id: PredicateId) -> Option<Predicate<'a>> {
        if !self.contains_predicate(id) {
            return None;
        }
        let segment = self.vocabulary_segment(id.0 as usize, |counts| counts.predicates)?;
        let signature = segment.predicates[id.0 as usize - segment.start.predicates];
        Some(Predicate {
            read: self,
            name: signature.name,
            arity: signature.arity,
            sign: signature.sign,
        })
    }
    pub(crate) fn atom(self, id: AtomId) -> Option<Atom<'a>> {
        if !self.contains_atom(id) {
            return None;
        }
        let segment = self.row_segment(id.0 as usize)?;
        let locator = segment.atoms[id.0 as usize - segment.start];
        Some(Atom {
            columns: &segment.columns[locator.block],
            locator,
        })
    }
}
