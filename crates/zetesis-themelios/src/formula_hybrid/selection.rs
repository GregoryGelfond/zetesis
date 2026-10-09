//! Necessary region truth applied before binding and scalar evaluation.
//!
//! Rows still belong to complete possible support. This view only maps their
//! original occurrence positions to the admitted formula's dense IDs; it owns
//! no atom payload and does not establish a new support certificate. A source
//! row absent from the formula catalog remains eligible: scalar filters may
//! exclude it before it contributes any admitted atom occurrence.

mod predicates;
pub(super) use predicates::RulePredicates;

use crate::ProgramSite;
use themelios_program::program::DefaultNegation;
use zetesis_core::{
    AtomLookup, AtomRow,
    catalog::Atoms,
    relation::{Failure, Row},
};
use zetesis_cpu::regions::Region;

use crate::formula_ir::RuleIr;
use crate::formula_support::{CompletedSupport, Counters, RowFilter, RowSelection};
use crate::{FormulaFailure, FormulaLimits};

/// Each kept source predicate's occurrence positions in the core's dense
/// catalog, in the order the core's streamed rows list those predicates. A
/// function of the immutable core alone: built once, by the first checker
/// that needs it, and shared by every checker of the core.
pub(super) struct RowPositions {
    predicates: Vec<Vec<Option<usize>>>,
    bytes: u128,
}

impl RowPositions {
    /// Map every source occurrence through `index`: one charged probe per
    /// kept support row. Each buffer is admitted against `support`'s
    /// workspace before it is filled; the caller retains the total.
    pub(super) fn prepare(
        support: &CompletedSupport<'_>,
        index: AtomLookup<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut count = 0;
        for _ in support.source_atoms() {
            counters.work(limits, location)?;
            count += 1;
        }
        let mut predicates = Vec::new();
        let mut bytes = size_of::<Self>() as u128;
        reserve(
            &mut predicates,
            count,
            &mut bytes,
            support,
            limits,
            counters,
            location,
        )?;
        for (_, atoms) in support.source_atoms() {
            counters.work(limits, location)?;
            let mut positions = Vec::new();
            reserve(
                &mut positions,
                atoms.len(),
                &mut bytes,
                support,
                limits,
                counters,
                location,
            )?;
            for atom in atoms {
                let found = index.get_with(atom, || counters.work(limits, location))?;
                counters.work(limits, location)?;
                positions.push(found.map(AtomRow::position));
            }
            counters.work(limits, location)?;
            predicates.push(positions);
        }
        Ok(Self { predicates, bytes })
    }

    /// The header and every reserved buffer's capacity.
    pub(super) const fn retained_bytes(&self) -> u128 {
        self.bytes
    }
}

struct PredicateRows<'source> {
    /// The exact source occurrence mapping used by every row of this relation.
    atoms: Atoms<'source>,
    positions: &'source [Option<usize>],
}

/// A checker's integer correspondence: its own occurrence maps paired with
/// the core's shared positions. No region decisions are retained.
pub(super) struct SourceRows<'source> {
    predicates: Vec<PredicateRows<'source>>,
    bytes: u128,
}

impl<'source> SourceRows<'source> {
    /// Pair each kept predicate's occurrence map in `support` with the core's
    /// positions: O(predicates) work. The maps and the positions list the
    /// same predicates in the same order with the same lengths, since both
    /// come from the core's immutable streamed rows; a mismatch is refused.
    pub(super) fn attach(
        support: &CompletedSupport<'source>,
        positions: &'source RowPositions,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let owner = || FormulaFailure::SupportRelation {
            error: Failure::Owner,
            location,
        };
        let mut count = 0;
        for _ in support.source_atoms() {
            counters.work(limits, location)?;
            count += 1;
        }
        if count != positions.predicates.len() {
            return Err(owner());
        }
        let mut predicates = Vec::new();
        let mut bytes = size_of::<Self>() as u128;
        reserve(
            &mut predicates,
            count,
            &mut bytes,
            support,
            limits,
            counters,
            location,
        )?;
        for ((_, atoms), positions) in support.source_atoms().zip(&positions.predicates) {
            counters.work(limits, location)?;
            if atoms.len() != positions.len() {
                return Err(owner());
            }
            predicates.push(PredicateRows { atoms, positions });
        }
        Ok(Self { predicates, bytes })
    }

    /// The header and the reserved pairing buffer's capacity, which the
    /// caller retains in its ledger; the shared positions are not included.
    pub(super) const fn retained_bytes(&self) -> u128 {
        self.bytes
    }
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    bytes: &mut u128,
    support: &CompletedSupport<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    // Each vector is reserved once before it is filled; no growth overlap or
    // hidden tuple copies. Check allocator slack before retaining the buffer.
    support.admit_workspace(
        *bytes + count as u128 * size_of::<T>() as u128,
        limits,
        counters,
        location,
    )?;
    counters.work(limits, location)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| FormulaFailure::SupportRelation {
            error: Failure::Allocation,
            location,
        })?;
    let actual = *bytes + values.capacity() as u128 * size_of::<T>() as u128;
    support.admit_workspace(actual, limits, counters, location)?;
    *bytes = actual;
    Ok(())
}

