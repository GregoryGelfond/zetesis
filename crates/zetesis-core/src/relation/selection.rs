//! Exact conjunction filtering over an explicitly supplied ordered selection.

use std::mem::size_of;

use crate::Value;

use super::{Failure, Limits, Relation, Row, Work, storage};

/// A dictionary-resolved equality. Its meaning requires the owning query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Equality {
    column: usize,
    value_id: u32,
}

impl Equality {
    /// Checked argument-column index.
    #[must_use]
    pub const fn column(self) -> usize {
        self.column
    }

    /// Equality identifier in the owning relation, never a numeric/order value.
    #[must_use]
    pub const fn value_id(self) -> u32 {
        self.value_id
    }
}

/// A checked conjunction of whole-value equalities for one immutable owner.
///
/// Empty equalities impose no restriction. An absent dictionary value makes
/// `is_possible()` false even when the remaining equalities are empty. This is
/// row-filter feasibility only, not an ASP consistency or membership result.
pub struct Query<'owner, 'source> {
    relation: &'owner Relation<'source>,
    equalities: Vec<Equality>,
    possible: bool,
    bytes: usize,
    work: u128,
}

impl<'owner, 'source> Query<'owner, 'source> {
    /// The exact borrowed relation owner.
    #[must_use]
    pub const fn relation(&self) -> &'owner Relation<'source> {
        self.relation
    }

    /// Checked equalities in caller order, resolved in this owner's dictionary.
    ///
    /// Repeated columns remain conjunctions; no source condition is deduplicated.
    #[must_use]
    pub fn equalities(&self) -> &[Equality] {
        &self.equalities
    }

    /// Whether every queried value was present in the dictionary.
    ///
    /// True does not imply that any row satisfies the conjunction.
    #[must_use]
    pub const fn is_possible(&self) -> bool {
        self.possible
    }

    /// Query object and equality-vector capacity, excluding the borrowed owner.
    #[must_use]
    pub const fn retained_bytes(&self) -> usize {
        self.bytes
    }

    /// Charged column inspections and typed dictionary-lookup work.
    #[must_use]
    pub const fn work(&self) -> u128 {
        self.work
    }
}

/// An owner-bound, ordered subset of row positions.
///
/// This type validates structure only. It is not evidence that a query was
/// evaluated, that every relation row was visited or that an answer set exists.
/// [`Relation::select`] completes one query over its supplied input subset;
/// arbitrary [`Relation::selection`] inputs make no completeness claim.
pub struct Selection<'owner, 'source> {
    relation: &'owner Relation<'source>,
    positions: Vec<usize>,
    bytes: usize,
    work: u128,
}

impl<'owner, 'source> Selection<'owner, 'source> {
    /// The exact borrowed relation owner.
    #[must_use]
    pub const fn relation(&self) -> &'owner Relation<'source> {
        self.relation
    }

    /// Increasing, unique local positions; source/catalog IDs are separate.
    #[must_use]
    pub fn positions(&self) -> &[usize] {
        &self.positions
    }

    /// Typed access to one selected row occurrence.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<Row<'owner, 'source>> {
        self.relation.row(*self.positions.get(index)?)
    }

    /// Selection object and position-vector capacity, excluding borrowed owner.
    #[must_use]
    pub const fn retained_bytes(&self) -> usize {
        self.bytes
    }

    /// Charged work for this construction/filter operation, excluding inputs.
    #[must_use]
    pub const fn work(&self) -> u128 {
        self.work
    }
}

/// The complete equality-filter result over one supplied ordered selection.
///
/// Bits denote original row positions in the exact borrowed owner. Equal typed
/// tuples at different positions remain distinct occurrences. Unused tail bits
/// are zero. The result covers the whole relation only when the input did so;
/// it does not establish full pattern matching or answer-set membership.
pub struct Mask<'owner, 'source> {
    relation: &'owner Relation<'source>,
    words: Vec<u32>,
    bytes: usize,
    work: u128,
}

impl<'owner, 'source> Mask<'owner, 'source> {
    /// The exact borrowed relation owner.
    #[must_use]
    pub const fn relation(&self) -> &'owner Relation<'source> {
        self.relation
    }

    /// Low-bit-first original-row membership, including zero tail padding.
    #[must_use]
    pub fn words(&self) -> &[u32] {
        &self.words
    }

    /// Mask object and word-vector capacity, excluding the borrowed owner.
    #[must_use]
    pub const fn retained_bytes(&self) -> usize {
        self.bytes
    }

    /// Charged zero writes, row visits, equality comparisons and selected bits.
    #[must_use]
    pub const fn work(&self) -> u128 {
        self.work
    }
}

