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
//! window over them is a scan of that range's words. Bound values are resolved
//! through checked canonical comparisons; rows themselves retain only bits.
//!
//! The New set of a round is a second bit array, cleared when the cutoff
//! advances; Old is present and not new. The bounds are an upper domain of
//! every derivable head, so a head outside them cannot arise; one that does
//! is an admitted-program invariant violation, not a missed row.

use std::{mem::size_of, ops::Range, sync::Arc};

use zetesis_core::catalog::{PredicateRef, TermRef};
#[cfg(test)]
use zetesis_core::{Atom, Predicate, Value};
use zetesis_core::{AtomKey, Program};

use super::super::argument_bounds::Bound;
use super::{RowSet, Work, charge};
use crate::Stop;

/// The shape of a dense relation: the predicate, each argument's values in
/// canonical order, and the mixed-radix strides over them.
pub(in crate::oracle) struct Layout {
    program: Program,
    signature: usize,
    axes: Vec<Vec<usize>>,
    /// `strides[k]` is the product of the widths of the arguments after `k`.
    strides: Vec<usize>,
    positions: usize,
}

impl Layout {
    /// Keep only domain coordinates from the Program's finite argument bounds.
    /// Unknown/oversized domains retain the tree route; malformed coordinates
    /// and allocation failures are typed refusals, never an empty relation.
    pub(in crate::oracle) fn new(
        program: &Program,
        predicate: PredicateRef<'_>,
        bounds: &[Bound],
        ceiling: usize,
    ) -> Result<Option<Self>, Stop> {
        if bounds.len() != predicate.arity() {
            return Err(Stop::InvalidProgram);
        }
        let signature = program
            .predicates()
            .binary_search(predicate)
            .map_err(|_| Stop::InvalidProgram)?;
        let mut positions = 1_usize;
        for bound in bounds {
            let Bound::Finite(values) = bound else {
                return Ok(None);
            };
            if values
                .last()
                .is_some_and(|&id| id >= program.domain().len())
                || !values.windows(2).all(|pair| pair[0] < pair[1])
            {
                return Err(Stop::InvalidProgram);
            }
            let Some(product) = positions.checked_mul(values.len()) else {
                return Ok(None);
            };
            positions = product;
        }
        if positions > ceiling {
            return Ok(None);
        }
        let mut axes = Vec::new();
        axes.try_reserve_exact(bounds.len())
            .map_err(|_| Stop::Allocation)?;
        let mut strides = Vec::new();
        strides
            .try_reserve_exact(bounds.len())
            .map_err(|_| Stop::Allocation)?;
        for bound in bounds {
            let Bound::Finite(values) = bound else {
                unreachable!("finite bounds checked");
            };
            let mut coordinates = Vec::new();
            coordinates
                .try_reserve_exact(values.len())
                .map_err(|_| Stop::Allocation)?;
            coordinates.extend_from_slice(values);
            axes.push(coordinates);
            strides.push(1);
        }
        let mut suffix = 1_usize;
        for (index, coordinates) in axes.iter().enumerate().rev() {
            strides[index] = suffix;
            suffix = suffix
                .checked_mul(coordinates.len())
                .ok_or(Stop::InvalidProgram)?;
        }
        Ok(Some(Self {
            program: program.clone(),
            signature,
            axes,
            strides,
            positions,
        }))
    }

