//! A relation whose every argument is bounded, held as a bit array.
//!
//! The argument bounds give each argument a finite list of values in
//! canonical order, so a tuple inside the bounds has a mixed-radix index: the
//! rank of each value, the first argument most significant. The index is a
//! bijection between the tuples inside the bounds and the bit positions, and
//! it orders tuples as the canonical atom order does, so a relation read off
//! its bits in position order is already in the order the model wants.
//! Membership is a bit test, insertion a bit set, and the rows matching a
//! bound prefix of arguments are one contiguous range of positions, so a
//! window over them is a scan of that range's words. No atom is allocated or
//! compared by value until the model is assembled.
//!
//! The New set of a round is a second bit array, cleared when the cutoff
//! advances; Old is present and not new. The bounds are an upper domain of
//! every derivable head, so a head outside them cannot arise; one that does
//! is an admitted-program invariant violation, not a missed row.

use std::{mem::size_of, ops::Range, sync::Arc};

use zetesis_core::{Atom, Predicate, Value};

use super::super::bounds::Bound;
use super::{RowSet, Work};
use crate::Stop;

/// The shape of a dense relation: the predicate, each argument's values in
/// canonical order, and the mixed-radix strides over them.
pub(in crate::oracle) struct Layout {
    predicate: Predicate,
    axes: Vec<Vec<Value>>,
    /// `strides[k]` is the product of the widths of the arguments after `k`.
    strides: Vec<usize>,
    cells: usize,
}

impl Layout {
    /// The layout of a predicate whose every argument is bounded and whose
    /// product of widths fits the ceiling; `None` keeps the tree.
    pub(in crate::oracle) fn new(
        predicate: &Predicate,
        bounds: &[Bound],
        ceiling: usize,
    ) -> Option<Self> {
        if bounds.len() != predicate.arity() {
            return None;
        }
        let mut axes = Vec::with_capacity(bounds.len());
        for bound in bounds {
            match bound {
                Bound::Finite(values) => axes.push(values.clone()),
                Bound::Unknown => return None,
            }
        }
        let mut strides = vec![1; axes.len()];
        let mut cells = 1usize;
        for (k, axis) in axes.iter().enumerate().rev() {
            strides[k] = cells;
            cells = cells.checked_mul(axis.len())?;
        }
        if cells > ceiling {
            return None;
        }
        Some(Self {
            predicate: predicate.clone(),
            axes,
            strides,
            cells,
        })
    }

    pub(in crate::oracle) fn predicate(&self) -> &Predicate {
        &self.predicate
    }

    /// Tuples inside the bounds: the bit positions.
    pub(in crate::oracle) fn cells(&self) -> usize {
        self.cells
    }

    fn words(&self) -> usize {
        self.cells.div_ceil(64)
    }

    /// Retained bytes: the value lists and strides. A shared name is counted
    /// once per layout, as the catalogs count theirs.
    pub(in crate::oracle) fn bytes(&self) -> u128 {
        let axes: u128 = self
            .axes
            .iter()
            .map(|axis| {
                axis.capacity() as u128 * size_of::<Value>() as u128
                    + axis
                        .iter()
                        .map(|value| value.checked_payload_capacity_bytes().unwrap_or(u128::MAX))
                        .fold(0u128, u128::saturating_add)
            })
            .fold(0u128, u128::saturating_add);
        (size_of::<Self>() as u128)
            .saturating_add(self.predicate.payload_capacity_bytes() as u128)
            .saturating_add(self.axes.capacity() as u128 * size_of::<Vec<Value>>() as u128)
            .saturating_add(axes)
            .saturating_add(self.strides.capacity() as u128 * size_of::<usize>() as u128)
    }

    fn rank(&self, argument: usize, value: &Value) -> Option<usize> {
        self.axes[argument].binary_search(value).ok()
    }

