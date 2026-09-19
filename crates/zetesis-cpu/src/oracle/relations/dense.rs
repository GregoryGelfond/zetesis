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

use zetesis_core::{Atom, AtomKey, Predicate, Value};

use super::super::bounds::Bound;
use super::{RowSet, Work, charge};
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
    /// once per layout, as the catalogs count theirs. `None` when the sum
    /// does not fit.
    pub(in crate::oracle) fn bytes(&self) -> Option<u128> {
        let axes = self.axes.iter().try_fold(0u128, |sum, axis| {
            let cells = (axis.capacity() as u128).checked_mul(size_of::<Value>() as u128)?;
            let payload = axis.iter().try_fold(0u128, |sum, value| {
                sum.checked_add(value.checked_payload_capacity_bytes()?)
            })?;
            sum.checked_add(cells)?.checked_add(payload)
        })?;
        (size_of::<Self>() as u128)
            .checked_add(self.predicate.payload_capacity_bytes() as u128)?
            .checked_add(
                (self.axes.capacity() as u128).checked_mul(size_of::<Vec<Value>>() as u128)?,
            )?
            .checked_add(axes)?
            .checked_add((self.strides.capacity() as u128).checked_mul(size_of::<usize>() as u128)?)
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

    /// The values of the last argument in canonical order, which index a
    /// position within a block of a bound prefix; `None` for arity zero.
    pub(in crate::oracle) fn last_values(&self) -> Option<&[Value]> {
        self.axes.last().map(Vec::as_slice)
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
        self.slot(predicate).map(|slot| &self.0[slot])
    }

    /// The layout's position among the layouts, which indexes the pending
    /// rows and is fixed for the preparation.
    pub(in crate::oracle) fn slot(&self, predicate: &Predicate) -> Option<usize> {
        self.0
            .binary_search_by(|layout| layout.predicate().cmp(predicate))
            .ok()
    }

    /// The layouts in predicate order, which is slot order.
    pub(in crate::oracle) fn iter(&self) -> impl Iterator<Item = &Arc<Layout>> {
        self.0.iter()
    }

    pub(in crate::oracle) fn len(&self) -> usize {
        self.0.len()
    }

    /// Retained bytes of every layout; `None` when the sum does not fit.
    pub(in crate::oracle) fn bytes(&self) -> Option<u128> {
        let handles = (self.0.capacity() as u128).checked_mul(size_of::<Arc<Layout>>() as u128)?;
        self.0
            .iter()
            .try_fold(handles, |sum, layout| sum.checked_add(layout.bytes()?))
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

    pub(in crate::oracle) fn contains(&self, position: usize) -> bool {
        self.holds(RowSet::Current, position)
    }

    /// The position of a complete key, ranking each value once; `None` when
    /// the key is incomplete or a value lies outside its argument's bound.
    pub(in crate::oracle) fn position(&self, key: &AtomKey<'_>) -> Option<usize> {
        self.layout
            .index_of((0..key.predicate().arity()).filter_map(|column| key.value(column)))
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
    /// order, leaving the relation empty. Each atom is built, then its bytes
    /// admitted and recorded against the base before the next is built; a
    /// refusal leaves the atoms taken so far in `atoms`.
    pub(super) fn take_atoms(
        &mut self,
        atoms: &mut Vec<Atom>,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
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
        Ok(())
    }
}

/// The heads one round derives for the dense relations, held as bits beside
/// the catalogs the round's joins borrow.
///
/// A row of words per layout, in the layouts' order, sized once for the
/// preparation. A marked position is a tuple absent from its relation when it
/// was derived, so the marks of a round are disjoint from the relation and
/// their number is the round's count of new dense atoms. Absorbing a row
/// clears it: between rounds every row is zero and nothing of a candidate
/// remains.
///
/// Retains one bit for each position of each layout, half of what the dense
/// relations themselves hold, for as long as the workspace lives. Marking is
/// constant time. Absorbing a row visits the words from its first mark to its
/// last, at worst the whole row in every round that marks both of its ends;
/// an unmarked row costs a round one unit.
#[derive(Default)]
pub(in crate::oracle) struct PendingRows {
    rows: Vec<PendingRow>,
    marked: usize,
}

#[derive(Default)]
struct PendingRow {
    words: Vec<u64>,
    marked: usize,
    /// The words holding marks, so that absorbing visits only them.
    touched: Range<usize>,
}

impl PendingRows {
    /// Size a zero row for each layout, admitting the growth against `live`.
    /// Rows already of their layout's size are kept.
    pub(in crate::oracle) fn prepare(
        &mut self,
        layouts: &Layouts,
        live: &mut u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if self.rows.len() < layouts.len() {
            let headers = layouts.len() as u128 * size_of::<PendingRow>() as u128;
            super::storage::admit(work, live.checked_add(headers).ok_or(Stop::StorageLimit)?)?;
            let old = self.header_bytes();
            self.rows
                .try_reserve_exact(layouts.len() - self.rows.len())
                .map_err(|_| Stop::Allocation)?;
            self.rows.resize_with(layouts.len(), PendingRow::default);
            *live = live
                .checked_add(self.header_bytes() - old)
                .ok_or(Stop::StorageLimit)?;
            super::storage::after_reservation(work, *live)?;
        }
        for (row, layout) in self.rows.iter_mut().zip(layouts.iter()) {
            work.tick()?;
            let words = layout.words();
            if row.words.len() == words {
                continue;
            }
            let added = words as u128 * size_of::<u64>() as u128;
            super::storage::admit(work, live.checked_add(added).ok_or(Stop::StorageLimit)?)?;
            let old = row.bytes();
            row.words
                .try_reserve_exact(words.saturating_sub(row.words.len()))
                .map_err(|_| Stop::Allocation)?;
            charge(work, words)?;
            row.words.clear();
            row.words.resize(words, 0);
            *live = live
                .checked_sub(old)
                .and_then(|bytes| bytes.checked_add(row.bytes()))
                .ok_or(Stop::StorageLimit)?;
            super::storage::after_reservation(work, *live)?;
        }
        Ok(())
    }

    fn header_bytes(&self) -> u128 {
        self.rows.capacity() as u128 * size_of::<PendingRow>() as u128
    }

    /// Retained bytes: the row headers and their words.
    pub(in crate::oracle) fn bytes(&self) -> u128 {
        self.rows
            .iter()
            .map(PendingRow::bytes)
            .fold(self.header_bytes(), u128::saturating_add)
    }

    /// Mark a position of the layout at `slot`; whether it was unmarked. The
    /// caller has found the tuple absent from its relation.
    pub(in crate::oracle) fn mark(&mut self, slot: usize, position: usize) -> bool {
        let row = &mut self.rows[slot];
        let (word, bit) = (position / 64, 1u64 << (position % 64));
        if row.words[word] & bit != 0 {
            return false;
        }
        row.words[word] |= bit;
        row.marked += 1;
        self.marked += 1;
        row.touched = cover(&row.touched, word);
        true
    }

    /// Positions marked since the rows were last absorbed.
    pub(in crate::oracle) fn len(&self) -> usize {
        self.marked
    }

    pub(in crate::oracle) fn is_empty(&self) -> bool {
        self.marked == 0
    }

    /// Insert the marks of the row at `slot` into its relation as new rows
    /// and clear them; the number inserted. One unit, and one per word that
    /// held a mark or lies between two that did.
    pub(super) fn absorb_into(
        &mut self,
        slot: usize,
        dense: &mut Dense,
        work: &mut Work<'_>,
    ) -> Result<usize, Stop> {
        let row = &mut self.rows[slot];
        charge(work, 1 + row.touched.len())?;
        if row.marked == 0 {
            return Ok(0);
        }
        for word in row.touched.clone() {
            let marks = std::mem::take(&mut row.words[word]);
            debug_assert_eq!(dense.present[word] & marks, 0, "a mark is an absent tuple");
            dense.present[word] |= marks;
            dense.new[word] |= marks;
        }
        let absorbed = std::mem::take(&mut row.marked);
        dense.count += absorbed;
        dense.new_count += absorbed;
        dense.new_words = if dense.new_words.is_empty() {
            row.touched.clone()
        } else {
            dense.new_words.start.min(row.touched.start)..dense.new_words.end.max(row.touched.end)
        };
        row.touched = 0..0;
        self.marked -= absorbed;
        Ok(absorbed)
    }
}

/// What joining one body row into a head's pending row found: the body rows
/// offered, each one binding of the rule, and those newly marked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::oracle) struct RowJoin {
    pub(in crate::oracle) offered: usize,
    pub(in crate::oracle) marked: usize,
}

/// The rows of one set of a dense relation within a block of positions: the
/// tuples sharing a bound prefix of arguments.
#[derive(Clone, Copy)]
pub(in crate::oracle) struct Block<'a> {
    pub(in crate::oracle) relation: &'a Dense,
    pub(in crate::oracle) set: RowSet,
    pub(in crate::oracle) start: usize,
    pub(in crate::oracle) len: usize,
}