/// Borrowed region decisions over prepared source and formula identities.
pub(super) struct Selection<'a, 'source> {
    pub(super) rows: &'a SourceRows<'source>,
    pub(super) index: AtomLookup<'a, 'source>,
    /// Present when this filter also serves one prepared source body.
    pub(super) predicates: Option<&'a RulePredicates<'source>>,
    pub(super) region: &'a Region,
}

impl Selection<'_, '_> {
    /// A necessary predicate-level condition, independent of bindings. Matching
    /// arguments and every scalar condition remain the ordinary join/body work.
    pub(super) fn possible(
        &self,
        rule: &RuleIr,
        support: &CompletedSupport<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        self.possible_with(
            rule,
            support,
            limits,
            counters,
            |_, negation, position| match negation {
                DefaultNegation::Not => self.region.is_cut(position),
                DefaultNegation::None | DefaultNegation::NotNot => self.region.is_held(position),
            },
        )
    }

    fn possible_with(
        &self,
        rule: &RuleIr,
        support: &CompletedSupport<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        accepts: impl Fn(usize, DefaultNegation, usize) -> bool,
    ) -> Result<bool, FormulaFailure> {
        for (occurrence, literal) in rule.body.iter().enumerate() {
            let Some((negation, pattern)) = super::literal_atom(literal) else {
                continue;
            };
            if self.predicates.is_some() {
                // The prepared occurrence descriptor replaces pattern.get's
                // charged descriptor read; empty windows still charge it.
                counters.work(limits, rule.location)?;
            }
            let rows = if let Some(prepared) = self
                .predicates
                .and_then(|prepared| prepared.at(&rule.body, occurrence))
            {
                prepared.rows()
            } else {
                let components = support
                    .components()
                    .ok_or_else(|| crate::formula_support::components::missing(rule.location))?;
                let pattern = pattern.get(components, limits, counters, rule.location)?;
                self.index
                    .predicate_with(pattern.predicate(), || counters.work(limits, rule.location))?
            };
            let mut possible = false;
            for row in rows {
                counters.work(limits, rule.location)?;
                possible = accepts(occurrence, negation, row.position());
                if possible {
                    break;
                }
            }
            if !possible {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl Selection<'_, '_> {
    fn position(
        &self,
        source: usize,
        row: Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        // The join pairs this slot with its authenticated immutable relation.
        // Reordered/repeated rows retain their original source index.
        counters.work(limits, location)?;
        self.rows
            .predicates
            .get(source)
            .and_then(|source| source.positions.get(row.source_index()))
            .copied()
            .ok_or(FormulaFailure::SupportRelation {
                error: Failure::Owner,
                location,
            })
    }
}

/// One immutable query mode: no open positive input, or exactly one original
/// positive occurrence whose mapped row must be open. No selection state is
/// retained across row backtracking; aliases with two open occurrences decline.
#[derive(Clone, Copy)]
pub(super) enum ConsequenceMode {
    Held,
    Pivot(usize),
    SingleOpen { forced: Option<usize> },
}

impl ConsequenceMode {
    fn accepts(self, occurrence: usize, decision: Option<bool>) -> bool {
        match self {
            Self::Held => decision == Some(true),
            Self::Pivot(pivot)
            | Self::SingleOpen {
                forced: Some(pivot),
            } => {
                if pivot == occurrence {
                    decision.is_none()
                } else {
                    decision == Some(true)
                }
            }
            Self::SingleOpen { forced: None } => decision != Some(false),
        }
    }
}

pub(super) struct ConsequenceSelection<'a, 'source> {
    pub(super) selection: Selection<'a, 'source>,
    pub(super) mode: ConsequenceMode,
}

impl ConsequenceSelection<'_, '_> {
    pub(super) fn possible(
        &self,
        rule: &RuleIr,
        support: &CompletedSupport<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        self.selection.possible_with(
            rule,
            support,
            limits,
            counters,
            |occurrence, negation, atom| {
                let region = self.selection.region;
                match negation {
                    DefaultNegation::None => self.mode.accepts(occurrence, region.decision(atom)),
                    DefaultNegation::Not => !region.is_held(atom),
                    DefaultNegation::NotNot => !region.is_cut(atom),
                }
            },
        )
    }
}

impl<'a, 'source> ConsequenceSelection<'a, 'source> {
    /// The union of viable held-only and original positive-pivot modes.
    /// Two positive occurrences with no held candidate cannot form a supported
    /// conservative unit. One such occurrence fixes the only possible pivot.
    pub(super) fn shared(
        selection: Selection<'a, 'source>,
        rule: &RuleIr,
        support: &CompletedSupport<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Option<Self>, FormulaFailure> {
        let mut forced = None;
        for (occurrence, literal) in rule.body.iter().enumerate() {
            counters.work(limits, rule.location)?;
            let Some((negation, pattern)) = super::literal_atom(literal) else {
                continue;
            };
            let rows = if let Some(prepared) = selection
                .predicates
                .and_then(|prepared| prepared.at(&rule.body, occurrence))
            {
                prepared.rows()
            } else {
                let components = support
                    .components()
                    .ok_or_else(|| crate::formula_support::components::missing(rule.location))?;
                let pattern = pattern.get(components, limits, counters, rule.location)?;
                selection
                    .index
                    .predicate_with(pattern.predicate(), || counters.work(limits, rule.location))?
            };
            let mut possible = false;
            let mut open = false;
            for row in rows {
                counters.work(limits, rule.location)?;
                let decision = selection.region.decision(row.position());
                possible = match negation {
                    DefaultNegation::None => decision == Some(true),
                    DefaultNegation::Not => decision != Some(true),
                    DefaultNegation::NotNot => decision != Some(false),
                };
                open |= decision.is_none();
                if possible {
                    break;
                }
            }
            if !possible {
                if negation != DefaultNegation::None || !open || forced.is_some() {
                    return Ok(None);
                }
                forced = Some(occurrence);
            }
        }
        Ok(Some(Self {
            selection,
            mode: ConsequenceMode::SingleOpen { forced },
        }))
    }
}

impl RowFilter for ConsequenceSelection<'_, '_> {
    fn single_open(&self) -> bool {
        matches!(self.mode, ConsequenceMode::SingleOpen { .. })
    }

    fn resolve(
        &self,
        atoms: Atoms<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        self.selection.resolve(atoms, limits, counters, location)
    }

    fn permits(
        &self,
        source: usize,
        row: Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        // Without an occurrence this is the held-only eligibility operation.
        // The join invokes select with the authenticated original occurrence.
        self.selection
            .permits(source, row, limits, counters, location)
    }

    fn select(
        &self,
        source: usize,
        occurrence: usize,
        row: Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<RowSelection, FormulaFailure> {
        let Some(atom) = self
            .selection
            .position(source, row, limits, counters, location)?
        else {
            return Ok(RowSelection::Possible);
        };
        let decision = self.selection.region.decision(atom);
        Ok(if !self.mode.accepts(occurrence, decision) {
            RowSelection::Rejected
        } else if decision == Some(true) {
            RowSelection::Held
        } else {
            // The mapped identity may be lent after exact matching,
            // but an open pivot never issues all-held evidence.
            RowSelection::Open { atom }
        })
    }
}

impl RowFilter for Selection<'_, '_> {
    fn resolve(
        &self,
        atoms: Atoms<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        // Resolve the exact occurrence map once per joined source. Equal atom
        // or predicate contents cannot establish the position correspondence.
        for (position, source) in self.rows.predicates.iter().enumerate() {
            counters.work(limits, location)?;
            if atoms.same_occurrences(source.atoms) {
                return Ok(position);
            }
        }
        Err(FormulaFailure::SupportRelation {
            error: Failure::Owner,
            location,
        })
    }

    fn permits(
        &self,
        source: usize,
        row: Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        self.select(source, 0, row, limits, counters, location)
            .map(RowSelection::permits)
    }

    fn select(
        &self,
        source: usize,
        _occurrence: usize,
        row: Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<RowSelection, FormulaFailure> {
        Ok(
            match self.position(source, row, limits, counters, location)? {
                None => RowSelection::Possible,
                Some(position) if self.region.is_held(position) => RowSelection::Held,
                Some(_) => RowSelection::Rejected,
            },
        )
    }
}

#[cfg(test)]
mod routes;

#[cfg(test)]
mod tests;
