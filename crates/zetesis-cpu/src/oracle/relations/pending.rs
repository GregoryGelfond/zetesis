//! One round's truth membership, using this execution's discovery coordinates.
//!
//! The set never crosses an authority or round boundary. Marks reject duplicate
//! heads; collected IDs are sorted by decoded identity before publication. No
//! atom payload, equality dictionary or second interner is retained.

use super::{Work, charge, storage};
use crate::Stop;
use std::mem::size_of;
use zetesis_core::atom_interner::AtomAppender;

#[derive(Default)]
pub(in crate::oracle) struct Pending {
    ids: Vec<usize>,
    marks: Vec<u64>,
}

impl Pending {
    pub(in crate::oracle) fn len(&self) -> usize {
        self.ids.len()
    }
    pub(in crate::oracle) fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    pub(in crate::oracle) fn bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.ids.capacity() as u128 * size_of::<usize>() as u128
            + self.marks.capacity() as u128 * size_of::<u64>() as u128
    }
    pub(in crate::oracle) fn contains(&self, id: usize) -> bool {
        self.marks
            .get(id / 64)
            .is_some_and(|word| word & (1 << (id % 64)) != 0)
    }
    pub(in crate::oracle) fn insert(
        &mut self,
        id: usize,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        charge(work, 1)?;
        if self.contains(id) {
            return Ok(());
        }
        let words = (id / 64).checked_add(1).ok_or(Stop::StorageLimit)?;
        let additional = words.saturating_sub(self.marks.len());
        let live = base.checked_add(self.bytes()).ok_or(Stop::StorageLimit)?;
        let live = storage::reserve(&mut self.marks, additional, live, work)?;
        storage::reserve(&mut self.ids, 1, live, work)?;
        charge(work, additional.checked_add(2).ok_or(Stop::StorageLimit)?)?;
        self.marks.resize(self.marks.len().max(words), 0);
        self.marks[id / 64] |= 1 << (id % 64);
        self.ids.push(id);
        Ok(())
    }

    /// A bottom-up merge sort; each descriptor/navigation comparison and copied
    /// ID is admitted. Refusal leaves only disposable, round-local metadata.
    pub(in crate::oracle) fn order(
        &mut self,
        appender: &AtomAppender<'_>,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let mut scratch = Vec::new();
        let live = base
            .checked_add(self.bytes())
            .and_then(|bytes| bytes.checked_add(size_of::<Vec<usize>>() as u128))
            .ok_or(Stop::StorageLimit)?;
        storage::admit(work, live)?;
        storage::record(work, live)?;
        storage::reserve(&mut scratch, self.ids.len(), live, work)?;
        let mut width = 1;
        while width < self.ids.len() {
            scratch.clear();
            let mut start = 0;
            while start < self.ids.len() {
                let middle = start.saturating_add(width).min(self.ids.len());
                let end = middle.saturating_add(width).min(self.ids.len());
                let (mut left, mut right) = (start, middle);
                while left < middle && right < end {
                    charge(work, 1)?;
                    let first = appender.get(self.ids[left]).ok_or(Stop::InvalidProgram)?;
                    let second = appender.get(self.ids[right]).ok_or(Stop::InvalidProgram)?;
                    let before = first.compare_ref_with(second, || charge(work, 1))?.is_le();
                    charge(work, 1)?;
                    scratch.push(if before {
                        let id = self.ids[left];
                        left += 1;
                        id
                    } else {
                        let id = self.ids[right];
                        right += 1;
                        id
                    });
                }
                charge(work, middle - left + end - right)?;
                scratch.extend_from_slice(&self.ids[left..middle]);
                scratch.extend_from_slice(&self.ids[right..end]);
                start = end;
            }
            std::mem::swap(&mut self.ids, &mut scratch);
            width = width.saturating_mul(2);
        }
        Ok(())
    }

    pub(in crate::oracle) fn ids(&self) -> &[usize] {
        &self.ids
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures;
    use super::*;
    use crate::Cancellation;
    use zetesis_core::{Atom, Predicate, Value};

    #[test]
    fn duplicate_marks_remain_one_discovery_across_word_boundaries() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        let mut pending = Pending::default();
        for id in [130, 63, 64, 0, 130, 64, 63] {
            pending.insert(id, 0, &mut work).unwrap();
        }
        assert_eq!(pending.ids(), [130, 63, 64, 0]);
        for id in [0, 63, 64, 130] {
            assert!(pending.contains(id));
        }
        for id in [1, 62, 65, 129, 131] {
            assert!(!pending.contains(id));
        }
    }

    #[test]
    fn pending_publication_order_is_semantic_not_discovery_order() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        let source = [3, 1, 2].map(|value| {
            Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(value)]).unwrap()
        });
        let mut catalogs = fixtures::catalogs(&source, &mut work);
        let mut pending = Pending::default();
        for atom in &source {
            let id = fixtures::intern(&mut catalogs, atom, &mut work);
            pending
                .insert(id, catalogs.total_bytes(), &mut work)
                .unwrap();
        }
        assert_eq!(pending.ids(), [0, 1, 2]);
        let (round, appender) = catalogs.split().unwrap();
        pending
            .order(
                &appender,
                round.base_bytes() + appender.storage_bytes(),
                &mut work,
            )
            .unwrap();
        assert_eq!(pending.ids(), [1, 2, 0]);
        let values: Vec<_> = pending
            .ids()
            .iter()
            .map(|&id| appender.get(id).unwrap().values().at(0).unwrap())
            .collect();
        assert_eq!(values, [1, 2, 3].map(Value::Number));
    }
}
