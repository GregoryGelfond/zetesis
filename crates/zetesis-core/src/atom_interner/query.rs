//! Borrowed complete identities for the shared ordered discovery index.

use crate::catalog::storage::{self, AtomId, Store};
use crate::catalog::{AssignmentSlice, AtomRef, PredicateRef, TermRef};
use crate::{AtomKey, ordered_index};

use super::index::{Link, Node};

#[derive(Clone, Copy)]
pub(super) enum Query<'a> {
    Atom(AtomRef<'a>),
    SignedAtom(AtomRef<'a>, crate::Sign),
    Key(AtomKey<'a>),
    Assigned {
        predicate: storage::PredicateId,
        values: AssignmentSlice<'a>,
        slots: &'a [usize],
        limits: crate::catalog::Limits,
    },
}

impl<'a> Query<'a> {
    pub(super) fn predicate<'read>(self, store: &'read Store) -> PredicateRef<'read>
    where
        'a: 'read,
    {
        match self {
            Self::Atom(atom) => atom.predicate(),
            Self::SignedAtom(atom, sign) => atom.predicate().with_sign(sign),
            Self::Key(key) => key.predicate(),
            Self::Assigned { predicate, .. } => {
                PredicateRef::new(storage::Read::from(store), predicate)
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
            Self::Assigned { values, slots, .. } => TermRef::new(
                store,
                values.slots[slots[column]].expect("validated assigned slot"),
            )
            .expect("validated assigned prefix"),
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
            |id| {
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
            },
            descend,
        )
    }

    pub(super) fn intern_with<E>(
        self,
        store: &mut Store,
        limits: crate::catalog::Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, storage::Failure<E>> {
        match self {
            Self::Assigned {
                predicate,
                values,
                slots,
                limits,
            } => store.import_assigned_row_with(predicate, values.slots, slots, limits, before),
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