    pub(in crate::oracle) fn predicate(&self) -> PredicateRef<'_> {
        self.program
            .predicates()
            .at(self.signature)
            .expect("admitted dense signature")
    }
    pub(super) fn program(&self) -> &Program {
        &self.program
    }
    pub(super) fn signature(&self) -> usize {
        self.signature
    }
    pub(in crate::oracle) fn positions(&self) -> usize {
        self.positions
    }
    fn words(&self) -> usize {
        self.positions.div_ceil(64)
    }

    /// Named coordinate/stride metadata; the shared Program is charged once by
    /// preparation/authority ownership, not once per dense axis occurrence.
    pub(in crate::oracle) fn bytes(&self) -> Option<u128> {
        let cells = self.axes.iter().try_fold(0_u128, |bytes, axis| {
            bytes.checked_add(axis.capacity() as u128 * size_of::<usize>() as u128)
        })?;
        (size_of::<Self>() as u128)
            .checked_add(self.axes.capacity() as u128 * size_of::<Vec<usize>>() as u128)?
            .checked_add(cells)?
            .checked_add(self.strides.capacity() as u128 * size_of::<usize>() as u128)
    }

    fn rank(
        &self,
        argument: usize,
        value: TermRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        let Some(axis) = self.axes.get(argument) else {
            return Ok(None);
        };
        let (mut start, mut end) = (0, axis.len());
        while start < end {
            charge(work, 1)?;
            let middle = start + (end - start) / 2;
            let candidate = self
                .program
                .domain()
                .at(axis[middle])
                .ok_or(Stop::InvalidProgram)?;
            match candidate.compare_ref_with(value, || charge(work, 1))? {
                std::cmp::Ordering::Equal => return Ok(Some(middle)),
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
            }
        }
        Ok(None)
    }

    pub(in crate::oracle) fn position_of<'v, T: Into<TermRef<'v>>>(
        &self,
        values: impl IntoIterator<Item = T>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        let mut position = 0;
        let mut arguments = 0;
        for (argument, value) in values.into_iter().enumerate() {
            let Some(rank) = self.rank(argument, value.into(), work)? else {
                return Ok(None);
            };
            position += rank * self.strides[argument];
            arguments += 1;
        }
        Ok((arguments == self.axes.len()).then_some(position))
    }

    /// These ranks compare directly only for layouts of the same Program.
    pub(in crate::oracle) fn last_coordinates(&self) -> Option<&[usize]> {
        self.axes.last().map(Vec::as_slice)
    }

    pub(super) fn coordinate(&self, argument: usize, position: usize) -> usize {
        let axis = &self.axes[argument];
        axis[(position / self.strides[argument]) % axis.len()]
    }

    pub(in crate::oracle) fn value(&self, argument: usize, position: usize) -> TermRef<'_> {
        self.program
            .domain()
            .at(self.coordinate(argument, position))
            .expect("admitted dense axis")
    }

    pub(in crate::oracle) fn prefix_range<'v, T: Into<TermRef<'v>>>(
        &self,
        prefix: impl IntoIterator<Item = T>,
        work: &mut Work<'_>,
    ) -> Result<Range<usize>, Stop> {
        let mut base = 0;
        let mut block = self.positions;
        for (argument, value) in prefix.into_iter().enumerate() {
            let Some(rank) = self.rank(argument, value.into(), work)? else {
                return Ok(0..0);
            };
            block = self.strides[argument];
            base += rank * block;
        }
        Ok(base..base + block)
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

    pub(in crate::oracle) fn get(&self, predicate: PredicateRef<'_>) -> Option<&Arc<Layout>> {
        self.slot(predicate).map(|slot| &self.0[slot])
    }

    /// The layout's position among the layouts, which indexes the pending
    /// rows and is fixed for the preparation.
    pub(in crate::oracle) fn slot(&self, predicate: PredicateRef<'_>) -> Option<usize> {
        self.0
            .binary_search_by(|layout| layout.predicate().cmp(&predicate))
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
    /// Discovery positions of rows whose identity the workspace's authority
    /// had already admitted when they were derived again, as (position,
    /// discovery position) pairs ascending by position. Identity metadata,
    /// not truth: a reset keeps it, and the authority's discovery positions
    /// never change, so a pair stays exact for as long as the workspace that
    /// owns the authority, which owns this relation.
    discovered: Vec<(usize, usize)>,
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
            discovered: Vec::new(),
        })
    }

    /// The bytes the two word vectors hold, for a layout of this shape.
    pub(super) fn word_bytes(layout: &Layout) -> u128 {
        2 * layout.words() as u128 * size_of::<u64>() as u128
    }

    pub(in crate::oracle) fn layout(&self) -> &Layout {
        &self.layout
    }

    pub(super) fn predicate(&self) -> PredicateRef<'_> {
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
        position < self.layout.positions
            && self.word(set, position / 64) >> (position % 64) & 1 == 1
    }

    pub(in crate::oracle) fn contains(&self, position: usize) -> bool {
        self.holds(RowSet::Current, position)
    }

    /// The position of a complete key, ranking each value once; `None` when
    /// the key is incomplete or a value lies outside its argument's bound.
    pub(in crate::oracle) fn position(
        &self,
        key: &AtomKey<'_>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        self.layout.position_of(
            (0..key.predicate().arity()).filter_map(|column| key.value(column)),
            work,
        )
    }

    /// Move the cutoff to the present extent: nothing is new.
    pub(super) fn advance(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 1 + self.new_words.len())?;
        self.new[self.new_words.clone()].fill(0);
        self.new_words = 0..0;
        self.new_count = 0;
        Ok(())
    }

    /// Empty the relation, keeping its capacity and its discovered positions.
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
        let end = range.end.min(self.layout.positions);
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

    /// Discovered positions of current rows, ascending by row position.
    pub(super) fn discovered(&self) -> &[(usize, usize)] {
        &self.discovered
    }

    /// Record the discovery positions of current rows whose identity existed
    /// before this pass. `ids` holds the discovery position of each current
    /// row in row order, and `admitted` the authority's size when the pass
    /// began, below which a discovery position names an older identity.
    /// Recording only saves later lookups, so it never refuses a closure: it
    /// is skipped when its work would exceed the remaining work, when its
    /// bytes would take the closure past its ceiling (`live` being the bytes
    /// already held), or when it cannot be allocated.
    ///
    /// # Errors
    /// Returns a cancellation stop observed while charging the scan or the
    /// merge. The guard admits recording only when their work fits the
    /// remaining work, so no work limit is reached here.
    pub(super) fn remember(
        &mut self,
        ids: &[usize],
        admitted: usize,
        live: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let repeats = ids.iter().filter(|&&id| id < admitted).count();
        if repeats == 0 {
            return Ok(());
        }
        // The scan visits each word and row once; the merge, each pair.
        let cost = self.layout.words() + ids.len() + self.discovered.len() + repeats;
        let remaining = work.limits.max_work.saturating_sub(work.statistics.work);
        let size = size_of::<(usize, usize)>() as u128;
        let bytes = (2 * repeats + self.discovered.len()) as u128 * size;
        if u64::try_from(cost).map_or(true, |cost| cost > remaining)
            || live.saturating_add(bytes) > work.limits.max_closure_bytes as u128
        {
            return Ok(());
        }
        let mut found = Vec::new();
        let mut merged = Vec::new();
        if found.try_reserve_exact(repeats).is_err()
            || merged
                .try_reserve_exact(self.discovered.len() + repeats)
                .is_err()
        {
            return Ok(());
        }
        let mut range = 0..self.layout.positions;
        for &id in ids {
            let position = self
                .next_row(RowSet::Current, &mut range, work)?
                .ok_or(Stop::InvalidProgram)?;
            if id < admitted {
                found.push((position, id));
            }
        }
        // A row already recorded meets its own record here: identities are
        // stable, so the pair is the same and is kept once.
        charge(work, self.discovered.len() + found.len())?;
        let (mut old, mut new) = (self.discovered.iter().peekable(), found.iter().peekable());
        while let (Some(&&left), Some(&&right)) = (old.peek(), new.peek()) {
            match left.0.cmp(&right.0) {
                std::cmp::Ordering::Less => {
                    merged.push(left);
                    old.next();
                }
                std::cmp::Ordering::Greater => {
                    merged.push(right);
                    new.next();
                }
                std::cmp::Ordering::Equal => {
                    debug_assert_eq!(left, right);
                    merged.push(left);
                    old.next();
                    new.next();
                }
            }
        }
        merged.extend(old.copied());
        merged.extend(new.copied());
        super::storage::record(
            work,
            live + (found.capacity() + merged.capacity()) as u128 * size,
        )?;
        self.discovered = merged;
        Ok(())
    }

    /// Actual retained bit-vector capacity and discovered positions; layout
    /// metadata is shared preparation.
    pub(super) fn retained_bytes(&self) -> u128 {
        (self.present.capacity() as u128 + self.new.capacity() as u128) * size_of::<u64>() as u128
            + self.discovered.capacity() as u128 * size_of::<(usize, usize)>() as u128
    }
}

