//! Exact conjunction filtering over an explicitly supplied ordered selection.

use std::mem::size_of;

use crate::Value;

use super::{Failure, Limits, Relation, Row, storage};

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
    /// Work is O(input rows times equalities), with constant-width ID comparisons.
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
        if !self.same_owner(query.relation) || !self.same_owner(input.relation) {
            return Err(Failure::Owner);
        }
        let inputs = query
            .bytes
            .checked_add(input.bytes)
            .ok_or(Failure::Overflow)?;
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
        if query.possible {
            for &position in &input.positions {
                work.tick(1)?;
                let mut matches = true;
                for equality in &query.equalities {
                    work.tick(1)?;
                    let id = self.columns[equality.column * self.row_count() + position];
                    if id != equality.value_id {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    work.tick(1)?;
                    positions.push(position);
                }
            }
        }
        Ok(Selection {
            relation: self,
            positions,
            bytes: work.live - self.storage.retained_bytes - inputs,
            work: work.used,
        })
    }
}
