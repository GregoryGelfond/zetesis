//! Shared construction metadata, distinct from final root-owned evidence.
//!
//! Each atom owns two chain headers and a reference to its first source entry. Entries live
//! in shared append arenas: producers retain insertion order and origins retain
//! full `Location` order without duplicates. Links never cross arena owners.
//! This removes per-atom staging allocations, at the cost of one link per entry.
//! The arena remains live while the public root-owned origin vectors are built.

use std::cmp::Ordering;
use std::iter::FusedIterator;
use std::num::NonZeroUsize;

use themelios_base::span::Location;

use crate::expansion::Budget;
use crate::formula_support::Counters;
use crate::{ExpansionResource, FormulaFailure, FormulaLimits};

#[cfg(test)]
mod tests;

type Link = Option<NonZeroUsize>;

#[derive(Clone, Copy, Default)]
struct Chain {
    first: Link,
    last: Link,
    len: usize,
}

struct Entry<T> {
    value: T,
    next: Link,
}

struct Atom {
    first_origin: NonZeroUsize,
    producers: Chain,
    origins: Chain,
}

#[derive(Default)]
pub(super) struct Metadata {
    atoms: Vec<Atom>,
    producers: Vec<Entry<usize>>,
    origins: Vec<Entry<Location>>,
}

impl Metadata {
    /// The caller has admitted this atom and its first source occurrence.
    /// Existing atom/producer work ticks cover fixed publication; shared arena
    /// relocations, origin searches/inserts and final evidence copies are extra.
    pub(super) fn atom(
        &mut self,
        location: Location,
        counters: &mut Counters,
        limits: &FormulaLimits,
    ) -> Result<(), FormulaFailure> {
        counters.charge_work(
            growth(&self.atoms) + growth(&self.origins),
            limits,
            location,
        )?;
        reserve(&mut self.atoms, 1, location)?;
        reserve(&mut self.origins, 1, location)?;
        let first = append(&mut self.origins, location, None);
        self.atoms.push(Atom {
            first_origin: first,
            producers: Chain::default(),
            origins: Chain {
                first: Some(first),
                last: Some(first),
                len: 1,
            },
        });
        Ok(())
    }

    pub(super) fn location(&self, atom: usize) -> Location {
        self.origins[self.atoms[atom].first_origin.get() - 1].value
    }

    pub(super) fn producer(
        &mut self,
        atom: usize,
        antecedent: usize,
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        counters.charge_work(growth(&self.producers), limits, location)?;
        reserve(&mut self.producers, 1, location)?;
        let entry = append(&mut self.producers, antecedent, None);
        let chain = &mut self.atoms[atom].producers;
        if let Some(last) = chain.last {
            self.producers[last.get() - 1].next = Some(entry);
        } else {
            chain.first = Some(entry);
        }
        chain.last = Some(entry);
        chain.len += 1;
        Ok(())
    }

    pub(super) fn origin(
        &mut self,
        atom: usize,
        location: Location,
        budget: &mut Budget,
        counters: &mut Counters,
        limits: &FormulaLimits,
    ) -> Result<(), FormulaFailure> {
        let position = self.find_origin(atom, location);
        let (previous, next, work) = match position {
            Position::Present { work } => {
                return counters.charge_work(work, limits, location);
            }
            Position::Vacant {
                previous,
                next,
                work,
            } => (previous, next, work),
        };
        // Keep first-occurrence admission ahead of the work refusal. Search is
        // bounded by the already admitted origin chain; refusal publishes no link.
        budget.charge(ExpansionResource::Origins, 1, location)?;
        counters.charge_work(work + 1 + growth(&self.origins), limits, location)?;
        reserve(&mut self.origins, 1, location)?;
        let entry = append(&mut self.origins, location, next);
        let chain = &mut self.atoms[atom].origins;
        if let Some(previous) = previous {
            self.origins[previous.get() - 1].next = Some(entry);
        } else {
            chain.first = Some(entry);
        }
        if next.is_none() {
            chain.last = Some(entry);
        }
        chain.len += 1;
        Ok(())
    }