/// The heads one round derives for the dense relations, held as bits beside
/// the catalogs the round's joins borrow.
///
/// The marks of each layout, in the layouts' order, sized once for the
/// preparation. A marked position is a tuple absent from its relation when it
/// was derived, so the marks of a round are disjoint from the relation and
/// their number is the round's count of new dense atoms. Absorbing a layout's
/// marks clears them: between rounds every mark is zero and nothing of a
/// candidate remains.
///
/// Retains one bit for each position of each layout, half of what the dense
/// relations themselves hold, for as long as the workspace lives. Marking is
/// constant time. Absorbing a layout's marks visits the words from its first
/// mark to its last, at worst the whole array in every round that marks both
/// of its ends; unmarked marks cost a round one unit.
#[derive(Default)]
pub(in crate::oracle) struct PendingMarks {
    layouts: Vec<LayoutMarks>,
    marked: usize,
}

/// One layout's marks: a word for every sixty-four positions.
#[derive(Default)]
struct LayoutMarks {
    words: Vec<u64>,
    marked: usize,
    /// The words holding marks, so that absorbing visits only them.
    touched: Range<usize>,
}

impl PendingMarks {
    /// Size zero marks for each layout, admitting the growth against `live`.
    /// Marks already of their layout's size are kept.
    pub(in crate::oracle) fn prepare(
        &mut self,
        layouts: &Layouts,
        live: &mut u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if self.layouts.len() < layouts.len() {
            let headers = layouts.len() as u128 * size_of::<LayoutMarks>() as u128;
            super::storage::admit(work, live.checked_add(headers).ok_or(Stop::StorageLimit)?)?;
            let old = self.header_bytes();
            self.layouts
                .try_reserve_exact(layouts.len() - self.layouts.len())
                .map_err(|_| Stop::Allocation)?;
            self.layouts
                .resize_with(layouts.len(), LayoutMarks::default);
            *live = live
                .checked_add(self.header_bytes() - old)
                .ok_or(Stop::StorageLimit)?;
            super::storage::after_reservation(work, *live)?;
        }
        for (marks, layout) in self.layouts.iter_mut().zip(layouts.iter()) {
            work.tick()?;
            let words = layout.words();
            if marks.words.len() == words {
                continue;
            }
            let added = words as u128 * size_of::<u64>() as u128;
            super::storage::admit(work, live.checked_add(added).ok_or(Stop::StorageLimit)?)?;
            let old = marks.bytes();
            marks
                .words
                .try_reserve_exact(words.saturating_sub(marks.words.len()))
                .map_err(|_| Stop::Allocation)?;
            charge(work, words)?;
            marks.words.clear();
            marks.words.resize(words, 0);
            *live = live
                .checked_sub(old)
                .and_then(|bytes| bytes.checked_add(marks.bytes()))
                .ok_or(Stop::StorageLimit)?;
            super::storage::after_reservation(work, *live)?;
        }
        Ok(())
    }

