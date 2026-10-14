//! Disjoint first-new-occurrence joins for certified possible-head producers.
//!
//! Every new positive tuple combination has a unique first occurrence whose
//! row was appended in the previous round. Earlier occurrences use old rows,
//! that occurrence uses new rows, and later occurrences use all current rows.
//! Flat and structural positive occurrences use the same row partition.
//! Scalar checks and generators remain with the complete existing matcher.
//! Ordinary negative atoms are non-inputs: their truth remains in the emitted
//! formula, while their already-bound arguments supply no support restriction.
//! A rich producer takes one full traversal when it is traversed at all, which
//! its registration decides. This scheduling certificate is not a certificate
//! of authored-body validation during final formula emission.

#[cfg(test)]
mod tests;

use std::ops::Range;

use crate::ProgramSite;
use themelios_program::program::DefaultNegation;

use super::{Counters, Support};
use crate::formula_ir::{LiteralIr, RuleIr};
use crate::{FormulaFailure, FormulaLimits};

/// One visit's variants: the full traversal at most once, then a certified
/// producer's delta variants over its registered positive occurrences.
pub(super) struct Variants<'a, 'source> {
    rule: &'a RuleIr,
    support: &'a Support<'source>,
    full: bool,
    next: usize,
    inputs: &'a [usize],
}

/// Whether structural occurrences are certified delta inputs; the test-only
/// structural reference turns this off.
#[cfg(test)]
pub(super) fn structural_enabled() -> bool {
    tests::structural_enabled()
}

/// A certified producer's variants over its positive body occurrences, which
/// the producer plan or the producer's registration recorded once; no
/// per-round classification scan. A producer without inputs runs once, in the
/// first round.
pub(super) fn prepared_variants<'a, 'source>(
    rule: &'a RuleIr,
    support: &'a Support<'source>,
    inputs: &'a [usize],
    first: bool,
) -> Variants<'a, 'source> {
    Variants {
        rule,
        support,
        full: inputs.is_empty() && first,
        next: 0,
        inputs,
    }
}

/// A rich producer's variants: its one full traversal when `traversed`, and
/// none otherwise.
pub(super) fn traversal<'a, 'source>(
    rule: &'a RuleIr,
    support: &'a Support<'source>,
    traversed: bool,
) -> Variants<'a, 'source> {
    Variants {
        rule,
        support,
        full: traversed,
        next: 0,
        inputs: &[],
    }
}

#[derive(Clone, Copy)]
pub(super) enum Variant {
    Full,
    Delta(usize),
}

impl Variants<'_, '_> {
    pub(super) fn next(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Option<Variant>, FormulaFailure> {
        if std::mem::take(&mut self.full) {
            return Ok(Some(Variant::Full));
        }
        while let Some(&occurrence) = self.inputs.get(self.next) {
            counters.work(limits, self.rule.location)?;
            let literal = &self.rule.body[occurrence];
            self.next += 1;
            let atom = match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => Some(*atom),
                LiteralIr::PatternAtom(pattern) => Some(pattern.atom),
                _ => None,
            };
            if let Some(atom) = atom {
                let components = self
                    .support
                    .components()
                    .ok_or_else(|| super::components::missing(self.rule.location))?;
                let atom = atom.get(components, limits, counters, self.rule.location)?;
                if self.support.old_rows(atom.predicate())
                    < self.support.row_count(atom.predicate())
                {
                    #[cfg(test)]
                    if matches!(literal, LiteralIr::PatternAtom(_)) {
                        tests::record_structural();
                    }
                    return Ok(Some(Variant::Delta(occurrence)));
                }
            }
        }
        Ok(None)
    }
}

/// Source occurrence, not chosen execution order, determines the partition.
pub(super) fn interval(
    pivot: Option<usize>,
    occurrence: usize,
    old: usize,
    total: usize,
) -> Range<usize> {
    match pivot {
        Some(pivot) if occurrence < pivot => 0..old,
        Some(pivot) if occurrence == pivot => old..total,
        _ => 0..total,
    }
}

/// One prepared input domain. Absence of this owner means the probe is pending.
#[derive(Clone)]
pub(super) enum Rows<'a> {
    Interval(Range<usize>),
    Posting(&'a [usize]),
}

impl<'a> Rows<'a> {
    pub(super) fn all(posting: Option<&'a [usize]>, total: usize) -> Self {
        posting.map_or(Self::Interval(0..total), Self::Posting)
    }

    pub(super) fn within(
        posting: Option<&'a [usize]>,
        range: Range<usize>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let Some(posting) = posting else {
            return Ok(Self::Interval(range));
        };
        let start = lower_bound(posting, range.start, limits, counters, location)?;
        let end = start + lower_bound(&posting[start..], range.end, limits, counters, location)?;
        Ok(Self::Posting(&posting[start..end]))
    }

    pub(super) fn get(&self, position: usize) -> Option<usize> {
        match self {
            Self::Interval(range) => position
                .checked_add(range.start)
                .filter(|&row| row < range.end),
            Self::Posting(posting) => posting.get(position).copied(),
        }
    }
}

fn lower_bound(
    posting: &[usize],
    row: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<usize, FormulaFailure> {
    let mut start = 0;
    let mut end = posting.len();
    while start < end {
        counters.work(limits, location)?;
        let middle = start + (end - start) / 2;
        if posting[middle] < row {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    Ok(start)
}
