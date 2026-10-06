//! Checked atom queries for exact identity and ordered discovery.

use crate::catalog::storage::{self, AtomId, PreparedAtom, Store, TermId};
use crate::catalog::{AtomRef, PredicateRef, TermRef};
use crate::{AtomKey, PatternTerms, TemplateTerm, ordered_index};

use super::Failure;
use super::index::{Link, Node};

/// Unauthenticated coordinates require the general semantic lookup. A local
/// miss, by contrast, establishes absence from the exact canonical row index.
pub(super) enum Identity {
    Foreign,
    Local(PreparedAtom),
}

/// Concrete immutable inputs, rather than a stateful caller iterator.
#[derive(Clone, Copy)]
pub(super) enum ProjectionSource<'a> {
    Slots(&'a [usize]),
    Pattern(PatternTerms<'a>),
}
impl<'a> ProjectionSource<'a> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Slots(slots) => slots.len(),
            Self::Pattern(terms) => terms.len(),
        }
    }

    pub(super) fn at(self, column: usize) -> TemplateTerm<'a> {
        match self {
            Self::Slots(slots) => TemplateTerm::Variable(slots[column]),
            Self::Pattern(terms) => terms.at(column).expect("projected argument arity"),
        }
    }
}

/// Borrowed coordinates shared by ordered lookup and canonical row admission.
/// The producer authenticates the scope, prefix and selected slots before this
/// becomes a Query. Insertion additionally checks its logical term limits.
/// No Store read is retained from the mutable writer.
pub(super) struct Projected<'a> {
    pub(super) predicate: storage::PredicateId,
    pub(super) values: &'a [Option<TermId>],
    pub(super) arguments: ProjectionSource<'a>,
}

