//! Fixed-length propagation counts of one width, chosen once per knowledge.
//!
//! The caller supplies an upper bound on every count throughout its lifetime.
//! Narrowing supplies the largest chain length or initial per-atom parent
//! occurrence count, retaining distinct nodes that carry the same atom. A
//! chain counter is incremented once per processed neutral operand; absorbing
//! operands use a separate witness bit. A ranking counter is decremented from
//! its initial parent-occurrence count. The width is decided from the
//! bound when the knowledge is created ([`compact_fits`]) and never changes, so the
//! propagation loop works on one concrete width without a per-access choice.
//! Copies own their arrays independently; no width changes the arithmetic
//! count, propagation work or region's split ranking.

use std::fmt::Debug;
use std::mem::size_of;

/// A counter cell: `u16` or `u32` when the bound fits and saves space, else `usize`.
pub(super) trait Count: Copy + Debug + Default {
    /// The count as a native integer; a count never exceeds its bound.
    fn get(self) -> usize;
    /// The count increased by `amount`, within the incidence bound.
    fn plus(self, amount: usize) -> Self;
    /// The count decreased by one; a parent is counted before it is revisited.
    fn less_one(self) -> Self;
}

impl Count for u16 {
    #[inline]
    fn get(self) -> usize {
        usize::from(self)
    }
    #[inline]
    fn plus(self, amount: usize) -> Self {
        u16::try_from(amount)
            .ok()
            .and_then(|amount| self.checked_add(amount))
            .expect("a count never exceeds its incidence bound")
    }
    #[inline]
    fn less_one(self) -> Self {
        self.checked_sub(1)
            .expect("a parent is counted before it is revisited")
    }
}

impl Count for u32 {
    #[inline]
    fn get(self) -> usize {
        usize::try_from(self).expect("compact counts fit the wider native width")
    }
    #[inline]
    fn plus(self, amount: usize) -> Self {
        u32::try_from(amount)
            .ok()
            .and_then(|amount| self.checked_add(amount))
            .expect("a count never exceeds its incidence bound")
    }
    #[inline]
    fn less_one(self) -> Self {
        self.checked_sub(1)
            .expect("a parent is counted before it is revisited")
    }
}

impl Count for usize {
    #[inline]
    fn get(self) -> usize {
        self
    }
    #[inline]
    fn plus(self, amount: usize) -> Self {
        self.checked_add(amount)
            .expect("a count never exceeds its incidence bound")
    }
    #[inline]
    fn less_one(self) -> Self {
        self.checked_sub(1)
            .expect("a parent is counted before it is revisited")
    }
}

/// Whether `C` represents the bound while reducing payload relative to the
/// native width. Trying narrower widths first never introduces a size refusal:
/// counts that do not fit either compact width retain `usize` storage.
pub(super) fn compact_fits<C: TryFrom<usize>>(upper_bound: usize) -> bool {
    size_of::<C>() < size_of::<usize>() && C::try_from(upper_bound).is_ok()
}

#[derive(Debug)]
pub(super) struct Counters<C>(Box<[C]>);

impl<C: Clone> Clone for Counters<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0);
    }
}

impl<C: Count> Counters<C> {
    /// Allocation remains the existing infallible Knowledge construction
    /// contract.
    pub(super) fn zeros(length: usize) -> Self {
        Self(vec![C::default(); length].into_boxed_slice())
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub(super) fn get(&self, index: usize) -> usize {
        self.0[index].get()
    }

    #[inline]
    pub(super) fn add(&mut self, index: usize, amount: usize) {
        self.0[index] = self.0[index].plus(amount);
    }

    #[inline]
    pub(super) fn decrement(&mut self, index: usize) {
        self.0[index] = self.0[index].less_one();
    }

    /// Owned payload only. Knowledge accounts the enclosing header.
    pub(super) fn allocated_bytes(&self) -> u128 {
        self.0.len() as u128 * size_of::<C>() as u128
    }
}

#[cfg(test)]
mod tests;
