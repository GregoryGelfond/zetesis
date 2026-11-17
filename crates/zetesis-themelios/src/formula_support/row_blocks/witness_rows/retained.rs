//! One complete, comparison-selected source family; no Activity values are retained.
//!
//! The ordinary Join validates the first traversal. Only successful exhaustion
//! transfers its exact plan and witness tape into this immutable owner.

#[cfg(test)]
mod tests;

use themelios_program::program::DefaultNegation;
use zetesis_core::TemplateTerm;

use super::{
    Buffer, Completion, Computation, Context, Counters, Descriptor, FormulaFailure, FormulaLimits,
    GroundingWork, Join, JoinPlan, Occurrence, StorageLease, Support, WitnessPlan, WitnessRow,
    owner, relations, stage,
};
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::{Coverage, Traversal, order};

pub(crate) struct Recording<'source> {
    rule: &'source RuleIr,
    relations: &'source relations::Relations<'source>,
    patterns: *const (),
    width: usize,
    completions: Buffer<Completion>,
    suffixes: Buffer<Occurrence>,
    lease: StorageLease,
}

/// An enclosing, already reserved source-family cell owns this header after
/// embedding. Its payload never grows again. The original source is borrowed.
pub(crate) struct Family<'plan, 'source> {
    pub(super) rule: &'source RuleIr,
    pub(super) plan: order::Plan<'plan>,
    pub(super) relations: &'source relations::Relations<'source>,
    pub(super) completions: Buffer<Completion>,
    pub(super) suffixes: Buffer<Occurrence>,
    lease: StorageLease,
}

impl<'source> Recording<'source> {
    pub(crate) fn new(
        rule: &'source RuleIr,
        cursor: &Join<'_, 'source>,
        context: Context<'_, &Computation<'_, 'source>>,
    ) -> Result<Option<Self>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        counters.charge_work(2, limits, location)?;
        if !std::ptr::eq(cursor.literals.get(), rule.body.as_slice())
            || !std::ptr::eq(cursor.support, computation.query_support())
        {
            return Err(owner(location));
        }
        counters.work(limits, location)?;
        if cursor.support.strategy() != crate::JoinStrategy::Indexed
            || !matches!(rule.head, HeadIr::Normal(Some(_)))
            || rule.body_variables != rule.variables
            || cursor.plan.patterns.is_empty()
        {
            return Ok(None);
        }
        if !super::super::cursor_rows::initial(
            cursor,
            Coverage::Selected,
            limits,
            counters,
            location,
        )? || !selected_comparisons(rule, cursor, computation, limits, counters)?
        {
            return Ok(None);
        }
        let mut lease = computation.lease();
        let header =
            size_of::<Self>() - size_of::<Buffer<Completion>>() - size_of::<Buffer<Occurrence>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        Ok(Some(Self {
            rule,
            relations: cursor.support.relations(),
            patterns: cursor.plan.patterns.as_ptr().cast(),
            width: cursor.plan.patterns.len(),
            completions: Buffer::new(computation, limits, counters, location)?,
            suffixes: Buffer::new(computation, limits, counters, location)?,
            lease,
        }))
    }

    pub(crate) fn stage(
        &mut self,
        row: &WitnessRow<'_, '_>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        context
            .work
            .counters
            .charge_work(2, context.work.limits, context.work.location)?;
        if row.patterns.as_ptr().cast::<()>() != self.patterns
            || !std::ptr::eq(row.support.relations(), self.relations)
        {
            return Err(owner(context.work.location));
        }
        stage(
            self.width,
            &mut self.completions,
            &mut self.suffixes,
            row,
            context,
        )
    }

    /// This consumes the cursor, dropping all non-plan scratch before reducing
    /// its live lease. A refused/interrupted cursor can never seal a family.
    pub(crate) fn finish<'plan>(
        self,
        cursor: Join<'plan, 'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Family<'plan, 'source>, FormulaFailure>
    where
        'source: 'plan,
    {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        counters.charge_work(6, limits, location)?;
        if !std::ptr::eq(cursor.literals.get(), self.rule.body.as_slice())
            || !std::ptr::eq(cursor.support.relations(), self.relations)
            || cursor.traversal != Traversal::Finished
            || cursor.generated
            || !cursor.head_slots.is_empty()
            || cursor.plan.patterns.as_ptr().cast::<()>() != self.patterns
        {
            return Err(owner(location));
        }
        let Self {
            rule,
            relations,
            completions,
            suffixes,
            lease,
            ..
        } = self;
        drop(lease);
        let (plan, mut lease) = completed_plan(cursor, location)?;
        let header = Family::<'plan, 'source>::header();
        let bytes = usize::try_from(plan.retained_bytes())
            .ok()
            .and_then(|bytes| bytes.checked_add(header))
            .ok_or_else(|| FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Overflow,
                location,
            })?;
        lease.observe(bytes, location)?;
        computation.storage_observed(&lease, 0, bytes, limits, counters, location)?;
        Ok(Family {
            rule,
            plan,
            relations,
            completions,
            suffixes,
            lease,
        })
    }
}