impl Block<'_> {
    /// Whether the block holds no row of its set. One unit a word examined.
    pub(in crate::oracle) fn is_empty(&self, work: &mut Work<'_>) -> Result<bool, Stop> {
        let mut range = self.start..self.start + self.len;
        Ok(self
            .relation
            .next_row(self.set, &mut range, work)?
            .is_none())
    }
}

impl PendingRows {
    /// Mark, for every tuple of the `body` block, the head position as far
    /// into the block of `head` starting at `head_start`: the step of
    /// a rule whose last body argument is its head's last argument, over the
    /// same values in both, taken a word at a time. A position the head holds
    /// or the round has marked is not marked again, so the marks are those
    /// of marking each offered position singly. One unit for each word of
    /// the block.
    ///
    /// Each pass reads the next `width` positions of the block, at most a
    /// word, so the passes partition it and the loop ends with it.
    pub(in crate::oracle) fn join_row(
        &mut self,
        slot: usize,
        head: &Dense,
        head_start: usize,
        body: Block<'_>,
        work: &mut Work<'_>,
    ) -> Result<RowJoin, Stop> {
        let row = &mut self.rows[slot];
        let mut joined = RowJoin {
            offered: 0,
            marked: 0,
        };
        let mut done = 0;
        while done < body.len {
            charge(work, 1)?;
            let width = (body.len - done).min(64);
            let offered = bits(
                |word| body.relation.word(body.set, word),
                body.start + done,
                width,
            );
            joined.offered += offered.count_ones() as usize;
            let start = head_start + done;
            let occupied = bits(|at| head.present[at] | row.words[at], start, width);
            let fresh = offered & !occupied;
            if fresh != 0 {
                let (first, shift) = (start / 64, start % 64);
                row.words[first] |= fresh << shift;
                row.touched = cover(&row.touched, first);
                if shift + width > 64 {
                    row.words[first + 1] |= fresh >> (64 - shift);
                    row.touched = cover(&row.touched, first + 1);
                }
                joined.marked += fresh.count_ones() as usize;
            }
            done += width;
        }
        row.marked += joined.marked;
        self.marked += joined.marked;
        Ok(joined)
    }
}