    /// The position of a tuple, or `None` when a value lies outside its
    /// argument's bound.
    pub(in crate::oracle) fn index_of<'v>(
        &self,
        values: impl IntoIterator<Item = &'v Value>,
    ) -> Option<usize> {
        let mut index = 0;
        let mut arguments = 0;
        for (argument, value) in values.into_iter().enumerate() {
            index += self.rank(argument, value)? * *self.strides.get(argument)?;
            arguments += 1;
        }
        (arguments == self.axes.len()).then_some(index)
    }

    /// The value of a position's tuple at an argument.
    pub(in crate::oracle) fn value(&self, argument: usize, index: usize) -> &Value {
        let axis = &self.axes[argument];
        &axis[(index / self.strides[argument]) % axis.len()]
    }

    /// The positions of the tuples whose first `bound` arguments take the
    /// given values: one contiguous range, empty when a value lies outside
    /// its bound.
    pub(in crate::oracle) fn prefix_range<'v>(
        &self,
        prefix: impl IntoIterator<Item = &'v Value>,
    ) -> Range<usize> {
        let mut base = 0;
        let mut block = self.cells;
        for (argument, value) in prefix.into_iter().enumerate() {
            let Some(rank) = self.rank(argument, value) else {
                return 0..0;
            };
            block = self.strides[argument];
            base += rank * block;
        }
        base..base + block
    }
}

/// The dense layouts a prepared program chose, by predicate.
#[derive(Default)]
pub(in crate::oracle) struct Layouts(Vec<Arc<Layout>>);

impl Layouts {
    pub(in crate::oracle) fn push(&mut self, layout: Layout) {
        debug_assert!(
            self.0
                .last()
                .is_none_or(|last| last.predicate() < layout.predicate()),
            "layouts are pushed in predicate order"
        );
        self.0.push(Arc::new(layout));
    }

    pub(in crate::oracle) fn get(&self, predicate: &Predicate) -> Option<&Arc<Layout>> {
        self.0
            .binary_search_by(|layout| layout.predicate().cmp(predicate))
            .ok()
            .map(|slot| &self.0[slot])
    }

    pub(in crate::oracle) fn len(&self) -> usize {
        self.0.len()
    }

    pub(in crate::oracle) fn bytes(&self) -> u128 {
        self.0.iter().map(|layout| layout.bytes()).fold(
            self.0.capacity() as u128 * size_of::<Arc<Layout>>() as u128,
            u128::saturating_add,
        )
    }
}

/// The rows of one dense relation: the tuples present, and among them the
/// tuples inserted since the round cutoff.
pub(in crate::oracle) struct Dense {
    layout: Arc<Layout>,
    present: Vec<u64>,
    new: Vec<u64>,
    count: usize,
    new_count: usize,
    /// The words holding new bits, so that advancing clears only them: an
    /// unchanged relation costs a round one unit, whatever its size.
    new_words: Range<usize>,
}

impl Dense {
    /// An empty relation over the layout. The two word vectors are the
    /// relation's whole variable storage.
    pub(super) fn new(layout: Arc<Layout>) -> Result<Self, Stop> {
        let words = layout.words();
        let mut present = Vec::new();
        let mut new = Vec::new();
        for vector in [&mut present, &mut new] {
            vector
                .try_reserve_exact(words)
                .map_err(|_| Stop::Allocation)?;
            vector.resize(words, 0);
        }
        Ok(Self {
            layout,
            present,
            new,
            count: 0,
            new_count: 0,
            new_words: 0..0,
        })
    }

    /// The bytes the two word vectors hold, for a layout of this shape.
    pub(super) fn word_bytes(layout: &Layout) -> u128 {
        2 * layout.words() as u128 * size_of::<u64>() as u128
    }

    pub(in crate::oracle) fn layout(&self) -> &Layout {
        &self.layout
    }

    pub(super) fn predicate(&self) -> &Predicate {
        self.layout.predicate()
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.count
    }

    pub(super) fn has_new(&self) -> bool {
        self.new_count > 0
    }

    fn word(&self, set: RowSet, word: usize) -> u64 {
        match set {
            RowSet::Current => self.present[word],
            RowSet::Old => self.present[word] & !self.new[word],
            RowSet::New => self.new[word],
        }
    }