fn completed_plan<'plan>(
    cursor: Join<'plan, '_>,
    location: crate::ProgramSite,
) -> Result<(order::Plan<'plan>, StorageLease), FormulaFailure> {
    let Join { plan, lease, .. } = cursor;
    let JoinPlan::Owned(plan) = plan else {
        return Err(owner(location));
    };
    // Returning ends this consuming helper's scope and drops every other field.
    Ok((plan, lease))
}

impl<'plan, 'source: 'plan> Family<'plan, 'source> {
    fn header() -> usize {
        size_of::<Self>() - size_of::<Buffer<Completion>>() - size_of::<Buffer<Occurrence>>()
    }

    /// The caller has reserved the complete enclosing cell before this transfer.
    /// None of these embedded buffers is grown after publication.
    pub(crate) fn embedded(mut self, location: crate::ProgramSite) -> Result<Self, FormulaFailure> {
        self.lease
            .observe(self.lease.bytes() - Self::header(), location)?;
        self.completions = self.completions.embedded(location)?;
        self.suffixes = self.suffixes.embedded(location)?;
        Ok(self)
    }

    pub(crate) fn view<'view>(
        &'view self,
        rule: &'source RuleIr,
        support: &Support<'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<WitnessPlan<'view, 'source>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        counters.work(limits, location)?;
        if !std::ptr::eq(rule, self.rule) {
            return Err(owner(location));
        }
        WitnessPlan::for_emission(
            Descriptor {
                rule,
                plan: &self.plan,
                relations: self.relations,
                rows: &[],
            },
            support,
            computation,
            limits,
            counters,
        )
    }
}

fn selected_comparisons(
    rule: &RuleIr,
    cursor: &Join<'_, '_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        match literal {
            LiteralIr::Atom(DefaultNegation::None, pattern) => {
                if !super::super::checked_pattern(
                    *pattern,
                    rule.variables,
                    computation,
                    limits,
                    counters,
                    rule.location,
                )? {
                    return Ok(false);
                }
            }
            LiteralIr::Compare(_) => {}
            _ => return Ok(false),
        }
    }
    let HeadIr::Normal(Some(head)) = rule.head else {
        return Ok(false);
    };
    if !super::super::checked_pattern(
        head,
        rule.variables,
        computation,
        limits,
        counters,
        rule.location,
    )? {
        return Ok(false);
    }
    // Every slot comes from an actual immutable source row. Comparisons do not
    // authorize an invented binding or omitted generator continuation.
    for variable in 0..rule.variables {
        let mut found = false;
        for occurrence in &cursor.plan.patterns {
            counters.work(limits, rule.location)?;
            let terms = occurrence.atom().terms();
            for column in 0..terms.len() {
                counters.work(limits, rule.location)?;
                if matches!(terms.at(column), Some(TemplateTerm::Variable(slot)) if slot == variable)
                {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            return Ok(false);
        }
    }
    Ok(true)
}
