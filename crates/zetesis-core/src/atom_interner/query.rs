//! One checked typed search and a bounded local record of its descent directions.

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

/// Fixed traversal state, never another index or an atom owner.
///
/// Let `N(h)` be the minimum node count of an AVL of height h, with empty height
/// zero. Its recurrence gives `N(h) >= 2*N(h-2)+1` and hence
/// `N(h) >= 2^ceil(h/2)-1`. For a nonempty tree with n nodes and bit width b,
/// `n < 2^b` implies `h <= 2*b`. Since `b <= usize::BITS`, two target-sized words
/// hold every descent. The planned insertion leaf is not a descent bit.
/// The empty tree records no directions. This argument uses mathematical `n+1`;
/// no potentially overflowing machine addition is needed to compute the bound.
///
/// Checked packing protects the representation independently of that AVL
/// invariant. A failed push from an actual search means the internal AVL height
/// invariant was broken, not a user resource refusal or missing atom.
/// This local stack record lives only during entry lookup/path preparation;
/// its two words and checked length are excluded from named vector capacities.
#[derive(Default)]
pub(super) struct Directions {
    words: [usize; 2],
    length: usize,
}

impl Directions {
    pub(super) fn push(&mut self, right: bool) -> Option<()> {
        let next = self.length.checked_add(1)?;
        if next > self.words.len() * usize::BITS as usize {
            return None;
        }
        // The newest direction occupies bit zero. Transfer the low word's
        // oldest bit before shifting; the admitted length ensures that no
        // recorded bit can leave the high word. Decode positions only on replay.
        let carry = self.words[0] >> (usize::BITS - 1);
        self.words[0] = (self.words[0] << 1) | usize::from(right);
        self.words[1] = (self.words[1] << 1) | carry;
        self.length = next;
        Some(())
    }

    pub(super) fn len(&self) -> usize {
        self.length
    }

    pub(super) fn get(&self, position: usize) -> Option<bool> {
        if position >= self.length {
            return None;
        }
        let (word, mask) = Self::position(self.length - 1 - position)?;
        Some(*self.words.get(word)? & mask != 0)
    }

    fn position(position: usize) -> Option<(usize, usize)> {
        let width = usize::BITS as usize;
        let offset = u32::try_from(position % width).ok()?;
        Some((position / width, 1_usize.checked_shl(offset)?))
    }
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
        mut cursor: Link,
        before: &mut impl FnMut() -> Result<(), E>,
        mut descend: impl FnMut(bool),
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
            descend(right);
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
