//! Disjoint first-new-occurrence joins for certified possible-head producers.
//!
//! Every new positive tuple combination has a unique first occurrence whose
//! row was appended in the previous round. Earlier occurrences use old rows,
//! that occurrence uses new rows, and later occurrences use all current rows.
//! Scalar checks and generators remain with the complete existing matcher.
//! Ordinary negative atoms are non-inputs: their truth remains in the emitted
//! formula, while their already-bound arguments supply no support restriction.
//! Rich producers retain full-round traversal. This scheduling certificate is
//! not a certificate of authored-body validation during final formula emission.

#[cfg(test)]
mod tests;

use std::ops::Range;

use crate::ProgramSite;
use themelios_program::program::DefaultNegation;

use super::{Counters, Support};
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::{FormulaFailure, FormulaLimits};

pub(super) struct Variants<'a, 'source> {
    rule: &'a RuleIr,
    support: &'a Support<'source>,
    full: bool,
    certified: bool,
    next: usize,
    prepared: Option<&'a [usize]>,
}

pub(super) fn variants<'a, 'source>(
    rule: &'a RuleIr,
    support: &'a Support<'source>,
    first: bool,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Variants<'a, 'source>, FormulaFailure> {
    let mut certified = matches!(rule.head, HeadIr::Normal(Some(_)));
    let mut inputs = 0;
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        match literal {
            LiteralIr::Atom(DefaultNegation::None, _) => inputs += 1,
            // Join::new collects only positive inputs. Flat negative atoms
            // neither generate bindings nor filter possible heads; any argument
            // evaluation is a separate, retained scalar operation in the IR.
            LiteralIr::Atom(DefaultNegation::Not | DefaultNegation::NotNot, _)
            | LiteralIr::Compare(..)
            | LiteralIr::ArgumentCheck { .. }
            | LiteralIr::TupleCompare(..)
            | LiteralIr::Guard(_)
            | LiteralIr::HeadGuard(_)
            | LiteralIr::Bind { .. }
            | LiteralIr::Range { .. } => {}
            LiteralIr::PatternAtom(_)
            | LiteralIr::ProjectedAtom(..)
            | LiteralIr::Conditional(_)
            | LiteralIr::Aggregate(_) => certified = false,
        }
    }
    Ok(Variants {
        rule,
        support,
        full: !certified || (inputs == 0 && first),
        certified,
        next: 0,
        prepared: None,
    })
}

/// The producer plan has already checked the complete positive-flat rule and
/// preserves these original body occurrences. No per-round classification scan.
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
        certified: true,
        next: 0,
        prepared: Some(inputs),
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
        if self.certified {
            loop {
                let occurrence = match self.prepared {
                    Some(inputs) => inputs.get(self.next).copied(),
                    None => (self.next < self.rule.body.len()).then_some(self.next),
                };
                let Some(occurrence) = occurrence else { break };
                counters.work(limits, self.rule.location)?;
                let literal = &self.rule.body[occurrence];
                self.next += 1;
                if let LiteralIr::Atom(DefaultNegation::None, atom) = literal {
                    let components = self
                        .support
                        .components()
                        .ok_or_else(|| super::components::missing(self.rule.location))?;
                    let atom = atom.get(components, limits, counters, self.rule.location)?;
                    if self.support.old_rows(atom.predicate())
                        < self.support.row_count(atom.predicate())
                    {
                        return Ok(Some(Variant::Delta(occurrence)));
                    }
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