    fn header_bytes(&self) -> u128 {
        self.layouts.capacity() as u128 * size_of::<LayoutMarks>() as u128
    }

    /// Retained bytes: the marks' headers and their words.
    pub(in crate::oracle) fn bytes(&self) -> u128 {
        self.layouts
            .iter()
            .map(LayoutMarks::bytes)
            .fold(self.header_bytes(), u128::saturating_add)
    }

    /// Mark a position of the layout at `slot`; whether it was unmarked. The
    /// caller has found the tuple absent from its relation.
    pub(in crate::oracle) fn mark(&mut self, slot: usize, position: usize) -> bool {
        let marks = &mut self.layouts[slot];
        let (word, bit) = (position / 64, 1u64 << (position % 64));
        if marks.words[word] & bit != 0 {
            return false;
        }
        marks.words[word] |= bit;
        marks.marked += 1;
        self.marked += 1;
        marks.touched = cover(&marks.touched, word);
        true
    }

    /// Positions marked since the marks were last absorbed.
    pub(in crate::oracle) fn len(&self) -> usize {
        self.marked
    }

    pub(in crate::oracle) fn is_empty(&self) -> bool {
        self.marked == 0
    }

    /// Insert the marks of the layout at `slot` into its relation as new rows
    /// and clear them; the number inserted. One unit, and one per word that
    /// held a mark or lies between two that did.
    pub(super) fn absorb_into(
        &mut self,
        slot: usize,
        dense: &mut Dense,
        work: &mut Work<'_>,
    ) -> Result<usize, Stop> {
        let marks = &mut self.layouts[slot];
        charge(work, 1 + marks.touched.len())?;
        if marks.marked == 0 {
            return Ok(0);
        }
        for word in marks.touched.clone() {
            let marks = std::mem::take(&mut marks.words[word]);
            debug_assert_eq!(dense.present[word] & marks, 0, "a mark is an absent tuple");
            dense.present[word] |= marks;
            dense.new[word] |= marks;
        }
        let absorbed = std::mem::take(&mut marks.marked);
        dense.count += absorbed;
        dense.new_count += absorbed;
        dense.new_words = if dense.new_words.is_empty() {
            marks.touched.clone()
        } else {
            dense.new_words.start.min(marks.touched.start)
                ..dense.new_words.end.max(marks.touched.end)
        };
        marks.touched = 0..0;
        self.marked -= absorbed;
        Ok(absorbed)
    }
}

