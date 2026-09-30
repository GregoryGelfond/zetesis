//! A checked semantic order over exact immutable catalog occurrences.

use std::sync::Arc;

use super::{
    AtomCatalog, Model, ModelError, ModelFailure, ModelPublication, ModelPublicationFailure,
    Selected,
};

const VECTOR_BYTES: u128 = size_of::<Vec<usize>>() as u128;
const MARK_BITS: usize = usize::BITS as usize;

/// Immutable semantic equivalence ranks for one exact borrowed occurrence map.
///
/// Equal logical atoms have equal ranks; rank ordering agrees with the existing
/// typed atom storage ordering. Original positions and canonical IDs are never
/// reinterpreted as semantic order. This value owns only integer metadata and
/// borrows its catalog, not a writer or another atom authority. A selected [`Model`]
/// shares that catalog and can outlive this preparation.
///
/// Preparation uses O(n log n) checked semantic comparisons and retains two
/// n-cell integer buffers: each occurrence's rank, and the occurrences in
/// semantic order (equal ranks by ascending position). Selection marks the m
/// supplied positions in an n-bit mask, then either walks the semantic order
/// once (O(n) integer steps) or sorts the marked positions (O(m log m) integer
/// comparisons), whichever the counts make cheaper; both publish the same model
/// and neither compares atom payloads again. Structural comparison includes
/// segment resolution and inspected term/text contents.
#[derive(Debug)]
pub struct ModelOrder<'catalog> {
    catalog: &'catalog AtomCatalog,
    ranks: Vec<usize>,
    semantic: Vec<usize>,
    preparation_peak_bytes: u128,
}

impl<'catalog> ModelOrder<'catalog> {
    /// Prepare semantic ranks without changing or copying the catalog payload.
    /// Duplicate logical occurrences are admitted and share an equivalence rank.
    ///
    /// `max_bytes` covers this order's header and actual integer capacities plus
    /// live temporary vector headers and old/new reservation overlap. Catalog
    /// payload, caller inputs, scalar/callback/receipt frames, allocator metadata
    /// and Arc counters are excluded. Actual peaks survive refusal; rejected
    /// proposals are not observed allocations. Initialized named headers count
    /// even on initial refusal; no integer capacity exists then. Zero is a real
    /// allowance.
    ///
    /// `before` precedes reservations, occurrence resolution, checked semantic
    /// comparisons, integer writes and publication. It supplies cumulative work
    /// and cancellation policy. The first callback precedes any allocation.
    ///
    /// # Errors
    /// Returns a typed metadata-capacity, allocation or caller refusal and the
    /// actual named peak. No partial order is returned; the catalog is unchanged.
    ///
    /// # Panics
    /// Panics if an immutable catalog violates its admitted occurrence extent.
    pub fn prepare_with<E>(
        catalog: &'catalog AtomCatalog,
        max_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, ModelPublicationFailure<E>> {
        let mut order = Self {
            catalog,
            ranks: Vec::new(),
            semantic: Vec::new(),
            preparation_peak_bytes: 0,
        };
        let mut scratch = Vec::new();
        let base = size_of::<Self>() as u128 + VECTOR_BYTES;
        let mut account = Account::new(max_bytes, base);
        let result = (|| {
            let mut checked = || before().map_err(ModelFailure::Stopped);
            checked()?;
            account.admit(base)?;
            let count = catalog.atoms().len();
            reserve(&mut order.ranks, count, base, &mut account, &mut checked)?;
            reserve(
                &mut scratch,
                count,
                base + cells(order.ranks.capacity()),
                &mut account,
                &mut checked,
            )?;
            for position in 0..count {
                checked()?;
                order.ranks.push(position);
                checked()?;
                scratch.push(0);
            }
            let atoms = catalog.atoms();
            crate::checked_sort::sort(
                &mut order.ranks,
                &mut scratch,
                |values, left, right, permit| {
                    permit()?;
                    let left = atoms
                        .at(values[left])
                        .expect("prepared occurrence is in the catalog");
                    permit()?;
                    let right = atoms
                        .at(values[right])
                        .expect("prepared occurrence is in the catalog");
                    left.compare_ref_with(right, permit)
                },
                &mut checked,
            )?;
            let mut previous = None;
            let mut rank = 0;
            for &position in &order.ranks {
                checked()?;
                let atom = atoms
                    .at(position)
                    .expect("sorted occurrence remains in the catalog");
                if let Some(previous) = previous {
                    checked()?;
                    let previous = atoms
                        .at(previous)
                        .expect("previous occurrence remains in the catalog");
                    if !atom.equals_ref_with(previous, &mut checked)? {
                        // There are at most count groups, so the last rank is < count.
                        rank += 1;
                    }
                }
                checked()?;
                scratch[position] = rank;
                previous = Some(position);
            }
            checked()?;
            // The sorted positions are the semantic order; the scratch holds ranks.
            std::mem::swap(&mut order.ranks, &mut scratch);
            order.semantic = scratch;
            Ok(())
        })();
        result.map_err(|failure| ModelPublicationFailure {
            failure,
            peak_bytes: account.peak,
        })?;
        order.preparation_peak_bytes = account.peak;
        Ok(order)
    }

    /// Order header and actual retained rank and semantic-order capacity;
    /// excludes the catalog.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128 + cells(self.ranks.capacity()) + cells(self.semantic.capacity())
    }