impl<'source> Relation<'source> {
    /// Resolve whole-value equalities without retaining caller value borrows.
    ///
    /// Construction performs logarithmic dictionary lookup per equality,
    /// including typed payload costs. Input order and repeated columns remain
    /// explicit. A missing value cannot make a later invalid column valid.
    ///
    /// # Errors
    /// Refuses invalid columns, resource excess and allocation failure. Limits
    /// include the relation and resulting query; other caller frames are external.
    pub fn query(
        &self,
        equalities: &[(usize, &Value)],
        limits: Limits,
    ) -> Result<Query<'_, 'source>, Failure> {
        let mut work = self.work(limits, size_of::<Query<'_, '_>>())?;
        let mut resolved = work.reserve(equalities.len())?;
        let mut possible = true;
        for &(column, value) in equalities {
            work.tick(1)?;
            if column >= self.predicate.arity() {
                return Err(Failure::Column);
            }
            if let Some(index) = storage::lookup(&self.dictionary, value, &mut work)? {
                let value_id = u32::try_from(index).map_err(|_| Failure::Overflow)?;
                work.tick(1)?;
                resolved.push(Equality { column, value_id });
            } else {
                possible = false;
            }
        }
        Ok(Query {
            relation: self,
            equalities: resolved,
            possible,
            bytes: work.live - self.storage.retained_bytes,
            work: work.used,
        })
    }

    /// Copy a structurally valid ordered subset into this owner's selection.
    ///
    /// This is also the checked boundary for reconstructed device row positions.
    /// It does not assert that a device or query selected the correct rows.
    /// Work and retained output capacity are linear in supplied positions.
    ///
    /// # Errors
    /// Refuses repeated, descending or out-of-range positions, resource excess
    /// and allocation failure. Limits include the relation and returned selection.
    pub fn selection(
        &self,
        positions: &[usize],
        limits: Limits,
    ) -> Result<Selection<'_, 'source>, Failure> {
        let mut work = self.work(limits, size_of::<Selection<'_, '_>>())?;
        let mut previous = None;
        for &position in positions {
            work.tick(1)?;
            if position >= self.row_count() || previous.is_some_and(|last| last >= position) {
                return Err(Failure::Selection);
            }
            previous = Some(position);
        }
        let mut copied = work.reserve(positions.len())?;
        for &position in positions {
            work.tick(1)?;
            copied.push(position);
        }
        Ok(Selection {
            relation: self,
            positions: copied,
            bytes: work.live - self.storage.retained_bytes,
            work: work.used,
        })
    }

    /// Decode one packed row mask into an increasing structural selection.
    ///
    /// Bit zero of the first word denotes original row position zero. The
    /// mask has exactly `row_count().div_ceil(32)` words, with unused high bits
    /// zero. Ordered bit traversal supplies unique in-range positions directly;
    /// no temporary row vector or second copy is retained.
    ///
    /// Limits include this relation, the borrowed mask's slice bytes and the
    /// returned selection. Allocation capacity outside that slice, an enclosing
    /// batch object and other live selections remain caller-accounted. Work
    /// charges two word scans and two operations per selected row (decode and
    /// copy). The returned work counter covers that complete reconstruction.
    ///
    /// Raw mask bits carry no owner or semantic certificate. The caller chooses
    /// this owner; shape validity does not prove query satisfaction, complete
    /// filtering, pattern matching or answer-set membership.
    ///
    /// # Errors
    /// Refuses wrong word count, nonzero unused tail bits, resource excess and
    /// allocation failure without publishing a partial selection.
    pub fn selection_from_mask(
        &self,
        words: &[u32],
        limits: Limits,
    ) -> Result<Selection<'_, 'source>, Failure> {
        let bits = u32::BITS as usize;
        if words.len() != self.row_count().div_ceil(bits) {
            return Err(Failure::Mask);
        }
        let mask_bytes = std::mem::size_of_val(words);
        let extra = mask_bytes
            .checked_add(size_of::<Selection<'_, '_>>())
            .ok_or(Failure::Overflow)?;
        let mut work = self.work(limits, extra)?;
        let tail = self.row_count() % bits;
        let mut count = 0_usize;
        for (index, &word) in words.iter().enumerate() {
            work.tick(1)?;
            if index + 1 == words.len() && tail != 0 && word >> tail != 0 {
                return Err(Failure::Mask);
            }
            count = count
                .checked_add(word.count_ones() as usize)
                .ok_or(Failure::Overflow)?;
        }
        let mut positions = work.reserve(count)?;
        for (index, &word) in words.iter().enumerate() {
            work.tick(1)?;
            let mut remaining = word;
            while remaining != 0 {
                work.tick(2)?;
                positions.push(index * bits + remaining.trailing_zeros() as usize);
                remaining &= remaining - 1;
            }
        }
        Ok(Selection {
            relation: self,
            positions,
            bytes: work.live - self.storage.retained_bytes - mask_bytes,
            work: work.used,
        })
    }

