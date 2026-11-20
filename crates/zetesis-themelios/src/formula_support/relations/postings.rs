//! One column's postings: the rows holding each of the column's values, found
//! by the value's equality identifier.
//!
//! The identifiers a relation's dictionary assigns are small and mostly
//! consecutive, so a column's values are usually found through a dense
//! directory: one slot per identifier in the span the column has met, holding
//! the value's list index, read with one subtraction and one load. A column
//! whose identifiers are too scattered for the slots to stay within a small
//! multiple of its values is found through an ordered map instead, and returns
//! to the directory once its values fill their span again. Either way the
//! values are visited in ascending identifier order, as the ordered map alone
//! visited them before.

use std::collections::BTreeMap;
use std::mem::size_of;

use super::Memory;
use crate::FormulaFailure;
use crate::formula_support::Counters;

/// A directory slot naming no value.
const ABSENT: u32 = u32::MAX;

/// Slots a dense directory may hold beyond [`DENSITY`] per value.
const SLACK: usize = 64;

/// Slots per value a dense directory may hold beyond [`SLACK`]: four 4-byte
/// slots weigh what one ordered-map entry with its node share does.
const DENSITY: usize = 4;

/// The rows holding each value of one column, by equality identifier. Two
/// postings are equal when they hold the same rows for the same identifiers,
/// whichever directory finds them.
#[derive(Default)]
pub(crate) struct ColumnPostings {
    directory: Directory,
    /// Each value's rows, ascending, in the order the values were met.
    lists: Vec<Vec<usize>>,
}

enum Directory {
    /// The list index of identifier `base + offset` at `slots[offset]`.
    Dense { base: u32, slots: Vec<u32> },
    /// The list index of each identifier met.
    Sparse(BTreeMap<u32, u32>),
}

impl Default for Directory {
    fn default() -> Self {
        Self::Dense {
            base: 0,
            slots: Vec::new(),
        }
    }
}

impl PartialEq for ColumnPostings {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl std::fmt::Debug for ColumnPostings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_map().entries(self.iter()).finish()
    }
}

impl ColumnPostings {
    /// The rows holding identifier `id`, ascending; empty when none does.
    pub(crate) fn get(&self, id: u32) -> &[usize] {
        let list = self.find(id);
        list.map_or(&[], |list| self.lists[list as usize].as_slice())
    }

    /// The number of distinct values.
    pub(crate) fn len(&self) -> usize {
        self.lists.len()
    }

    /// Each value's identifier and rows, in ascending identifier order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, &[usize])> + '_ {
        let (dense, sparse) = match &self.directory {
            Directory::Dense { base, slots } => (Some((*base, slots)), None),
            Directory::Sparse(map) => (None, Some(map)),
        };
        dense
            .into_iter()
            .flat_map(|(base, slots)| {
                slots
                    .iter()
                    .enumerate()
                    .filter(|&(_, &list)| list != ABSENT)
                    .map(move |(offset, &list)| (identifier(base, offset), list))
            })
            .chain(sparse.into_iter().flatten().map(|(&id, &list)| (id, list)))
            .map(|(id, list)| (id, self.lists[list as usize].as_slice()))
    }

    /// Each value's rows, in ascending identifier order.
    pub(crate) fn values(&self) -> impl Iterator<Item = &[usize]> + '_ {
        self.iter().map(|(_, rows)| rows)
    }

    /// Row identifiers held.
    pub(crate) fn entries(&self) -> usize {
        self.lists.iter().map(Vec::len).sum()
    }

    /// Named capacity: the directory, the list headers and each list's rows.
    /// Map node overhead is excluded.
    pub(crate) fn bytes(&self) -> usize {
        let directory = match &self.directory {
            Directory::Dense { slots, .. } => slots.capacity() * size_of::<u32>(),
            Directory::Sparse(map) => map.len() * size_of::<(u32, u32)>(),
        };
        directory
            + self.lists.capacity() * size_of::<Vec<usize>>()
            + self
                .lists
                .iter()
                .map(|rows| rows.capacity() * size_of::<usize>())
                .sum::<usize>()
    }

    /// Append `row` to identifier `id`'s rows. Rows arrive ascending.
    pub(super) fn push(
        &mut self,
        id: u32,
        row: usize,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let list = match self.find(id) {
            Some(list) => list,
            None => self.insert(id, memory, counters)?,
        };
        let rows = &mut self.lists[list as usize];
        if rows.len() == rows.capacity() {
            counters.charge_work(rows.len() as u128, memory.limits, memory.location)?;
        }
        memory.reserve(rows, 1)?;
        rows.push(row);
        Ok(())
    }

    fn find(&self, id: u32) -> Option<u32> {
        match &self.directory {
            Directory::Dense { base, slots } => id
                .checked_sub(*base)
                .and_then(|offset| slots.get(offset as usize))
                .copied()
                .filter(|&list| list != ABSENT),
            Directory::Sparse(map) => map.get(&id).copied(),
        }
    }

    /// Give identifier `id` a new, empty list.
    fn insert(
        &mut self,
        id: u32,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<u32, FormulaFailure> {
        let list = u32::try_from(self.lists.len()).map_err(|_| {
            super::failure(zetesis_core::relation::Failure::Overflow, memory.location)
        })?;
        memory.reserve(&mut self.lists, 1)?;
        self.lists.push(Vec::new());
        let values = self.lists.len();
        let fits = match &self.directory {
            Directory::Dense { base, slots } => {
                let (low, high) = if slots.is_empty() {
                    (id, id)
                } else {
                    let end = identifier(*base, slots.len() - 1);
                    ((*base).min(id), end.max(id))
                };
                Some(((high - low) as usize) < SLACK + DENSITY * values)
            }
            Directory::Sparse(_) => None,
        };
        match fits {
            Some(true) => {
                let Directory::Dense { base, slots } = &mut self.directory else {
                    unreachable!("a dense directory");
                };
                if slots.is_empty() {
                    *base = id;
                }
                if id < *base {
                    let extra = (*base - id) as usize;
                    counters.charge_work(slots.len() as u128, memory.limits, memory.location)?;
                    memory.reserve(slots, extra)?;
                    slots.splice(0..0, std::iter::repeat_n(ABSENT, extra));
                    *base = id;
                }
                let offset = (id - *base) as usize;
                if offset >= slots.len() {
                    memory.reserve(slots, offset + 1 - slots.len())?;
                    slots.resize(offset + 1, ABSENT);
                }
                slots[offset] = list;
            }
            Some(false) => {
                self.sparse(memory, counters)?;
                self.insert_sparse(id, list, memory, counters)?;
            }
            None => self.insert_sparse(id, list, memory, counters)?,
        }
        Ok(list)
    }

    fn insert_sparse(
        &mut self,
        id: u32,
        list: u32,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let Directory::Sparse(map) = &mut self.directory else {
            unreachable!("a sparse directory");
        };
        memory.add(size_of::<(u32, u32)>())?;
        map.insert(id, list);
        let (Some((&low, _)), Some((&high, _))) = (map.first_key_value(), map.last_key_value())
        else {
            return Ok(());
        };
        // Half the dense allowance, so a column on the boundary does not
        // alternate between the two.
        if ((high - low) as usize + 1) * 2 <= SLACK + DENSITY * map.len() {
            self.dense(low, high, memory, counters)?;
        }
        Ok(())
    }

    /// Move a dense directory's identifiers into an ordered map.
    fn sparse(
        &mut self,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let Directory::Dense { base, slots } = &self.directory else {
            unreachable!("a dense directory");
        };
        counters.charge_work(slots.len() as u128, memory.limits, memory.location)?;
        let mut map = BTreeMap::new();
        for (offset, &list) in slots.iter().enumerate() {
            if list != ABSENT {
                memory.add(size_of::<(u32, u32)>())?;
                map.insert(identifier(*base, offset), list);
            }
        }
        memory.release(slots.capacity() * size_of::<u32>());
        self.directory = Directory::Sparse(map);
        Ok(())
    }

    /// Move an ordered map's identifiers, all within `low..=high`, into a
    /// dense directory.
    fn dense(
        &mut self,
        low: u32,
        high: u32,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let Directory::Sparse(map) = &self.directory else {
            unreachable!("a sparse directory");
        };
        let span = (high - low) as usize + 1;
        counters.charge_work(span as u128, memory.limits, memory.location)?;
        let mut slots = Vec::new();
        memory.reserve(&mut slots, span)?;
        slots.resize(span, ABSENT);
        for (&id, &list) in map {
            slots[(id - low) as usize] = list;
        }
        memory.release(map.len() * size_of::<(u32, u32)>());
        self.directory = Directory::Dense { base: low, slots };
        Ok(())
    }
}