/// The `width` bits from position `start`, at most a word, lowest first.
fn bits(word: impl Fn(usize) -> u64, start: usize, width: usize) -> u64 {
    let (index, shift) = (start / 64, start % 64);
    let mut value = word(index) >> shift;
    if shift + width > 64 {
        value |= word(index + 1) << (64 - shift);
    }
    if width < 64 {
        value &= (1u64 << width) - 1;
    }
    value
}

impl PendingRow {
    fn bytes(&self) -> u128 {
        self.words.capacity() as u128 * size_of::<u64>() as u128
    }
}

/// The word range extended to hold `word`.
fn cover(range: &Range<usize>, word: usize) -> Range<usize> {
    if range.is_empty() {
        word..word + 1
    } else {
        range.start.min(word)..range.end.max(word + 1)
    }
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
    fn a_product_above_the_ceiling_keeps_the_tree() {
        let predicate = Predicate::new("e", 2).unwrap();
        assert!(Layout::new(&predicate, &[numbers(&[1, 2, 3]), numbers(&[10, 20])], 5).is_none());
    }

    #[test]
    fn an_unknown_argument_keeps_the_tree() {
        let predicate = Predicate::new("e", 2).unwrap();
        assert!(Layout::new(&predicate, &[numbers(&[1]), Bound::Unknown], 64).is_none());
    }

    #[test]
    fn bounds_of_another_arity_lay_out_nothing() {
        let predicate = Predicate::new("e", 2).unwrap();
        assert!(Layout::new(&predicate, &[numbers(&[1])], 64).is_none());
    }

    #[test]
    fn rows_are_scanned_in_position_order_within_a_set() {
        let control = Control::default();
        let mut work = Work::source(&control, 1_000);
        work.limits.max_closure_bytes = 1 << 20;
        let (_, mut pending) = pending(&mut work);
        let mut dense = Dense::new(Arc::new(layout())).unwrap();
        insert(&mut dense, &mut pending, &[4], &mut work);
        dense.advance(&mut work).unwrap();
        insert(&mut dense, &mut pending, &[1, 5], &mut work);
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

    fn pending(work: &mut Work<'_>) -> (Layouts, PendingRows) {
        let mut layouts = Layouts::default();
        layouts.push(layout());
        let mut pending = PendingRows::default();
        let mut live = 0;
        pending.prepare(&layouts, &mut live, work).unwrap();
        assert_eq!(live, pending.bytes());
        (layouts, pending)
    }

    /// Rows enter a dense relation only through the pending rows.
    fn insert(
        dense: &mut Dense,
        pending: &mut PendingRows,
        positions: &[usize],
        work: &mut Work<'_>,
    ) {
        for &position in positions {
            assert!(pending.mark(0, position));
        }
        assert_eq!(
            pending.absorb_into(0, dense, work).unwrap(),
            positions.len()
        );
    }

    #[test]
    fn a_position_is_marked_pending_once() {
        let control = Control::default();
        let mut work = Work::source(&control, 1_000);
        work.limits.max_closure_bytes = 1 << 20;
        let (_, mut pending) = pending(&mut work);
        assert!(pending.is_empty());
        assert!(pending.mark(0, 4));
        assert!(!pending.mark(0, 4));
        assert!(pending.mark(0, 1));
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn absorbing_pending_rows_inserts_exactly_the_marked_positions() {
        let control = Control::default();
        let mut work = Work::source(&control, 1_000);
        work.limits.max_closure_bytes = 1 << 20;
        let (layouts, mut pending) = pending(&mut work);
        let mut dense = Dense::new(layouts.iter().next().unwrap().clone()).unwrap();
        insert(&mut dense, &mut pending, &[4], &mut work);
        dense.advance(&mut work).unwrap();
        for position in [5, 1] {
            pending.mark(0, position);
        }
        assert_eq!(pending.absorb_into(0, &mut dense, &mut work).unwrap(), 2);
        assert_eq!(dense.len(), 3);
        let scan = |set: RowSet, work: &mut Work<'_>| {
            let mut range = 0..6;
            let mut found = Vec::new();
            while let Some(position) = dense.next_row(set, &mut range, work).unwrap() {
                found.push(position);
            }
            found
        };
        assert_eq!(scan(RowSet::Current, &mut work), vec![1, 4, 5]);
        assert_eq!(scan(RowSet::New, &mut work), vec![1, 5]);
        assert_eq!(scan(RowSet::Old, &mut work), vec![4]);
    }

    #[test]
    fn absorbed_pending_rows_are_empty_for_the_next_round() {
        let control = Control::default();
        let mut work = Work::source(&control, 1_000);
        work.limits.max_closure_bytes = 1 << 20;
        let (layouts, mut pending) = pending(&mut work);
        let mut dense = Dense::new(layouts.iter().next().unwrap().clone()).unwrap();
        pending.mark(0, 3);
        pending.absorb_into(0, &mut dense, &mut work).unwrap();
        assert!(pending.is_empty());
        assert_eq!(pending.absorb_into(0, &mut dense, &mut work).unwrap(), 0);
        // The cleared bit can be marked again: nothing of the round remains.
        assert!(pending.mark(0, 3));
    }

    /// A body of three values by seventy and a head of two by seventy: rows
    /// wider than a word, starting off a word boundary.
    fn wide() -> (Layouts, PendingRows, Dense, Dense) {
        let control = Control::default();
        let mut work = Work::source(&control, 100_000);
        work.limits.max_closure_bytes = 1 << 20;
        let last: Vec<i32> = (0..70).collect();
        let mut layouts = Layouts::default();
        for (name, first) in [("body", &[1, 2, 3][..]), ("head", &[7, 8][..])] {
            layouts.push(
                Layout::new(
                    &Predicate::new(name, 2).unwrap(),
                    &[numbers(first), numbers(&last)],
                    1 << 10,
                )
                .unwrap(),
            );
        }
        let mut pending = PendingRows::default();
        pending.prepare(&layouts, &mut 0, &mut work).unwrap();
        let mut relations: Vec<Dense> = layouts
            .iter()
            .map(|layout| Dense::new(layout.clone()).unwrap())
            .collect();
        let head = relations.pop().unwrap();
        let body = relations.pop().unwrap();
        (layouts, pending, body, head)
    }

    fn block(relation: &Dense, set: RowSet, start: usize) -> Block<'_> {
        Block {
            relation,
            set,
            start,
            len: 70,
        }
    }

    #[test]
    fn joining_a_row_marks_what_marking_each_of_its_positions_would() {
        let control = Control::default();
        let mut work = Work::source(&control, 100_000);
        let (_, mut pending, mut body, mut head) = wide();
        // The body's second row, positions 70..140, holds these last values.
        let values = [0, 1, 5, 58, 63, 64, 69];
        for value in values {
            assert!(pending.mark(0, 70 + value));
        }
        pending.absorb_into(0, &mut body, &mut work).unwrap();
        // The head's second row, positions 70..140, already holds 5 and has
        // 64 pending: neither is marked again.
        assert!(pending.mark(1, 70 + 5));
        pending.absorb_into(1, &mut head, &mut work).unwrap();
        assert!(pending.mark(1, 70 + 64));
        let joined = pending
            .join_row(1, &head, 70, block(&body, RowSet::Current, 70), &mut work)
            .unwrap();
        assert_eq!(
            joined,
            RowJoin {
                offered: 7,
                marked: 5
            }
        );
        assert_eq!(pending.len(), 6);
        assert_eq!(pending.absorb_into(1, &mut head, &mut work).unwrap(), 6);
        let mut range = 0..140;
        let mut found = Vec::new();
        while let Some(position) = head
            .next_row(RowSet::Current, &mut range, &mut work)
            .unwrap()
        {
            found.push(position);
        }
        assert_eq!(found, values.map(|value| 70 + value));
    }

    #[test]
    fn joining_a_row_reads_only_the_selected_rows() {
        let control = Control::default();
        let mut work = Work::source(&control, 100_000);
        let (_, mut pending, mut body, head) = wide();
        assert!(pending.mark(0, 3));
        pending.absorb_into(0, &mut body, &mut work).unwrap();
        body.advance(&mut work).unwrap();
        assert!(pending.mark(0, 66));
        pending.absorb_into(0, &mut body, &mut work).unwrap();
        for (set, offered) in [(RowSet::Old, 1), (RowSet::New, 1), (RowSet::Current, 2)] {
            let (_, mut pending, _, _) = wide();
            let joined = pending
                .join_row(1, &head, 0, block(&body, set, 0), &mut work)
                .unwrap();
            assert_eq!(
                joined,
                RowJoin {
                    offered,
                    marked: offered
                }
            );
        }
    }

    /// The relation holding positions 5, 0 and 3 with its atoms taken.
    fn taken(work: &mut Work<'_>) -> (Dense, Vec<Atom>) {
        work.limits.max_closure_bytes = 1 << 20;
        let (_, mut pending) = pending(work);
        let mut dense = Dense::new(Arc::new(layout())).unwrap();
        insert(&mut dense, &mut pending, &[5, 0, 3], work);
        let mut atoms = Vec::new();
        dense.take_atoms(&mut atoms, 0, work).unwrap();
        (dense, atoms)
    }

    #[test]
    fn taken_atoms_are_in_canonical_order() {
        let control = Control::default();
        let mut work = Work::source(&control, 10_000);
        let (_, atoms) = taken(&mut work);
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
    }

    #[test]
    fn taking_the_atoms_empties_the_relation() {
        let control = Control::default();
        let mut work = Work::source(&control, 10_000);
        let (dense, _) = taken(&mut work);
        assert_eq!(dense.len(), 0);
        assert!(!dense.contains(5));
    }
}