    /// Construct the complete input row sequence in original occurrence order.
    ///
    /// This asserts only structural row coverage, not satisfaction or membership.
    ///
    /// # Errors
    /// Refuses resource excess or allocation failure before publishing a result.
    /// Limits include the relation and resulting position-vector capacity.
    pub fn all(&self, limits: Limits) -> Result<Selection<'_, 'source>, Failure> {
        let mut work = self.work(limits, size_of::<Selection<'_, '_>>())?;
        let mut positions = work.reserve(self.row_count())?;
        for position in 0..self.row_count() {
            work.tick(1)?;
            positions.push(position);
        }
        Ok(Selection {
            relation: self,
            positions,
            bytes: work.live - self.storage.retained_bytes,
            work: work.used,
        })
    }

    /// Return exactly the input rows satisfying every supplied equality.
    ///
    /// The result preserves the complete input order. It covers the relation
    /// only when the caller supplied all rows. No equality discharges structural
    /// matching, repeated unbound variables, guards, support or minimality.
    /// Work is O(input rows times (1 + equalities)), with constant-width IDs.
    ///
    /// # Errors
    /// Refuses foreign owners before filtering, resource excess or allocation
    /// failure. No partial selection is published. Limits include this relation,
    /// both supplied objects and output capacity; unrelated live frames and
    /// borrowed source payload are the caller's responsibility.
    pub fn select(
        &self,
        query: &Query<'_, 'source>,
        input: &Selection<'_, 'source>,
        limits: Limits,
    ) -> Result<Selection<'_, 'source>, Failure> {
        let inputs = self.selection_inputs(query, input)?;
        let extra = inputs
            .checked_add(size_of::<Selection<'_, '_>>())
            .ok_or(Failure::Overflow)?;
        let mut work = self.work(limits, extra)?;
        let count = if query.possible {
            input.positions.len()
        } else {
            0
        };
        let mut positions = work.reserve(count)?;
        self.visit_matches(query, input, &mut work, |position| positions.push(position))?;
        Ok(Selection {
            relation: self,
            positions,
            bytes: work.live - self.storage.retained_bytes - inputs,
            work: work.used,
        })
    }

    /// Return exactly the matching input rows as a packed original-row mask.
    ///
    /// Uses the same typed equality predicate and ordered input traversal as
    /// [`Self::select`], writing bits directly without a position vector. Work
    /// additionally charges one zero write per mask word. Even an impossible
    /// query retains `row_count().div_ceil(32)` zero words. This bounded dense
    /// output can cost more storage than positions for a sparse selection.
    ///
    /// # Errors
    /// Refuses foreign owners before allocation, resource excess or allocation
    /// failure. No partial mask is returned. Limits include this relation, both
    /// supplied objects, the mask object and its actual word capacity. Borrowed
    /// source payload and unrelated live frames remain caller-accounted.
    pub fn select_mask(
        &self,
        query: &Query<'_, 'source>,
        input: &Selection<'_, 'source>,
        limits: Limits,
    ) -> Result<Mask<'_, 'source>, Failure> {
        let inputs = self.selection_inputs(query, input)?;
        let extra = inputs
            .checked_add(size_of::<Mask<'_, '_>>())
            .ok_or(Failure::Overflow)?;
        let mut work = self.work(limits, extra)?;
        let bits = u32::BITS as usize;
        let count = self.row_count().div_ceil(bits);
        let mut words = work.reserve(count)?;
        work.tick(count as u128)?;
        words.resize(count, 0_u32);
        self.visit_matches(query, input, &mut work, |position| {
            words[position / bits] |= 1 << (position % bits);
        })?;
        Ok(Mask {
            relation: self,
            words,
            bytes: work.live - self.storage.retained_bytes - inputs,
            work: work.used,
        })
    }

    fn selection_inputs(
        &self,
        query: &Query<'_, 'source>,
        input: &Selection<'_, 'source>,
    ) -> Result<usize, Failure> {
        if !self.same_owner(query.relation) || !self.same_owner(input.relation) {
            return Err(Failure::Owner);
        }
        query
            .bytes
            .checked_add(input.bytes)
            .ok_or(Failure::Overflow)
    }

    fn visit_matches(
        &self,
        query: &Query<'_, 'source>,
        input: &Selection<'_, 'source>,
        work: &mut Work,
        mut emit: impl FnMut(usize),
    ) -> Result<(), Failure> {
        if query.possible {
            for &position in &input.positions {
                work.tick(1)?;
                if self.matches_row(query, position, work)? {
                    work.tick(1)?;
                    emit(position);
                }
            }
        }
        Ok(())
    }

    fn matches_row(
        &self,
        query: &Query<'_, 'source>,
        position: usize,
        work: &mut Work,
    ) -> Result<bool, Failure> {
        for equality in &query.equalities {
            work.tick(1)?;
            let id = self.columns[equality.column * self.row_count() + position];
            if id != equality.value_id {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