    /// Actual named peak during successful preparation, including its scratch.
    #[must_use]
    pub const fn preparation_peak_bytes(&self) -> u128 {
        self.preparation_peak_bytes
    }

    /// Publish supplied positions using this exact catalog's prepared order.
    ///
    /// Every position is checked. Repeats and equal logical atoms coalesce,
    /// retaining the least supplied original position of each equivalence class.
    /// An unselected occurrence is never introduced. The existing [`Model`] shape is
    /// published directly; its catalog and selection remain independent of this
    /// order's lifetime. No semantic comparison or payload import is repeated.
    ///
    /// Supplied positions are marked in an n-bit mask. When the m marked
    /// positions are dense enough that m·⌈log₂(m + 1)⌉ reaches n, the prepared
    /// semantic order is walked once, keeping the first marked occurrence of
    /// each rank (the least marked position, since equal ranks ascend by
    /// position), and the walk ends at the last marked occurrence. Otherwise the
    /// marked positions are sorted by rank and position and each rank's first is
    /// kept. The choice depends only on m and n and is made before either runs;
    /// both publish the same model.
    ///
    /// The allowance and success/failure peak include the retained order plus
    /// this attempt's live mask/position/scratch headers, actual capacities,
    /// growth overlap and final selection header when allocated. Previously
    /// published models, catalog payload, caller inputs, callback/receipt frames,
    /// allocator metadata and Arc counters are excluded. The mask and scratch are
    /// not retained.
    ///
    /// Permits precede iterator pulls, mask and rank access, sort comparisons,
    /// writes, reservations and final publication. The walk's loop control and
    /// its rank comparison follow the permit taken for the same occurrence. A finite caller work
    /// budget bounds successful iterator pulls; it cannot preempt a blocking
    /// `next`. The first callback precedes iterator conversion and allocation.
    /// Stable Rust's final Arc allocation remains infallible after the last
    /// permit.
    ///
    /// # Errors
    /// Returns the first invalid position, metadata-capacity, allocation or
    /// caller refusal with the actual named peak. No partial model escapes;
    /// the order, catalog and earlier models remain valid for subsequent calls.
    pub fn select_with<E>(
        &self,
        positions: impl IntoIterator<Item = usize>,
        max_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<ModelPublication, ModelPublicationFailure<E>> {
        let mut marks = Vec::new();
        let mut selected = Vec::new();
        let mut scratch = Vec::new();
        let base = self.retained_bytes() + 3 * VECTOR_BYTES;
        let mut account = Account::new(max_bytes, base);
        let result = (|| {
            let mut checked = || before().map_err(ModelFailure::Stopped);
            checked()?;
            account.admit(base)?;
            let count = self.ranks.len();
            let words = count.div_ceil(MARK_BITS);
            reserve(&mut marks, words, base, &mut account, &mut checked)?;
            for _ in 0..words {
                checked()?;
                marks.push(0);
            }
            let mut marked = 0;
            let mut positions = positions.into_iter();
            loop {
                checked()?;
                let Some(position) = positions.next() else {
                    break;
                };
                checked()?;
                if position >= count {
                    return Err(ModelFailure::Model(ModelError::Position {
                        position,
                        atoms: count,
                    }));
                }
                let bit = 1 << (position % MARK_BITS);
                let word = &mut marks[position / MARK_BITS];
                if *word & bit == 0 {
                    *word |= bit;
                    marked += 1;
                }
            }
            let other = base + cells(marks.capacity());
            reserve(&mut selected, marked, other, &mut account, &mut checked)?;
            if walks(marked, count) {
                self.walk(&marks, marked, &mut selected, &mut checked)?;
            } else {
                for (index, &word) in marks.iter().enumerate() {
                    checked()?;
                    let mut bits = word;
                    while bits != 0 {
                        checked()?;
                        selected.push(index * MARK_BITS + bits.trailing_zeros() as usize);
                        bits &= bits - 1;
                    }
                }
                reserve(
                    &mut scratch,
                    selected.len(),
                    other + cells(selected.capacity()),
                    &mut account,
                    &mut checked,
                )?;
                for _ in 0..selected.len() {
                    checked()?;
                    scratch.push(0);
                }
                self.sort_marked(&mut selected, &mut scratch, &mut checked)?;
            }
            drop(scratch);
            drop(marks);
            let published =
                self.retained_bytes() + size_of::<Selected>() as u128 + cells(selected.capacity());
            account.admit(published)?;
            checked()?;
            let model = Model(Arc::new(Selected {
                catalog: self.catalog.clone(),
                positions: selected,
            }));
            account.record(published);
            Ok(ModelPublication {
                model,
                peak_bytes: account.peak,
            })
        })();
        result.map_err(|failure| ModelPublicationFailure {
            failure,
            peak_bytes: account.peak,
        })
    }

    /// Keep the first marked occurrence of each rank along the semantic order,
    /// stopping after the last marked occurrence. `selected` has room for
    /// `marked` cells.
    ///
    /// The first marked occurrence of a rank is its least marked position, as
    /// the sorting route keeps: preparation's merge sort is stable over
    /// ascending positions, and occurrences of one rank compare equal, so equal
    /// ranks appear in the semantic order by ascending position.
    fn walk<E>(
        &self,
        marks: &[usize],
        marked: usize,
        selected: &mut Vec<usize>,
        checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
    ) -> Result<(), ModelFailure<E>> {
        let mut previous = None;
        let mut seen = 0;
        for &position in &self.semantic {
            if seen == marked {
                break;
            }
            checked()?;
            if marks[position / MARK_BITS] & (1 << (position % MARK_BITS)) == 0 {
                continue;
            }
            seen += 1;
            checked()?;
            let rank = self.ranks[position];
            if previous != Some(rank) {
                checked()?;
                selected.push(position);
                previous = Some(rank);
            }
        }
        Ok(())
    }

    /// Sort marked positions by rank, then position, and keep each rank's first.
    fn sort_marked<E>(
        &self,
        selected: &mut Vec<usize>,
        scratch: &mut Vec<usize>,
        checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
    ) -> Result<(), ModelFailure<E>> {
        crate::checked_sort::sort(
            selected,
            scratch,
            |values, left, right, permit| {
                permit()?;
                let left = values[left];
                permit()?;
                let right = values[right];
                permit()?;
                let left_rank = self.ranks[left];
                permit()?;
                let right_rank = self.ranks[right];
                permit()?;
                let order = left_rank.cmp(&right_rank);
                if order.is_eq() {
                    permit()?;
                    Ok(left.cmp(&right))
                } else {
                    Ok(order)
                }
            },
            checked,
        )?;
        let mut previous = None;
        let mut kept = 0;
        for index in 0..selected.len() {
            checked()?;
            let position = selected[index];
            checked()?;
            let rank = self.ranks[position];
            checked()?;
            if previous != Some(rank) {
                checked()?;
                selected[kept] = position;
                kept += 1;
                previous = Some(rank);
            }
        }
        checked()?;
        selected.truncate(kept);
        Ok(())
    }
}

/// Whether walking the n-occurrence semantic order is expected to cost no
/// more than sorting `marked` positions: m·⌈log₂(m + 1)⌉ ≥ n.
const fn walks(marked: usize, count: usize) -> bool {
    let depth = (usize::BITS - marked.leading_zeros()) as usize;
    marked.saturating_mul(depth) >= count
}

fn cells(capacity: usize) -> u128 {
    capacity as u128 * size_of::<usize>() as u128
}

struct Account {
    limit: usize,
    peak: u128,
}

impl Account {
    const fn new(limit: usize, initialized_headers: u128) -> Self {
        Self {
            limit,
            peak: initialized_headers,
        }
    }

