//! Width-aware equality cells with unchanged logical identifiers.

use std::{mem::size_of, slice};

use super::{Failure, Work};

/// A borrowed equality column in original row order.
///
/// Physical widths change storage only. Every decoded value is the original
/// `u32` identifier in the enclosing relation's dictionary, never a logical
/// number or term-order rank. Variants permit dispatch once per bulk operation;
/// borrowing this view allocates no unpacked copy.
#[derive(Clone, Copy, Debug)]
pub enum Column<'a> {
    /// Identifiers at most 255.
    U8(&'a [u8]),
    /// Identifiers at most 65,535.
    U16(&'a [u16]),
    /// Full-width identifiers.
    U32(&'a [u32]),
}

impl Column<'_> {
    /// Number of row occurrences.
    #[must_use]
    pub const fn len(self) -> usize {
        match self {
            Self::U8(values) => values.len(),
            Self::U16(values) => values.len(),
            Self::U32(values) => values.len(),
        }
    }

    /// Whether there are no row occurrences.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Physical bits per cell, independent of the dictionary's semantic order.
    #[must_use]
    pub const fn bits(self) -> u32 {
        match self {
            Self::U8(_) => 8,
            Self::U16(_) => 16,
            Self::U32(_) => 32,
        }
    }

    /// The unchanged logical identifier at one original row position.
    #[must_use]
    pub fn get(self, position: usize) -> Option<u32> {
        match self {
            Self::U8(values) => values.get(position).copied().map(u32::from),
            Self::U16(values) => values.get(position).copied().map(u32::from),
            Self::U32(values) => values.get(position).copied(),
        }
    }
}

impl<'a> Column<'a> {
    /// Iterate decoded identifiers without allocating or changing row order.
    #[must_use]
    pub fn iter(self) -> Values<'a> {
        match self {
            Self::U8(values) => Values(Iter::U8(values.iter())),
            Self::U16(values) => Values(Iter::U16(values.iter())),
            Self::U32(values) => Values(Iter::U32(values.iter())),
        }
    }
}

impl PartialEq for Column<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for Column<'_> {}

impl<'a> IntoIterator for Column<'a> {
    type Item = u32;
    type IntoIter = Values<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &Column<'a> {
    type Item = u32;
    type IntoIter = Values<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

enum Iter<'a> {
    U8(slice::Iter<'a, u8>),
    U16(slice::Iter<'a, u16>),
    U32(slice::Iter<'a, u32>),
}

/// Allocation-free decoded iteration over one borrowed equality column.
pub struct Values<'a>(Iter<'a>);

impl Iterator for Values<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        match &mut self.0 {
            Iter::U8(values) => values.next().copied().map(u32::from),
            Iter::U16(values) => values.next().copied().map(u32::from),
            Iter::U32(values) => values.next().copied(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = self.len();
        (length, Some(length))
    }
}

impl DoubleEndedIterator for Values<'_> {
    fn next_back(&mut self) -> Option<u32> {
        match &mut self.0 {
            Iter::U8(values) => values.next_back().copied().map(u32::from),
            Iter::U16(values) => values.next_back().copied().map(u32::from),
            Iter::U32(values) => values.next_back().copied(),
        }
    }
}

impl ExactSizeIterator for Values<'_> {
    fn len(&self) -> usize {
        match &self.0 {
            Iter::U8(values) => values.len(),
            Iter::U16(values) => values.len(),
            Iter::U32(values) => values.len(),
        }
    }
}

impl std::iter::FusedIterator for Values<'_> {}

/// Only this owner stores cells. Growth can widen their physical type, but
/// never changes a published logical cell or publishes a partial appended row.
pub(super) enum OwnedColumn {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
}

impl OwnedColumn {
    pub(super) const fn new() -> Self {
        Self::U8(Vec::new())
    }