/// What joining one body row into a head's pending marks found: the body rows
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

impl PendingMarks {
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
        let marks = &mut self.layouts[slot];
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
            let occupied = bits(|at| head.present[at] | marks.words[at], start, width);
            let fresh = offered & !occupied;
            if fresh != 0 {
                let (first, shift) = (start / 64, start % 64);
                marks.words[first] |= fresh << shift;
                marks.touched = cover(&marks.touched, first);
                if shift + width > 64 {
                    marks.words[first + 1] |= fresh >> (64 - shift);
                    marks.touched = cover(&marks.touched, first + 1);
                }
                joined.marked += fresh.count_ones() as usize;
            }
            done += width;
        }
        marks.marked += joined.marked;
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

impl LayoutMarks {
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
    use crate::Cancellation;

    // Admit one row per axis value, not the Cartesian product. These are
    // source descriptions solely for establishing the test Program vocabulary.
    fn source(name: &str, axes: &[&[i32]]) -> Vec<Atom> {
        let predicate = Predicate::new(name, axes.len()).unwrap();
        let base: Vec<_> = axes.iter().map(|axis| Value::Number(axis[0])).collect();
        axes.iter()
            .enumerate()
            .flat_map(|(argument, axis)| {
                let predicate = &predicate;
                let base = &base;
                axis.iter().map(move |&value| {
                    let mut values = base.clone();
                    values[argument] = Value::Number(value);
                    Atom::new(predicate.clone(), values).unwrap()
                })
            })
            .collect()
    }

    fn numbers(program: &Program, values: &[i32]) -> Bound {
        Bound::Finite(
            values
                .iter()
                .map(|&value| {
                    program
                        .domain()
                        .iter()
                        .position(|candidate| candidate == Value::Number(value))
                        .unwrap()
                })
                .collect(),
        )
    }