    fn admit<E>(&self, required: u128) -> Result<(), ModelFailure<E>> {
        if required > self.limit as u128 {
            Err(ModelFailure::Model(ModelError::Bytes {
                required,
                limit: self.limit,
            }))
        } else {
            Ok(())
        }
    }

    fn record(&mut self, observed: u128) {
        self.peak = self.peak.max(observed);
    }

    fn observe<E>(&mut self, observed: u128) -> Result<(), ModelFailure<E>> {
        self.record(observed);
        self.admit(observed)
    }
}

/// `other` includes named owner/vector headers and all other live capacities.
/// Allocate a separate replacement to make old/new overlap explicit. Refusal
/// during copying preserves the original buffer; actual replacement capacity is
/// recorded before any later callback can refuse. All byte arithmetic uses u128
/// over a fixed number of usize capacities, so it cannot overflow.
fn reserve<E>(
    buffer: &mut Vec<usize>,
    needed: usize,
    other: u128,
    account: &mut Account,
    before: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
) -> Result<(), ModelFailure<E>> {
    if needed <= buffer.capacity() {
        return Ok(());
    }
    let requested = needed.max(buffer.capacity().saturating_mul(2));
    let overlap = other + cells(buffer.capacity()) + VECTOR_BYTES;
    account.admit(overlap + cells(requested))?;
    before()?;
    let mut replacement = Vec::new();
    account.observe(overlap)?;
    let result = replacement.try_reserve_exact(requested);
    account.observe(overlap + cells(replacement.capacity()))?;
    result.map_err(|_| ModelFailure::Model(ModelError::Allocation))?;
    for &position in buffer.iter() {
        before()?;
        replacement.push(position);
    }
    before()?;
    *buffer = replacement;
    Ok(())
}