impl Projected<'_> {
    pub(super) const HEADER_BYTES: u128 = size_of::<Self>() as u128;

    /// The source is immutable and fully validated before this accessor runs.
    /// Admitted constants may resolve immutable segment metadata; this is not
    /// necessarily a raw constant-time ID load. No user callback, allocation or
    /// recoverable validation occurs during prepared row publication.
    fn argument(&self, column: usize) -> TermId {
        match self.arguments.at(column) {
            TemplateTerm::Variable(slot) => self.values[slot].expect("validated bound slot"),
            TemplateTerm::Constant(term) => {
                term.canonical().expect("validated canonical constant").1
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Query<'a> {
    Atom(AtomRef<'a>),
    SignedAtom(AtomRef<'a>, crate::Sign),
    Key(AtomKey<'a>),
    Projected(&'a Projected<'a>),
}

impl<'a> Query<'a> {
    /// Reuse canonical identity only under this writer's exact scope and prefix.
    /// No temporary vector or retained mutation is required for borrowed keys.
    pub(super) fn identity_with<E>(
        self,
        store: &Store,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Identity, Failure<E>> {
        before().map_err(Failure::Stopped)?;
        if let Self::Projected(projected) = self {
            return store
                .prepare_atom_by_with(
                    projected.predicate,
                    projected.arguments.len(),
                    |column| projected.argument(column),
                    before,
                )
                .map(Identity::Local)
                .map_err(|error| match error {
                    storage::Failure::Storage(error) => Failure::Catalog(error),
                    storage::Failure::Stopped(error) => Failure::Stopped(error),
                });
        }
        let read = storage::Read::from(store);
        if let Self::Atom(atom) = self
            && let Some((source, id)) = atom.canonical()
        {
            before().map_err(Failure::Stopped)?;
            if read.same_atoms(source) && read.contains_atom(id) {
                return Ok(Identity::Local(PreparedAtom::existing(id)));
            }
        }
        before().map_err(Failure::Stopped)?;
        let predicate = self.predicate(store);
        let Some((source, id)) = predicate.canonical() else {
            return Ok(Identity::Foreign);
        };
        before().map_err(Failure::Stopped)?;
        if !read.same_vocabulary(source) || !read.contains_predicate(id) {
            return Ok(Identity::Foreign);
        }
        before().map_err(Failure::Stopped)?;
        let arity = predicate.arity();
        for column in 0..arity {
            before().map_err(Failure::Stopped)?;
            let Some((source, term)) = self.argument(column, store).canonical() else {
                return Ok(Identity::Foreign);
            };
            before().map_err(Failure::Stopped)?;
            if !read.same_vocabulary(source) || !read.contains_term(term) {
                return Ok(Identity::Foreign);
            }
        }
        store
            .prepare_atom_by_with(
                id,
                arity,
                |column| {
                    self.argument(column, store)
                        .canonical()
                        .expect("authenticated argument retains its borrowed prefix")
                        .1
                },
                before,
            )
            .map(Identity::Local)
            .map_err(|error| match error {
                storage::Failure::Storage(error) => Failure::Catalog(error),
                storage::Failure::Stopped(error) => Failure::Stopped(error),
            })
    }

    pub(super) fn extra_bytes(self) -> u128 {
        match self {
            Self::Projected(_) => Projected::HEADER_BYTES,
            Self::Atom(_) | Self::SignedAtom(_, _) | Self::Key(_) => 0,
        }
    }

    pub(super) fn predicate<'read>(self, store: &'read Store) -> PredicateRef<'read>
    where
        'a: 'read,
    {
        match self {
            Self::Atom(atom) => atom.predicate(),
            Self::SignedAtom(atom, sign) => atom.predicate().with_sign(sign),
            Self::Key(key) => key.predicate(),
            Self::Projected(projected) => {
                PredicateRef::new(storage::Read::from(store), projected.predicate)
                    .expect("validated assigned predicate")
            }
        }
    }

    fn argument<'read>(self, column: usize, store: &'read Store) -> TermRef<'read>
    where
        'a: 'read,
    {
        match self {
            Self::Atom(atom) | Self::SignedAtom(atom, _) => {
                atom.values().at(column).expect("admitted argument")
            }
            Self::Key(key) => key.argument(column),
            Self::Projected(projected) => {
                TermRef::new(store, projected.argument(column)).expect("validated projected prefix")
            }
        }
    }

    /// Search within one signed predicate. Discovery positions, not canonical
    /// IDs, index the AVL nodes. Every argument resolves in its own read owner.
    pub(super) fn search<'rows, E>(
        self,
        store: &Store,
        resolve: impl Fn(usize) -> Option<AtomRef<'rows>>,
        nodes: &[Node],
        cursor: Link,
        before: &mut impl FnMut() -> Result<(), E>,
        descend: impl FnMut(bool),
    ) -> Result<Option<usize>, E> {
        ordered_index::search(
            nodes,
            cursor,
            |id| self.compare_at(store, &resolve, id, &mut *before),
            descend,
        )
    }

    /// The query against the indexed atom at `id`, within one signed
    /// predicate: its arguments in order, each in its own read owner.
    pub(super) fn compare_at<'rows, E>(
        self,
        store: &Store,
        resolve: &impl Fn(usize) -> Option<AtomRef<'rows>>,
        id: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<std::cmp::Ordering, E> {
        before()?;
        let atom = resolve(id).expect("index references admitted atom");
        before()?;
        let arity = self.predicate(store).arity();
        for column in 0..arity {
            before()?;
            let left = self.argument(column, store);
            let right = atom.values().at(column).expect("same predicate arity");
            let order = left.compare_ref_with(right, &mut *before)?;
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(std::cmp::Ordering::Equal)
    }

    pub(super) fn intern_with<E>(
        self,
        store: &mut Store,
        limits: crate::catalog::Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, storage::Failure<E>> {
        match self {
            Self::Projected(_) => unreachable!("projected queries retain their prepared identity"),
            Self::Atom(atom) => store.import_atom_with(atom, limits, before),
            Self::SignedAtom(atom, sign) => store.import_row_with(
                atom.predicate().with_sign(sign),
                atom.values(),
                limits,
                before,
            ),
            Self::Key(key) => store.import_row_with(
                key.predicate(),
                (0..key.predicate().arity()).map(|column| key.argument(column)),
                limits,
                before,
            ),
        }
    }

    /// The same source authenticated by `identity_with`; preparation and this
    /// consumption are tied by the exclusive `AtomEntry`. Present rows read no
    /// arguments. Vacant rows reuse the exact tuple without another hash/probe.
    pub(super) fn publish_with<E>(
        self,
        store: &mut Store,
        prepared: PreparedAtom,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, storage::Failure<E>> {
        store.publish_prepared_atom_with(
            prepared,
            |column| match self {
                Self::Projected(projected) => projected.argument(column),
                Self::Atom(atom) | Self::SignedAtom(atom, _) => {
                    atom.values()
                        .at(column)
                        .expect("authenticated arity")
                        .canonical()
                        .expect("authenticated term")
                        .1
                }
                Self::Key(key) => {
                    key.argument(column)
                        .canonical()
                        .expect("authenticated term")
                        .1
                }
            },
            before,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use crate::atom_interner::{AtomInterner, Failure, Limits};
    use crate::{Atom, Predicate, Sign, Value};

    fn fixture() -> (AtomInterner, Limits) {
        let limits = Limits {
            max_atoms: 8,
            max_bytes: 1_048_576,
        };
        let mut owner = AtomInterner::new();
        for sign in [Sign::Negative, Sign::Positive] {
            let atom = Atom::new(
                Predicate::with_sign("p", 1, sign).unwrap(),
                vec![Value::Number(7)],
            )
            .unwrap();
            owner
                .entry_atom_with(&atom, limits, || Ok::<_, Infallible>(()))
                .unwrap()
                .insert_with(limits, || Ok::<_, Infallible>(()))
                .unwrap();
        }
        (owner, limits)
    }

    #[test]
    fn signed_lookup_borrows_the_opposite_identity() {
        let (owner, limits) = fixture();
        let original = owner.get(0).unwrap();
        let bytes = owner.storage_bytes();
        assert_eq!(
            owner
                .find_signed_atom_with(original, Sign::Positive, limits, || Ok::<_, Infallible>(()))
                .unwrap(),
            Some(1)
        );
        assert_eq!(owner.storage_bytes(), bytes);
        let signature = original.predicate().with_sign(Sign::Positive);
        assert_eq!(signature, owner.get(1).unwrap().predicate());
        assert_eq!(
            signature.name().as_ptr(),
            original.predicate().name().as_ptr()
        );
    }

    #[test]
    fn signed_lookup_preserves_every_caller_stop() {
        let (owner, limits) = fixture();
        let atom = owner.get(0).unwrap();
        let mut total = 0;
        owner
            .find_signed_atom_with(atom, Sign::Positive, limits, || {
                total += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap();
        for cutoff in 0..total {
            let cause = ("signed lookup", cutoff);
            let mut accepted = 0;
            let result = owner.find_signed_atom_with(atom, Sign::Positive, limits, || {
                if accepted == cutoff {
                    Err(&cause)
                } else {
                    accepted += 1;
                    Ok(())
                }
            });
            assert!(
                matches!(result, Err(Failure::Stopped(actual)) if std::ptr::eq(actual, &raw const cause))
            );
            assert_eq!(accepted, cutoff);
        }
    }
}