/// The identifier at slot `offset` of a directory starting at `base`; slots
/// span identifiers, so the sum fits.
fn identifier(base: u32, offset: usize) -> u32 {
    base + u32::try_from(offset).expect("a slot offset spans identifiers")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use super::{ColumnPostings, Directory};
    use crate::FormulaLimits;
    use crate::formula_support::Counters;
    use crate::formula_support::relations::Memory;
    use crate::test_support::location;

    /// Push `ids` as rows 0, 1, ... and compare with an ordered map of lists;
    /// the charged bytes are the postings' named bytes.
    fn agrees(ids: &[u32]) -> ColumnPostings {
        let limits = FormulaLimits::default();
        let mut counters = Counters::default();
        let mut memory = Memory::new(0, 0, &limits, &counters, location());
        let mut postings = ColumnPostings::default();
        let mut expected: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (row, &id) in ids.iter().enumerate() {
            postings.push(id, row, &mut memory, &mut counters).unwrap();
            expected.entry(id).or_default().push(row);
            assert_eq!(postings.get(id), expected[&id].as_slice());
        }
        assert_eq!(postings.len(), expected.len());
        assert_eq!(postings.entries(), ids.len());
        assert!(
            postings
                .iter()
                .eq(expected.iter().map(|(&id, rows)| (id, rows.as_slice())))
        );
        for probe in [0, 1, u32::MAX, ids.iter().max().map_or(0, |id| id + 1)] {
            assert_eq!(
                postings.get(probe),
                expected.get(&probe).map_or(&[][..], Vec::as_slice)
            );
        }
        assert_eq!(memory.bytes, postings.bytes());
        postings
    }

    #[test]
    fn consecutive_identifiers_stay_dense() {
        let ids: Vec<u32> = (0..1000).rev().chain(500..1500).collect();
        let postings = agrees(&ids);
        assert!(matches!(postings.directory, Directory::Dense { .. }));
    }

    #[test]
    fn scattered_identifiers_move_to_the_map_and_back() {
        let mut ids = vec![7, 20_000, 60_000];
        let postings = agrees(&ids);
        assert!(matches!(postings.directory, Directory::Sparse(_)));
        ids.extend(0..60_000);
        let postings = agrees(&ids);
        assert!(matches!(postings.directory, Directory::Dense { .. }));
    }

    proptest! {
        #[test]
        fn postings_agree_with_an_ordered_map(
            ids in prop::collection::vec(
                prop_oneof![0_u32..64, 1000_u32..1100, any::<u32>()],
                0..400,
            ),
        ) {
            agrees(&ids);
        }
    }
}