    /// Whether the tuple at a position is in the set.
    pub(super) fn holds(&self, set: RowSet, position: usize) -> bool {
        position < self.layout.cells && self.word(set, position / 64) >> (position % 64) & 1 == 1
    }

    pub(super) fn contains(&self, position: usize) -> bool {
        self.holds(RowSet::Current, position)
    }

    /// Insert the tuple at a position; whether it was absent. The insertion
    /// is new until the cutoff advances.
    pub(super) fn insert(&mut self, position: usize) -> bool {
        let (word, bit) = (position / 64, 1u64 << (position % 64));
        if self.present[word] & bit != 0 {
            return false;
        }
        self.present[word] |= bit;
        self.new[word] |= bit;
        self.count += 1;
        self.new_count += 1;
        self.new_words = if self.new_words.is_empty() {
            word..word + 1
        } else {
            self.new_words.start.min(word)..self.new_words.end.max(word + 1)
        };
        true
    }

    /// Move the cutoff to the present extent: nothing is new.
    pub(super) fn advance(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 1 + self.new_words.len())?;
        self.new[self.new_words.clone()].fill(0);
        self.new_words = 0..0;
        self.new_count = 0;
        Ok(())
    }

    /// Empty the relation, keeping its capacity.
    pub(super) fn reset(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 2 * self.present.len())?;
        self.present.fill(0);
        self.new.fill(0);
        self.count = 0;
        self.new_count = 0;
        self.new_words = 0..0;
        Ok(())
    }

    /// The first position in the range whose tuple is in the set, advancing
    /// the range past it; `None` exhausts the range. Charges one unit per
    /// word examined, so a run of empty words costs one unit each.
    pub(super) fn next_row(
        &self,
        set: RowSet,
        range: &mut Range<usize>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        let end = range.end.min(self.layout.cells);
        let mut position = range.start;
        while position < end {
            charge(work, 1)?;
            let index = position / 64;
            let bits = self.word(set, index) & (!0u64 << (position % 64));
            if bits == 0 {
                position = (index + 1) * 64;
                continue;
            }
            let found = index * 64 + bits.trailing_zeros() as usize;
            if found >= end {
                break;
            }
            range.start = found + 1;
            return Ok(Some(found));
        }
        range.start = end;
        Ok(None)
    }

    /// The present tuples as atoms in position order, which is canonical
    /// order, leaving the relation empty. Each atom's bytes are admitted
    /// against the base before it is built.
    pub(super) fn take_atoms(
        &mut self,
        atoms: &mut Vec<Atom>,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<u128, Stop> {
        let mut live = base;
        let mut range = 0..self.layout.cells;
        while let Some(position) = self.next_row(RowSet::Current, &mut range, work)? {
            let values: Vec<Value> = (0..self.layout.axes.len())
                .map(|argument| self.layout.value(argument, position).clone())
                .collect();
            let atom = Atom::new(self.layout.predicate.clone(), values)
                .map_err(|_| Stop::InvalidProgram)?;
            live = live
                .checked_add(super::atom_bytes(&atom, work)?)
                .ok_or(Stop::StorageLimit)?;
            super::storage::admit(work, live)?;
            super::storage::record(work, live)?;
            atoms.push(atom);
        }
        self.reset(work)?;
        Ok(live)
    }
}

