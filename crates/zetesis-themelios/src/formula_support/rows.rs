//! A staging token locates a complete frame before the consumer borrows it.
//!
//! Returning a reference from the traversal's mutation loop would keep that
//! loop borrowed. Instead the loop reports the frame location, and `next_row`
//! creates its sole `Row`/`Binding` view after mutation has stopped. The token
//! owns no second copy of current slots and never escapes the join module.

use crate::ProgramSite;

use super::{Computation, Context, Counters, Join};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::{FormulaFailure, FormulaLimits};

/// A necessary condition on an original positive support occurrence. The join
/// consults it before binding or evaluating scalar expressions. It must charge
/// its work before execution through the supplied counters, preserving failure
/// prefixes. `false` skips only this row; an error stops the enclosing scan.
///
/// Row positions are predicate-local support coordinates. Any interpretation
/// in another catalog must authenticate that mapping; no equality of positions
/// is implied. Unmapped rows may be retained conservatively. Only already
/// admitted witness scans attach this filter, never source-family validation.
pub(crate) trait RowFilter {
    /// Restrict matched positive prefixes to at most one mapped open row.
    /// The default only lends evidence and preserves ordinary row enumeration.
    /// A consequence consumer may request this sufficient-unit query after
    /// admission has established its source-diagnostic coverage.
    fn single_open(&self) -> bool {
        false
    }

    /// Authenticate one resolved relation's original occurrence map, returning
    /// a slot owned by this filter. The join retains that slot with the exact
    /// relation and supplies it only with rows from that relation. Resolution
    /// happens once per positive occurrence, including across backtracking;
    /// absent or empty sources offer no rows and need no filter slot.
    fn resolve(
        &self,
        atoms: zetesis_core::catalog::Atoms<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure>;

    /// Select current truth after source authentication. `source` is the slot
    /// returned by this filter's successful `resolve` for `row`'s relation;
    /// the row's original source index, not its local position, reads that map.
    fn permits(
        &self,
        source: usize,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure>;

    /// Optional truth evidence at the same charged pre-binding selection.
    /// The default establishes only eligibility. An override may return Held
    /// only for the exact authenticated source occurrence held in this filter's
    /// immutable interpretation. Open also supplies that occurrence's mapped
    /// dense identity. Unmapped rows remain Possible and carry neither proof.
    /// `occurrence` is the literal position in the original body, unaffected
    /// by physical join ordering or another occurrence of the same predicate.
    fn select(
        &self,
        source: usize,
        _occurrence: usize,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<RowSelection, FormulaFailure> {
        self.permits(source, row, limits, counters, location)
            .map(|permitted| {
                if permitted {
                    RowSelection::Possible
                } else {
                    RowSelection::Rejected
                }
            })
    }
}

/// What a filtered row already established before matching its pattern.
/// Possible rows carry no candidate truth; Held is an authenticated fact in
/// the filter's immutable interpretation, not membership in possible support.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowSelection {
    Rejected,
    Possible,
    Held,
    /// Exact dense identity of this original occurrence, open in the same
    /// immutable interpretation that supplies Held. Pattern matching is still
    /// required before the identity can describe a completed binding.
    Open {
        atom: usize,
    },
}

impl RowSelection {
    pub(crate) fn permits(self) -> bool {
        self != Self::Rejected
    }
}

/// Exact positive rows in one immutable interpretation: all held, or one
/// mapped open occurrence with every other occurrence held. The loan ends
/// before cursor mutation and grants no final-model or arithmetic evidence.
pub(crate) struct PositiveRows<'row> {
    literals: &'row [crate::formula_ir::LiteralIr],
    truth: PositiveTruth,
}

enum PositiveTruth {
    Held,
    Open { occurrence: usize, atom: usize },
}

impl PositiveRows<'_> {
    /// The all-held proposition only; an open pivot never satisfies it.
    pub(crate) fn covers(&self, literals: &[crate::formula_ir::LiteralIr]) -> bool {
        std::ptr::eq(self.literals, literals) && matches!(self.truth, PositiveTruth::Held)
    }

    pub(crate) fn open_in(
        &self,
        literals: &[crate::formula_ir::LiteralIr],
    ) -> Option<(usize, usize)> {
        if !std::ptr::eq(self.literals, literals) {
            return None;
        }
        match self.truth {
            PositiveTruth::Held => None,
            PositiveTruth::Open { occurrence, atom } => Some((occurrence, atom)),
        }
    }
}

#[derive(Clone, Copy)]
struct MatchedPivot {
    depth: usize,
    occurrence: usize,
    atom: usize,
}

/// The longest truth-covered positive prefix and the first matched open row.
/// An unmapped gap stops truth coverage, but an open row beyond that gap still
/// restricts a single-open query. Such a row cannot lend evidence unless the
/// whole current prefix is covered. Undo truncates both by physical depth.
#[derive(Default)]
pub(super) struct PositivePrefix {
    matched: usize,
    pivot: Option<MatchedPivot>,
    single_open: bool,
}

impl PositivePrefix {
    pub(super) fn permits(&self, selected: RowSelection) -> bool {
        !self.single_open || self.pivot.is_none() || !matches!(selected, RowSelection::Open { .. })
    }

    pub(super) fn advance(&mut self, depth: usize, occurrence: usize, selected: RowSelection) {
        match selected {
            RowSelection::Held if self.matched == depth => self.matched += 1,
            RowSelection::Open { atom } if self.pivot.is_none() => {
                self.pivot = Some(MatchedPivot {
                    depth,
                    occurrence,
                    atom,
                });
                if self.matched == depth {
                    self.matched += 1;
                }
            }
            RowSelection::Held
            | RowSelection::Rejected
            | RowSelection::Possible
            | RowSelection::Open { .. } => {}
        }
    }

