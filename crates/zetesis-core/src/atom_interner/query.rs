//! One checked typed search for immutable membership and vacant-path recording.

use std::cmp::Ordering;

use crate::{Atom, AtomKey, Value, identity};

use super::{
    get,
    index::{Link, Node, position},
};

#[derive(Clone, Copy)]
pub(super) enum Query<'a> {
    Atom(&'a Atom),
    Key(AtomKey<'a>),
}

impl Query<'_> {
    /// The optional descent observer records no payload and cannot change the
    /// borrowed nodes. Both passes therefore use the same typed search. A pure
    /// probe supplies a no-op observer; only a confirmed vacant entry records
    /// tentative mutation metadata during its second pass.
    pub(super) fn search<E, F: FnMut() -> Result<(), E>>(
        self,
        committed: &[Atom],
        pending: &[Atom],
        nodes: &[Node],
        mut cursor: Link,
        before: &mut F,
        mut descend: impl FnMut(usize, &Node, bool, &mut F) -> Result<(), E>,
    ) -> Result<Option<usize>, E> {
        while let Some(next) = cursor {
            before()?;
            let id = position(next);
            let atom = get(committed, pending, id).expect("index references admitted atom");
            let order = match self {
                Self::Atom(value) => identity::atom(value, atom, before),
                Self::Key(value) => value.compare_identity_with(atom, &mut *before),
            }?;
            if order.is_eq() {
                return Ok(Some(id));
            }
            let right = order == Ordering::Greater;
            let node = &nodes[id];
            descend(id, node, right, before)?;
            cursor = node.children[usize::from(right)];
        }
        Ok(None)
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
