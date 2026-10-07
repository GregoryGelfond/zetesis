//! Exact positive witnesses remain lent until the existing join undo boundary.
//!
//! This state records actual relation rows, never posting cursors or recovered
//! tuples. It publishes no formula information. The consumer may retain a
//! derived prefix only through the reported unchanged prefix of the next row.

#[cfg(test)]
mod tests;

use themelios_program::program::DefaultNegation;
use zetesis_core::relation::Row as MatchedRow;

use super::{
    AtomPattern, Computation, Context, Counters, Event, FormulaFailure, FormulaLimits, HeadIr,
    Join, LiteralIr, Ownership, PatternOccurrence, Resolution, RowHead, reserve,
};
use crate::expansion::Budget;
use crate::formula_ir::RuleIr;

pub(super) struct Witnesses<'source> {
    rows: Vec<MatchedRow<'source, 'source>>,
    unchanged: usize,
}

impl<'source> Witnesses<'source> {
    pub(super) fn capacity_bytes(&self) -> usize {
        self.rows.capacity() * size_of::<MatchedRow<'source, 'source>>()
    }

    pub(super) fn push(&mut self, depth: usize, row: MatchedRow<'source, 'source>) {
        debug_assert_eq!(self.rows.len(), depth);
        debug_assert!(self.rows.len() < self.rows.capacity());
        self.rows.push(row);
    }

    pub(super) fn removed_at(&self, depth: usize) -> usize {
        self.rows.len().saturating_sub(depth)
    }

    pub(super) fn undo(&mut self, depth: usize) {
        self.rows.truncate(depth);
        self.unchanged = self.unchanged.min(depth);
    }
}

/// The exact ordered row occurrences of a selected complete binding. The join
/// cannot advance while this loan remains in use.
pub(crate) struct WitnessRow<'row, 'source> {
    pub(crate) rows: &'row [MatchedRow<'source, 'source>],
    pub(crate) unchanged: usize,
    pub(super) patterns: &'row [PatternOccurrence<'row>],
    pub(super) support: &'row super::Support<'source>,
    pub(super) resolutions: &'row [Resolution<'source>],
}

/// Only total flat positive bodies share this traversal. In particular, a
/// normal nullary head is eligible; a missing head is a constraint. One body
/// occurrence has no repeated prefix to share and uses the ordinary route.
pub(crate) fn eligible(
    rule: &RuleIr,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<AtomPattern>, FormulaFailure> {
    let HeadIr::Normal(Some(head)) = &rule.head else {
        return Ok(None);
    };
    if rule.body.len() < 2 || rule.body_variables != rule.variables {
        return Ok(None);
    }
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        if !matches!(literal, LiteralIr::Atom(DefaultNegation::None, _)) {
            return Ok(None);
        }
    }
    Ok(Some(*head))
}

impl<'source> Join<'_, 'source> {
    pub(crate) fn retain_witnesses(
        &mut self,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context {
            computation,
            work:
                super::GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        debug_assert!(!self.generated && self.head_slots.is_empty());
        debug_assert_eq!(self.plan.patterns.len(), self.literals.len());
        let other =
            self.storage_bytes() - self.witnesses.as_ref().map_or(0, Witnesses::capacity_bytes);
        let witnesses = self.witnesses.get_or_insert_with(|| Witnesses {
            rows: Vec::new(),
            unchanged: 0,
        });
        let additional = self.plan.patterns.len() - witnesses.rows.len();
        // The joined owner retains any capacity admitted before a refusal.
        // A retry checks the same buffer rather than losing its live receipt.
        reserve(
            &mut witnesses.rows,
            additional,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        self.lease.observe(self.storage_bytes(), location)?;
        Ok(())
    }

    /// `projected` is present only for support production. Formula emission
    /// passes None and therefore retains every distinct positive activation.
    pub(crate) fn next_witness_row(
        &mut self,
        projected: Option<AtomPattern>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<WitnessRow<'_, 'source>>, FormulaFailure> {
        if self
            .next_selected(Ownership::Lend, projected, budget, context)?
            .is_none()
        {
            return Ok(None);
        }
        let witnesses = self.witnesses.as_mut().expect("enabled positive witnesses");
        debug_assert_eq!(witnesses.rows.len(), self.plan.patterns.len());
        let unchanged = std::mem::replace(&mut witnesses.unchanged, witnesses.rows.len());
        Ok(Some(WitnessRow {
            rows: &witnesses.rows,
            unchanged,
            patterns: &self.plan.patterns,
            resolutions: &self.resolutions,
            support: self.support,
        }))
    }
}

/// The same projection capability serves possible-support and formula heads.
/// Preparation remains after the first selected complete descendant, so an
/// empty prefix introduces no new head validation or publication.
pub(super) fn derive(
    rule: &RuleIr,
    outer: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    let Some(pattern) = eligible(rule, limits, counters)? else {
        return Ok(false);
    };
    outer.retain_witnesses(Context::new(computation, limits, counters, rule.location))?;
    let mut head = None;
    while let Some(row) = counters.observe_work(Event::SupportJoinWork, |counters| {
        outer.next_witness_row(
            Some(pattern),
            budget,
            Context::new(computation, limits, counters, rule.location),
        )
    })? {
        counters.observe_work(Event::SupportHeadWork, |counters| {
            if head.is_none() {
                head = Some(RowHead::new(
                    pattern,
                    &row,
                    computation,
                    limits,
                    counters,
                    rule.location,
                )?);
            }
            let atom = head.as_ref().expect("prepared positive head").atom(
                &row,
                computation,
                limits,
                counters,
                rule.location,
            )?;
            computation.support(&atom, limits, counters, rule.location)
        })?;
    }
    Ok(true)
}
