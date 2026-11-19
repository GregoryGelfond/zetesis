//! Checked atom queries for exact identity and ordered discovery.

use crate::catalog::storage::{self, AtomId, PreparedAtom, Store, TermId};
use crate::catalog::{AtomRef, PredicateRef, TermRef};
use crate::{AtomKey, PatternTerms, TemplateTerm};

use super::Failure;
use super::terms::RowColumn;
use crate::relation::Row;
use crate::template::catalog::TermData;

/// Unauthenticated coordinates resolve by content before the exact row lookup.
/// A local miss establishes absence from the exact canonical row index.
pub(super) enum Identity {
    Foreign,
    Local(PreparedAtom),
}

/// Content resolution applies no logical measure: it admits nothing.
const UNBOUNDED: crate::catalog::Limits = crate::catalog::Limits {
    max_nodes: usize::MAX,
    max_depth: usize::MAX,
    max_bytes: usize::MAX,
};

fn storage_failure<E>(error: storage::Failure<E>) -> Failure<E> {
    match error {
        storage::Failure::Storage(error) => Failure::Catalog(error),
        storage::Failure::Stopped(error) => Failure::Stopped(error),
    }
}

/// Concrete immutable inputs, rather than a stateful caller iterator.
#[derive(Clone, Copy)]
pub(super) enum SlotSource<'a> {
    Slots(&'a [usize]),
    Pattern(PatternTerms<'a>),
    Admitted(storage::Read<'a>, &'a [TermData]),
    Rows(&'a RowArguments<'a>),
}
impl<'a> SlotSource<'a> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Slots(slots) => slots.len(),
            Self::Pattern(terms) => terms.len(),
            Self::Admitted(_, terms) => terms.len(),
            Self::Rows(rows) => rows.terms.len(),
        }
    }

    pub(super) fn at(self, column: usize) -> TemplateTerm<'a> {
        match self {
            Self::Slots(slots) => TemplateTerm::Variable(slots[column]),
            Self::Pattern(terms) => terms.at(column).expect("projected argument arity"),
            Self::Admitted(read, terms) => TemplateTerm::admitted(read, terms[column]),
            Self::Rows(rows) => TemplateTerm::Constant(rows.argument(column)),
        }
    }
}

/// Checked relation coordinates stay borrowed until canonical publication.
/// The source plan authenticates the pattern, relation owners and selected rows;
/// this accessor has no caller code, deferred validation or tuple allocation.
pub(super) struct RowArguments<'a> {
    pub(super) source: storage::Read<'a>,
    pub(super) terms: &'a [TermData],
    pub(super) columns: &'a [Option<RowColumn>],
    pub(super) rows: &'a [Row<'a, 'a>],
}

impl<'a> RowArguments<'a> {
    pub(super) fn argument(&self, column: usize) -> TermRef<'a> {
        match self.terms[column] {
            TermData::Constant(id) => TermRef::new(self.source, id)
                .expect("prepared constant belongs to the admitted pattern"),
            TermData::Variable(_) => {
                let column = self.columns[column].expect("prepared variable has a row column");
                self.rows[column.input]
                    .atom()
                    .values()
                    .at(column.column)
                    .expect("prepared source column is inside its predicate")
            }
        }
    }
}

/// Borrowed coordinates shared by ordered lookup and canonical row admission.
/// The producer authenticates the scope, prefix and selected slots before this
/// becomes a Query. Insertion additionally checks its logical term limits.
/// No Store read is retained from the mutable writer.
pub(super) struct SlotQuery<'a> {
    pub(super) predicate: storage::PredicateId,
    pub(super) values: &'a [Option<TermId>],
    pub(super) arguments: SlotSource<'a>,
}

impl SlotQuery<'_> {
    pub(super) const HEADER_BYTES: u128 = size_of::<Self>() as u128;
    pub(super) const MAX_BYTES: u128 = Self::HEADER_BYTES + size_of::<RowArguments<'_>>() as u128;

    fn extra_bytes(&self) -> u128 {
        Self::HEADER_BYTES
            + match self.arguments {
                SlotSource::Rows(_) => size_of::<RowArguments<'_>>() as u128,
                _ => 0,
            }
    }

    /// The source is immutable and fully validated before this accessor runs.
    /// Admitted constants may resolve immutable segment metadata; this is not
    /// necessarily a raw constant-time ID load. No user callback, allocation or
    /// recoverable validation occurs during prepared row publication.
    fn argument(&self, column: usize) -> TermId {
        if let SlotSource::Admitted(_, terms) = self.arguments {
            return match terms[column] {
                TermData::Variable(slot) => self.values[slot].expect("validated bound slot"),
                TermData::Constant(id) => id,
            };
        }
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
    Slots(&'a SlotQuery<'a>),
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
        if let Self::Slots(projected) = self {
            return store
                .prepare_atom_by_with(
                    projected.predicate,
                    projected.arguments.len(),
                    |column| projected.argument(column),
                    before,
                )
                .map(Identity::Local)
                .map_err(storage_failure);
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
            .map_err(storage_failure)
    }

    pub(super) fn extra_bytes(self) -> u128 {
        match self {
            Self::Slots(projected) => projected.extra_bytes(),
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
            Self::Slots(projected) => {
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
            Self::Slots(projected) => {
                TermRef::new(store, projected.argument(column)).expect("validated projected prefix")
            }
        }
    }

    /// Resolve unauthenticated coordinates by content through this store's
    /// exact hash indexes: the signed predicate, each argument, then the row.
    /// Any missing component proves the atom absent; nothing is imported.
    /// Holds one arity-cell argument buffer and the term lookup's charged
    /// expansion scratch while resolving.
    pub(super) fn resolve_with<E>(
        self,
        store: &Store,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<AtomId>, Failure<E>> {
        let predicate = self.predicate(store);
        let Some(id) = store
            .find_predicate_with(predicate, &mut *before)
            .map_err(storage_failure)?
        else {
            return Ok(None);
        };
        let arity = predicate.arity();
        let mut lookup = storage::TermLookup::new(store);
        let mut terms = Vec::new();
        terms
            .try_reserve_exact(arity)
            .map_err(Failure::Allocation)?;
        for column in 0..arity {
            let Some(term) = lookup
                .find_term_with(self.argument(column, store), UNBOUNDED, &mut *before)
                .map_err(storage_failure)?
            else {
                return Ok(None);
            };
            terms.push(term);
        }
        let prepared = store
            .prepare_atom_by_with(id, arity, |column| terms[column], before)
            .map_err(storage_failure)?;
        Ok(prepared.present())
    }

    pub(super) fn intern_with<E>(
        self,
        store: &mut Store,
        limits: crate::catalog::Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, storage::Failure<E>> {
        match self {
            Self::Slots(_) => unreachable!("projected queries retain their prepared identity"),
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
                Self::Slots(projected) => projected.argument(column),
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

    use crate::atom_registry::{AtomRegistry, Failure, Limits};
    use crate::{Atom, Predicate, Sign, Value};

    fn fixture() -> (AtomRegistry, Limits) {
        let limits = Limits {
            max_atoms: 8,
            max_bytes: 1_048_576,
        };
        let mut owner = AtomRegistry::new();
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
