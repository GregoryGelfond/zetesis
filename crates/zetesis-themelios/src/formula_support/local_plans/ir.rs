//! Preserve a source IR borrow through shorter-lived joins and nested cursors.
//!
//! Factoring also supplies temporary IR. Both variants use the same lowering
//! and owned plan representation; only Source can enter the snapshot's table.
use std::ops::Deref;

pub(crate) enum Ir<'a, 'source, T: ?Sized> {
    Source(&'source T),
    Temporary(&'a T),
}
impl<T: ?Sized> Copy for Ir<'_, '_, T> {}
impl<T: ?Sized> Clone for Ir<'_, '_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, 'source: 'a, T: ?Sized> Ir<'a, 'source, T> {
    pub(crate) fn get(self) -> &'a T {
        match self {
            // Explicitly shorten the source borrow; a combined pattern would
            // require the two variant lifetimes to be equal.
            Self::Source(value) => value as &'a T,
            Self::Temporary(value) => value,
        }
    }
    pub(crate) fn map<U: ?Sized>(self, select: impl Fn(&T) -> &U) -> Ir<'a, 'source, U> {
        match self {
            Self::Source(value) => Ir::Source(select(value)),
            Self::Temporary(value) => Ir::Temporary(select(value)),
        }
    }
}
impl<'a, 'source: 'a, T: ?Sized> Deref for Ir<'a, 'source, T> {
    type Target = T;
    fn deref(&self) -> &T {
        (*self).get()
    }
}
impl<'a, 'source: 'a, T> Ir<'a, 'source, [T]> {
    pub(crate) fn iter(self) -> impl Iterator<Item = Ir<'a, 'source, T>> {
        (0..self.len()).map(move |index| self.map(|items| &items[index]))
    }
}

impl<'a, T: ?Sized> From<&'a T> for Ir<'a, '_, T> {
    fn from(value: &'a T) -> Self {
        Self::Temporary(value)
    }
}

impl<'a, T> From<&'a Vec<T>> for Ir<'a, '_, [T]> {
    fn from(value: &'a Vec<T>) -> Self {
        Self::Temporary(value)
    }
}
impl<'a, T, const N: usize> From<&'a [T; N]> for Ir<'a, '_, [T]> {
    fn from(value: &'a [T; N]) -> Self {
        Self::Temporary(value)
    }
}