    fn layout() -> Layout {
        let program = super::super::fixtures::program(&source("e", &[&[1, 2, 3], &[10, 20]]));
        Layout::new(
            &program,
            (&Predicate::new("e", 2).unwrap()).into(),
            &[numbers(&program, &[1, 2, 3]), numbers(&program, &[10, 20])],
            64,
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn positions_are_mixed_radix_with_the_first_argument_most_significant() {
        let layout = layout();
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        assert_eq!(layout.positions(), 6);
        let mut index = |a: i32, b: i32| {
            layout
                .position_of([&Value::Number(a), &Value::Number(b)], &mut work)
                .unwrap()
        };
        assert_eq!(index(1, 10), Some(0));
        assert_eq!(index(1, 20), Some(1));
        assert_eq!(index(3, 20), Some(5));
        assert_eq!(index(4, 10), None);
        assert_eq!(layout.value(0, 5), Value::Number(3));
        assert_eq!(layout.value(1, 4), Value::Number(10));
    }

    #[test]
    fn a_bound_prefix_is_one_contiguous_range() {
        let layout = layout();
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        assert_eq!(
            layout.prefix_range([] as [&Value; 0], &mut work).unwrap(),
            0..6
        );
        assert_eq!(
            layout.prefix_range([&Value::Number(2)], &mut work).unwrap(),
            2..4
        );
        assert_eq!(
            layout
                .prefix_range([&Value::Number(2), &Value::Number(20)], &mut work)
                .unwrap(),
            3..4
        );
        assert_eq!(
            layout.prefix_range([&Value::Number(9)], &mut work).unwrap(),
            0..0
        );
    }

    #[test]
    fn a_product_above_the_ceiling_keeps_the_tree() {
        let program = super::super::fixtures::program(&source("e", &[&[1, 2, 3], &[10, 20]]));
        let predicate = Predicate::new("e", 2).unwrap();
        assert!(
            Layout::new(
                &program,
                (&predicate).into(),
                &[numbers(&program, &[1, 2, 3]), numbers(&program, &[10, 20])],
                5
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn an_unknown_argument_keeps_the_tree() {
        let program = super::super::fixtures::program(&source("e", &[&[1], &[1]]));
        let predicate = Predicate::new("e", 2).unwrap();
        assert!(
            Layout::new(
                &program,
                (&predicate).into(),
                &[numbers(&program, &[1]), Bound::Unknown],
                64
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn bounds_of_another_arity_are_refused() {
        let program = super::super::fixtures::program(&source("e", &[&[1], &[1]]));
        let predicate = Predicate::new("e", 2).unwrap();
        assert!(matches!(
            Layout::new(
                &program,
                (&predicate).into(),
                &[numbers(&program, &[1])],
                64
            ),
            Err(Stop::InvalidProgram)
        ));
    }

    #[test]
    fn rows_are_scanned_in_position_order_within_a_set() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 1_000);
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

    fn pending(work: &mut Work<'_>) -> (Layouts, PendingMarks) {
        let mut layouts = Layouts::default();
        layouts.push(layout());
        let mut pending = PendingMarks::default();
        let mut live = 0;
        pending.prepare(&layouts, &mut live, work).unwrap();
        assert_eq!(live, pending.bytes());
        (layouts, pending)
    }

    /// Rows enter a dense relation only through the pending marks.
    fn insert(
        dense: &mut Dense,
        pending: &mut PendingMarks,
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

    /// A dense relation over `layout()` holding the rows at `positions`.
    fn rows(positions: &[usize], work: &mut Work<'_>) -> Dense {
        let (layouts, mut pending) = pending(work);
        let mut dense = Dense::new(layouts.iter().next().unwrap().clone()).unwrap();
        insert(&mut dense, &mut pending, positions, work);
        dense
    }

    #[test]
    fn remembering_records_older_identities_in_position_order() {
        // Rows 1, 4 and 5 hold discovery positions 7, 2 and 9. With three
        // identities admitted before the first pass, only row 4's is older;
        // a later pass, after ten, records rows 1 and 5 around it.
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 1_000);
        work.limits.max_closure_bytes = 1 << 20;
        let mut dense = rows(&[1, 4, 5], &mut work);
        dense.remember(&[7, 2, 9], 3, 0, &mut work).unwrap();
        assert_eq!(dense.discovered(), [(4, 2)]);
        dense.remember(&[7, 2, 9], 10, 0, &mut work).unwrap();
        assert_eq!(dense.discovered(), [(1, 7), (4, 2), (5, 9)]);
    }

    #[test]
    fn remembering_charges_every_pair_it_merges() {
        // After row 4 is recorded, a pass over rows 1, 4 and 5, all older
        // identities, scans three rows and merges the one record with the
        // three repeated pairs, row 4's own included: seven units.
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 1_000);
        work.limits.max_closure_bytes = 1 << 20;
        let mut dense = rows(&[1, 4, 5], &mut work);
        dense.remember(&[7, 2, 9], 3, 0, &mut work).unwrap();
        let spent = work.statistics.work;
        dense.remember(&[7, 2, 9], 10, 0, &mut work).unwrap();
        assert_eq!(work.statistics.work - spent, 7);
    }

    #[test]
    fn remembering_past_the_byte_ceiling_records_nothing() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 1_000);
        work.limits.max_closure_bytes = 1 << 20;
        let mut dense = rows(&[1, 4, 5], &mut work);
        dense.remember(&[0, 1, 2], 3, 1 << 20, &mut work).unwrap();
        assert!(dense.discovered().is_empty());
    }

    #[test]
    fn remembering_past_the_remaining_work_records_and_charges_nothing() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 1_000);
        work.limits.max_closure_bytes = 1 << 20;
        let mut dense = rows(&[1, 4, 5], &mut work);
        work.limits.max_work = work.statistics.work + 1;
        let spent = work.statistics.work;
        dense.remember(&[0, 1, 2], 3, 0, &mut work).unwrap();
        assert!(dense.discovered().is_empty());
        assert_eq!(work.statistics.work, spent);
    }

    #[test]
    fn a_position_is_marked_pending_once() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 1_000);
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
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 1_000);
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
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 1_000);
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
    fn wide() -> (Layouts, PendingMarks, Dense, Dense) {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 100_000);
        work.limits.max_closure_bytes = 1 << 20;
        let last: Vec<i32> = (0..70).collect();
        let mut sources = source("body", &[&[1, 2, 3], &last]);
        sources.extend(source("head", &[&[7, 8], &last]));
        let program = super::super::fixtures::program(&sources);
        let mut layouts = Layouts::default();
        for (name, first) in [("body", &[1, 2, 3][..]), ("head", &[7, 8][..])] {
            layouts.push(
                Layout::new(
                    &program,
                    (&Predicate::new(name, 2).unwrap()).into(),
                    &[numbers(&program, first), numbers(&program, &last)],
                    1 << 10,
                )
                .unwrap()
                .unwrap(),
            );
        }
        let mut pending = PendingMarks::default();
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
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 100_000);
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
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 100_000);
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

    fn populated(work: &mut Work<'_>) -> Dense {
        work.limits.max_closure_bytes = 1 << 20;
        let (_, mut pending) = pending(work);
        let mut dense = Dense::new(Arc::new(layout())).unwrap();
        insert(&mut dense, &mut pending, &[5, 0, 3], work);
        dense
    }

    #[test]
    fn selected_positions_decode_in_canonical_order() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 10_000);
        let dense = populated(&mut work);
        let mut range = 0..dense.layout().positions();
        let mut values = Vec::new();
        while let Some(position) = dense
            .next_row(RowSet::Current, &mut range, &mut work)
            .unwrap()
        {
            let row: Vec<_> = (0..2)
                .map(
                    |argument| match dense.layout().value(argument, position).descriptor() {
                        zetesis_core::ValueNodeRef::Number(number) => number,
                        _ => unreachable!(),
                    },
                )
                .collect();
            values.push(row);
        }
        assert_eq!(values, vec![vec![1, 10], vec![2, 20], vec![3, 20]]);
    }

    #[test]
    fn reset_clears_truth_and_keeps_the_layout() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, 10_000);
        let mut dense = populated(&mut work);
        let layout = dense.layout.clone();
        dense.reset(&mut work).unwrap();
        assert_eq!(dense.len(), 0);
        assert!(!dense.contains(5));
        assert!(Arc::ptr_eq(&layout, &dense.layout));
    }
}