    pub(super) fn zeroed(rows: usize, largest_id: u32, work: &mut Work) -> Result<Self, Failure> {
        if u8::try_from(largest_id).is_ok() {
            Ok(Self::U8(zeroed(rows, work)?))
        } else if u16::try_from(largest_id).is_ok() {
            Ok(Self::U16(zeroed(rows, work)?))
        } else {
            Ok(Self::U32(zeroed(rows, work)?))
        }
    }

    pub(super) fn view(&self) -> Column<'_> {
        match self {
            Self::U8(values) => Column::U8(values),
            Self::U16(values) => Column::U16(values),
            Self::U32(values) => Column::U32(values),
        }
    }

    /// Admit capacity and any exact widening before an indivisible append.
    /// A refused copy leaves all old cells and their owner intact; an admitted
    /// widening may remain when a later reservation for another column fails.
    pub(super) fn reserve(&mut self, id: u32, work: &mut Work) -> Result<(), Failure> {
        let replacement = match self {
            Self::U8(values) if u8::try_from(id).is_ok() => return work.grow(values, 1),
            Self::U16(values) if u16::try_from(id).is_ok() => return work.grow(values, 1),
            Self::U32(values) => return work.grow(values, 1),
            Self::U8(values) if u16::try_from(id).is_ok() => Self::U16(widen(values, work)?),
            Self::U8(values) => Self::U32(widen(values, work)?),
            Self::U16(values) => Self::U32(widen(values, work)?),
        };
        match std::mem::replace(self, replacement) {
            Self::U8(values) => work.release(values),
            Self::U16(values) => work.release(values),
            Self::U32(values) => work.release(values),
        }
        Ok(())
    }

    /// `reserve(id)` establishes width and spare capacity before publication.
    pub(super) fn push(&mut self, id: u32) {
        match self {
            Self::U8(values) => values.push(u8::try_from(id).expect("reserved column width")),
            Self::U16(values) => values.push(u16::try_from(id).expect("reserved column width")),
            Self::U32(values) => values.push(id),
        }
    }

    /// Immutable construction admitted the dictionary's largest possible ID.
    pub(super) fn set(&mut self, row: usize, id: u32) {
        match self {
            Self::U8(values) => values[row] = u8::try_from(id).expect("admitted dictionary width"),
            Self::U16(values) => {
                values[row] = u16::try_from(id).expect("admitted dictionary width");
            }
            Self::U32(values) => values[row] = id,
        }
    }

    pub(super) fn clear(&mut self) {
        match self {
            Self::U8(values) => values.clear(),
            Self::U16(values) => values.clear(),
            Self::U32(values) => values.clear(),
        }
    }

    /// Heap capacity only; the enclosing vector owns this enum's header.
    pub(super) fn retained_bytes(&self) -> usize {
        match self {
            Self::U8(values) => values.capacity(),
            Self::U16(values) => values.capacity() * size_of::<u16>(),
            Self::U32(values) => values.capacity() * size_of::<u32>(),
        }
    }
}

fn zeroed<T: Copy + Default>(rows: usize, work: &mut Work) -> Result<Vec<T>, Failure> {
    let mut values = reserve(rows, work)?;
    if let Err(error) = work.tick(rows as u128) {
        work.release(values);
        return Err(error);
    }
    values.resize(rows, T::default());
    Ok(values)
}

fn widen<T: Copy, U: From<T>>(values: &[T], work: &mut Work) -> Result<Vec<U>, Failure> {
    let needed = values.len().checked_add(1).ok_or(Failure::Overflow)?;
    let mut widened = reserve(needed, work)?;
    for &value in values {
        if let Err(error) = work.tick(1) {
            work.release(widened);
            return Err(error);
        }
        widened.push(U::from(value));
    }
    Ok(widened)
}

/// A refused local reservation is dropped, unlike an owner-retained growth.
/// Keep the observed peak but remove that temporary buffer from live bytes.
fn reserve<T>(count: usize, work: &mut Work) -> Result<Vec<T>, Failure> {
    let previous = work.live;
    let result = work.reserve(count);
    if result.is_err() {
        work.live = previous;
    }
    result
}

#[cfg(test)]
mod tests;
