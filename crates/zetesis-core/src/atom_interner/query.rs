//! Typed atom and borrowed-key adapters for the shared ordered ID index.

use crate::{Atom, AtomKey, Value, identity, ordered_index};

use super::{
    get,
    index::{Link, Node},
};

#[derive(Clone, Copy)]
pub(super) enum Query<'a> {
    Atom(&'a Atom),
    Key(AtomKey<'a>),
}

impl Query<'_> {
    /// One node-work unit admits its probe, child selection and optional
    /// constant-size local direction recording. Typed descriptor/text work is
    /// separately charged. These units describe operations, not instructions.
    /// The observer cannot mutate the borrowed nodes. Pure find uses no recorder;
    /// entry retains directions without copying nodes or growing path scratch.
    pub(super) fn search<E>(
        self,
        committed: &[Atom],
        pending: &[Atom],
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
                let atom = get(committed, pending, id).expect("index references admitted atom");
                match self {
                    Self::Atom(value) => identity::atom(value, atom, before),
                    Self::Key(value) => value.compare_identity_with(atom, &mut *before),
                }
            },
            descend,
        )
    }

    pub(super) fn prepare_copy<E>(
        self,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        let predicate = match self {
            Self::Atom(atom) => atom.predicate(),
            Self::Key(key) => key.predicate(),
        };
        before()?;
        for _ in predicate.name().as_bytes() {
            before()?;
        }
        for column in 0..predicate.arity() {
            before()?;
            let value: &Value = match self {
                Self::Atom(atom) => &atom.values()[column],
                Self::Key(key) => key.argument(column),
            };
            if let Value::String(text) | Value::Symbol(text) = value {
                for _ in text.as_bytes() {
                    before()?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn to_atom(self) -> Atom {
        match self {
            Self::Atom(atom) => atom.clone(),
            Self::Key(key) => key.to_atom(),
        }
    }
}
