//! Fixed-length propagation counts with a checked, representation-only width.
//!
//! The caller supplies an upper bound on every count throughout its lifetime.
//! Narrowing supplies the total number of parent incidences: every chain
//! operand and every parent counted for an atom belongs to that stream. A
//! counter is incremented once per newly learned occurrence, or decremented
//! from its initial occurrence count. Width never changes during propagation.
//! Copies own their arrays independently; neither width changes the arithmetic
//! count, propagation work or region's split ranking.

use std::mem::size_of;

#[derive(Clone, Debug)]
pub(super) enum Counters {
    Compact(Box<[u32]>),
    Native(Box<[usize]>),
}

impl Counters {
    /// No new size refusal: a bound outside u32, or a host on which u32 saves
    /// no payload space, retains the native width. Allocation remains the
    /// existing infallible Knowledge construction contract.
    pub(super) fn zeros(length: usize, upper_bound: usize) -> Self {
        if size_of::<u32>() < size_of::<usize>() && u32::try_from(upper_bound).is_ok() {
            Self::Compact(vec![0; length].into_boxed_slice())
        } else {
            Self::Native(vec![0; length].into_boxed_slice())
        }
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Compact(values) => values.len(),
            Self::Native(values) => values.len(),
        }
    }

    pub(super) fn get(&self, index: usize) -> usize {
        match self {
            Self::Compact(values) => {
                usize::try_from(values[index]).expect("compact counts fit the wider native width")
            }
            Self::Native(values) => values[index],
        }
    }

    pub(super) fn add(&mut self, index: usize, amount: usize) {
        match self {
            Self::Compact(values) => {
                let amount = u32::try_from(amount).expect("an increment fits the incidence bound");
                values[index] = values[index]
                    .checked_add(amount)
                    .expect("a count never exceeds its incidence bound");
            }
            Self::Native(values) => {
                values[index] = values[index]
                    .checked_add(amount)
                    .expect("a count never exceeds its incidence bound");
            }
        }
    }

    pub(super) fn decrement(&mut self, index: usize) {
        match self {
            Self::Compact(values) => {
                values[index] = values[index]
                    .checked_sub(1)
                    .expect("a parent is counted before it is revisited");
            }
            Self::Native(values) => {
                values[index] = values[index]
                    .checked_sub(1)
                    .expect("a parent is counted before it is revisited");
            }
        }
    }

    /// Owned payload only. Knowledge accounts the enclosing enum header.
    pub(super) fn allocated_bytes(&self) -> u128 {
        match self {
            Self::Compact(values) => values.len() as u128 * size_of::<u32>() as u128,
            Self::Native(values) => values.len() as u128 * size_of::<usize>() as u128,
        }
    }
}

#[cfg(test)]
mod tests;
