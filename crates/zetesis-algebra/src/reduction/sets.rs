//! Occurrence-stream refinements of intersection and union.
//!
//! A stream denotes the set of its values, independently of order or repeated
//! occurrences. These operations compute that set value without collecting it;
//! the caller retains its existing canonical storage and fallible publication.
//! They are refinements of `AxisFolds.inter` and `AxisFolds.union`, not a new set
//! owner or a claim that a fallible materializer implements pure `Reduction`.
//!
//! Representation reads must be faithful, finite and free of allocation or
//! control effects. For ordered union, comparison must additionally be a pure
//! total order consistent with value equality. Rust's Iterator/Ord bounds do
//! not enforce these premises. No result authenticates semantic membership or
//! coverage. An empty stream is not an empty-family verdict, and this interface
//! neither represents the universal intersection identity nor decides a fold.

use std::cmp::Ordering;
use std::iter::{Chain, Peekable};

/// Select left occurrences whose recorded membership in the right set is true.
///
/// Each record contains an existing left occurrence and the result of its
/// existing right-set lookup. For every logical value that lookup must agree
/// with right-set membership; equal left occurrences must agree. No membership
/// callback, second lookup or retained set is introduced by this operation.
/// Reads remain lazy: requesting an output visits records through the first
/// selected occurrence, preserving left order and repeated occurrences.
#[must_use]
#[inline]
pub fn intersection<I, T>(records: I) -> Intersection<I>
where
    I: Iterator<Item = (T, bool)>,
    T: Copy,
{
    Intersection { records }
}

/// Lazy selected occurrences, retaining only the supplied representation cursor.
/// No storage is allocated and no record is read until iteration requests it.
pub struct Intersection<I> {
    records: I,
}

impl<I, T> Iterator for Intersection<I>
where
    I: Iterator<Item = (T, bool)>,
    T: Copy,
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        for (occurrence, member) in self.records.by_ref() {
            if member {
                return Some(occurrence);
            }
        }
        None
    }
}

/// Denote union by every left occurrence followed by every right occurrence.
///
/// Repetitions are deliberately preserved. A caller whose existing publication
/// admits occurrences before canonicalizing them must retain that effect order;
/// replacing this stream with a unique merge would change its resource trace.
/// This function advances neither input and allocates no storage.
#[inline]
pub fn union<I, J>(left: I, right: J) -> Chain<I, J>
where
    I: Iterator,
    J: Iterator<Item = I::Item>,
    I::Item: Copy,
{
    left.chain(right)
}

/// Canonical union of two strictly increasing streams under their value order.
///
/// The caller establishes ordering and uniqueness within each input; no
/// validation scan is added. Equal cross-input values keep the left
/// representative, consuming the right occurrence first. Each next operation
/// peeks left then right and performs at most one comparison. The caller must
/// perform any capacity admission before constructing/draining this cursor.
/// The cursor retains only the two input cursors and one lookahead per input;
/// it performs no allocation, copying of owned payload or fallible publication.
#[must_use]
#[inline]
pub fn ordered_union<I, J>(left: I, right: J) -> OrderedUnion<I, J>
where
    I: Iterator,
    J: Iterator<Item = I::Item>,
    I::Item: Copy + Ord,
{
    OrderedUnion {
        left: left.peekable(),
        right: right.peekable(),
    }
}

/// A two-input merge cursor; no collected set or catalog authority is added.
pub struct OrderedUnion<I: Iterator, J: Iterator<Item = I::Item>> {
    left: Peekable<I>,
    right: Peekable<J>,
}

impl<I, J> Iterator for OrderedUnion<I, J>
where
    I: Iterator,
    J: Iterator<Item = I::Item>,
    I::Item: Copy + Ord,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match (self.left.peek(), self.right.peek()) {
            (None, None) => None,
            (Some(_), None) => self.left.next(),
            (None, Some(_)) => self.right.next(),
            (Some(first), Some(second)) => match first.cmp(second) {
                Ordering::Less => self.left.next(),
                Ordering::Greater => self.right.next(),
                Ordering::Equal => {
                    self.right.next();
                    self.left.next()
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