/// Charge relation work, which counts as catalog work.
fn charge(work: &mut Work<'_>, amount: usize) -> Result<(), Stop> {
    let before = work.statistics.work;
    let result = work.charge(amount);
    let charged = work.statistics.work - before;
    work.statistics.catalog_work = work
        .statistics
        .catalog_work
        .checked_add(charged)
        .ok_or(Stop::InvalidProgram)?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Control;

    fn numbers(values: &[i32]) -> Bound {
        Bound::Finite(values.iter().map(|&value| Value::Number(value)).collect())
    }

    fn layout() -> Layout {
        Layout::new(
            &Predicate::new("e", 2).unwrap(),
            &[numbers(&[1, 2, 3]), numbers(&[10, 20])],
            64,
        )
        .unwrap()
    }

    #[test]
    fn positions_are_mixed_radix_with_the_first_argument_most_significant() {
        let layout = layout();
        assert_eq!(layout.cells(), 6);
        let index = |a: i32, b: i32| layout.index_of([&Value::Number(a), &Value::Number(b)]);
        assert_eq!(index(1, 10), Some(0));
        assert_eq!(index(1, 20), Some(1));
        assert_eq!(index(3, 20), Some(5));
        assert_eq!(index(4, 10), None);
        assert_eq!(*layout.value(0, 5), Value::Number(3));
        assert_eq!(*layout.value(1, 4), Value::Number(10));
    }

    #[test]
    fn a_bound_prefix_is_one_contiguous_range() {
        let layout = layout();
        assert_eq!(layout.prefix_range([]), 0..6);
        assert_eq!(layout.prefix_range([&Value::Number(2)]), 2..4);
        assert_eq!(
            layout.prefix_range([&Value::Number(2), &Value::Number(20)]),
            3..4
        );
        assert_eq!(layout.prefix_range([&Value::Number(9)]), 0..0);
    }

    #[test]
    fn a_product_above_the_ceiling_or_an_unknown_argument_keeps_the_tree() {
        let predicate = Predicate::new("e", 2).unwrap();
        assert!(Layout::new(&predicate, &[numbers(&[1, 2, 3]), numbers(&[10, 20])], 5).is_none());
        assert!(Layout::new(&predicate, &[numbers(&[1]), Bound::Unknown], 64).is_none());
        assert!(Layout::new(&predicate, &[numbers(&[1])], 64).is_none());
    }

    #[test]
    fn rows_are_scanned_in_position_order_within_a_set() {
        let control = Control::default();
        let mut work = Work::source(&control, 1_000);
        let mut dense = Dense::new(Arc::new(layout())).unwrap();
        assert!(dense.insert(4));
        assert!(!dense.insert(4));
        dense.advance(&mut work).unwrap();
        assert!(dense.insert(1));
        assert!(dense.insert(5));
        let scan = |set: RowSet, work: &mut Work<'_>| {
            let mut range = 0..6;
            let mut found = Vec::new();
            while let Some(position) = dense.next_row(set, &mut range, work).unwrap() {
                found.push(position);
            }
            found
        };
        assert_eq!(scan(RowSet::Current, &mut work), vec![1, 4, 5]);
        assert_eq!(scan(RowSet::Old, &mut work), vec![4]);
        assert_eq!(scan(RowSet::New, &mut work), vec![1, 5]);
        assert!(dense.has_new());
        let mut range = 4..5;
        assert_eq!(
            dense
                .next_row(RowSet::Current, &mut range, &mut work)
                .unwrap(),
            Some(4)
        );
        assert_eq!(
            dense
                .next_row(RowSet::Current, &mut range, &mut work)
                .unwrap(),
            None
        );
    }

    #[test]
    fn taken_atoms_are_in_canonical_order_and_empty_the_relation() {
        let control = Control::default();
        let mut work = Work::source(&control, 10_000);
        work.limits.max_closure_bytes = 1 << 20;
        let mut dense = Dense::new(Arc::new(layout())).unwrap();
        for position in [5, 0, 3] {
            dense.insert(position);
        }
        let mut atoms = Vec::new();
        dense.take_atoms(&mut atoms, 0, &mut work).unwrap();
        let values: Vec<Vec<i32>> = atoms
            .iter()
            .map(|atom| {
                atom.values()
                    .iter()
                    .map(|value| match value {
                        Value::Number(number) => *number,
                        _ => unreachable!(),
                    })
                    .collect()
            })
            .collect();
        assert_eq!(values, vec![vec![1, 10], vec![2, 20], vec![3, 20]]);
        assert!(atoms.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(dense.len(), 0);
        assert!(!dense.contains(5));
    }
}