    pub(super) fn undo(&mut self, depth: usize) {
        self.matched = self.matched.min(depth);
        if self.pivot.is_some_and(|pivot| pivot.depth >= depth) {
            self.pivot = None;
        }
    }
}

pub(crate) struct SelectedRow<'row> {
    pub(crate) values: Binding<'row>,
    pub(crate) passes: bool,
    pub(crate) positives: Option<PositiveRows<'row>>,
}

/// One original positive occurrence restricted to an exact borrowed atom.
/// The source's local row is resolved once, independently of join prefixes.
pub(super) struct Anchor<'a> {
    pub(super) occurrence: usize,
    pub(super) atom: zetesis_core::catalog::AtomRef<'a>,
    pub(super) row: AnchorRow,
}

#[derive(Clone, Copy)]
pub(super) enum AnchorRow {
    Unresolved,
    Absent,
    Present(usize),
}

/// One existing join cursor with a selected-row-only consumer interface. Its
/// private cursor cannot export partial arithmetic evidence as a whole family.
pub(crate) struct FilteredRows<'a, 'source> {
    join: Join<'a, 'source>,
}

impl<'a, 'source> FilteredRows<'a, 'source> {
    pub(super) fn new(mut join: Join<'a, 'source>) -> Self {
        // This proof describes the current base binding. Generated continuations
        // and head suffixes retain the ordinary lookup path.
        join.positive_prefix = (join.row_filter.is_some()
            && !join.generated
            && join.head_slots.is_empty())
        .then(|| PositivePrefix {
            single_open: join.row_filter.is_some_and(RowFilter::single_open),
            ..PositivePrefix::default()
        });
        Self { join }
    }

    /// Restrict an unstarted filtered query at one original positive occurrence.
    /// The caller establishes coverage of its union of anchored queries. The
    /// prepared order, scalar schedule and current-region filter stay intact;
    /// the atom is a typed lookup key, never a source-row coordinate.
    pub(crate) fn anchored(
        mut self,
        occurrence: usize,
        atom: zetesis_core::catalog::AtomRef<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        counters.work(limits, location)?;
        let mut found = false;
        for pattern in &self.join.plan.patterns {
            counters.work(limits, location)?;
            found |= pattern.source == occurrence;
        }
        if !found
            || self.join.row_filter.is_none()
            || self.join.generated
            || self.join.coverage != super::Coverage::Selected
            || !self.join.head_slots.is_empty()
            || self.join.anchor.is_some()
            || self.join.traversal != super::Traversal::Searching
            || self.join.depth != 0
        {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location,
            });
        }
        self.join.anchor = Some(Anchor {
            occurrence,
            atom,
            row: AnchorRow::Unresolved,
        });
        Ok(self)
    }

    pub(crate) fn next_row(
        &mut self,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<SelectedRow<'_>>, FormulaFailure> {
        let row = self.join.next_staged(
            Ownership::Lend,
            None,
            Advance::Base,
            budget,
            Context::new(computation, limits, counters, location),
        )?;
        Ok(row.map(|row| {
            let prefix = self.join.positive_prefix.as_ref().filter(|prefix| {
                row.passes
                    && matches!(&row.frame, Frame::Current)
                    && prefix.matched == self.join.plan.patterns.len()
            });
            let positives = prefix.map(|prefix| PositiveRows {
                literals: self.join.literals,
                truth: prefix
                    .pivot
                    .map_or(PositiveTruth::Held, |pivot| PositiveTruth::Open {
                        occurrence: pivot.occurrence,
                        atom: pivot.atom,
                    }),
            });
            SelectedRow {
                values: row.frame.into_binding(&self.join.values),
                passes: row.passes,
                positives,
            }
        }))
    }
}

/// The current generator may drain independently while a base row is suspended.
#[derive(Clone, Copy)]
pub(super) enum Advance {
    Base,
    Current,
}

#[derive(Clone, Copy)]
pub(super) enum Ownership {
    /// The consumer finishes reading before the next traversal step.
    Lend,
    /// A generator or owning adapter retains the completed binding.
    Own,
}

pub(super) enum Frame {
    /// `Join::values` is still complete; its final-depth undo is suspended.
    Current,
    Owned(Binding<'static>),
}

impl Frame {
    pub(super) fn binding<'a>(&'a self, current: &'a Binding) -> Binding<'a> {
        Binding::borrowed(match self {
            Self::Current => current.slots(),
            Self::Owned(binding) => binding.slots(),
        })
    }

    pub(super) fn into_binding<'a>(self, current: &'a Binding<'_>) -> Binding<'a> {
        match self {
            Self::Current => Binding::borrowed(current.slots()),
            Self::Owned(binding) => binding,
        }
    }

    /// Owning adapters and generated continuations request `Ownership::Own`
    /// before completion, preserving copy admission before scalar filtering.
    pub(super) fn into_owned(self) -> Binding<'static> {
        match self {
            Self::Owned(binding) => binding,
            Self::Current => unreachable!("owning continuation requested an owned frame"),
        }
    }
}

pub(super) struct Staged {
    pub(super) frame: Frame,
    pub(super) passes: bool,
}

#[cfg(test)]
mod filtering;
#[cfg(test)]
mod tests;