    fn find_origin(&self, atom: usize, location: Location) -> Position {
        let chain = self.atoms[atom].origins;
        let last = chain.last.expect("every atom retains its first origin");
        match location.cmp(&self.origins[last.get() - 1].value) {
            Ordering::Equal => return Position::Present { work: 1 },
            Ordering::Greater => {
                return Position::Vacant {
                    previous: Some(last),
                    next: None,
                    work: 1,
                };
            }
            Ordering::Less => {}
        }
        let mut previous = None;
        let mut next = chain.first;
        let mut work = 1;
        while let Some(index) = next {
            let entry = &self.origins[index.get() - 1];
            work += 1;
            match location.cmp(&entry.value) {
                Ordering::Equal => return Position::Present { work },
                Ordering::Less => {
                    return Position::Vacant {
                        previous,
                        next,
                        work,
                    };
                }
                Ordering::Greater => {
                    previous = next;
                    next = entry.next;
                }
            }
        }
        unreachable!("the larger tail bounds this sorted insertion search")
    }

    pub(super) fn producers(
        &self,
        atom: usize,
    ) -> impl ExactSizeIterator<Item = usize> + FusedIterator + '_ {
        Values::new(&self.producers, self.atoms[atom].producers)
    }

    pub(super) fn origins(
        &self,
        atom: usize,
    ) -> impl ExactSizeIterator<Item = Location> + FusedIterator + '_ {
        Values::new(&self.origins, self.atoms[atom].origins)
    }

    /// Called only after root-count and emitted-origin admission. Reserve the
    /// required final view once; charge both link traversal and location copying.
    pub(super) fn copy_origins(
        &self,
        atom: usize,
        counters: &mut Counters,
        limits: &FormulaLimits,
    ) -> Result<Vec<Location>, FormulaFailure> {
        let origins = self.origins(atom);
        let location = self.location(atom);
        counters.charge_work(origins.len() as u128 * 2, limits, location)?;
        let mut result = Vec::new();
        reserve(&mut result, origins.len(), location)?;
        result.extend(origins);
        Ok(result)
    }
}

enum Position {
    Present {
        work: u128,
    },
    Vacant {
        previous: Link,
        next: Link,
        work: u128,
    },
}

struct Values<'a, T> {
    entries: &'a [Entry<T>],
    next: Link,
    remaining: usize,
}

impl<'a, T> Values<'a, T> {
    fn new(entries: &'a [Entry<T>], chain: Chain) -> Self {
        Self {
            entries,
            next: chain.first,
            remaining: chain.len,
        }
    }
}

impl<T: Copy> Iterator for Values<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let index = self.next?;
        let entry = &self.entries[index.get() - 1];
        self.next = entry.next;
        self.remaining -= 1;
        Some(entry.value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T: Copy> ExactSizeIterator for Values<'_, T> {}
impl<T: Copy> FusedIterator for Values<'_, T> {}

fn growth<T>(entries: &Vec<T>) -> u128 {
    if entries.len() == entries.capacity() {
        entries.len() as u128
    } else {
        0
    }
}

fn reserve<T>(
    entries: &mut Vec<T>,
    additional: usize,
    location: Location,
) -> Result<(), FormulaFailure> {
    entries
        .try_reserve(additional)
        .map_err(|error| FormulaFailure::MetadataAllocation { error, location })
}

/// The reservation already succeeded. Every entry is nonzero-sized, so Vec's
/// isize-sized allocation bound makes len + 1 representable. Links encode
/// absence explicitly; zero is never a valid arena position.
fn append<T>(entries: &mut Vec<Entry<T>>, value: T, next: Link) -> NonZeroUsize {
    let index = NonZeroUsize::new(entries.len() + 1).expect("one-based arena position");
    entries.push(Entry { value, next });
    index
}
