//! Successful scalar preparation for one rule in one immutable support round.
//!
//! The first eligible variant owns ordinary preparation and traversal. After it
//! finishes, this owner retains that same successful expression map. Subsequent
//! variants borrow the map but prepare their own join order and multiples target.
//! The owner borrows both rule and support, so it cannot outlive publication.
//! Its presence grants neither source completion nor generic table selection.

use crate::FormulaFailure;
use crate::expansion::Budget;
use crate::formula_ir::RuleIr;
use crate::formula_support::{
    Computation, Context, Join, StorageLease, Support, delta, projections, queries,
};

pub(in crate::formula_support) struct RoundPreparation<'rule, 'source> {
    rule: &'rule RuleIr,
    support: &'rule Support<'source>,
    values: projections::ProjectionValues<'rule>,
    lease: StorageLease,
}

impl<'rule, 'source> RoundPreparation<'rule, 'source> {
    /// Move the first successful cursor's map without cloning its buffers. Its
    /// lease continues to own the map header and payload; this owner charges
    /// only its additional header, including the optional slot's padding.
    /// Refusal publishes no owner and releases the consumed cursor normally.
    pub(in crate::formula_support) fn retain(
        rule: &'rule RuleIr,
        join: Join<'rule, 'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context { computation, work } = context;
        work.counters.work(work.limits, rule.location)?;
        if !std::ptr::eq(join.literals, rule.body.as_slice())
            || join.delta.is_none()
            || join.source_evidence
            || join.certified_total
            || join.multiples.is_none()
        {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            });
        }
        let projections::Projections::Local(values) = join.projections else {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            });
        };
        let mut lease = computation.lease();
        let header = size_of::<Option<Self>>() - size_of::<projections::ProjectionValues<'_>>();
        lease.observe(header, rule.location)?;
        computation.storage_observed(
            &lease,
            0,
            header,
            work.limits,
            work.counters,
            rule.location,
        )?;
        Ok(Self {
            rule,
            support: join.support,
            values,
            lease,
        })
    }

    /// Build a fresh variant with the same rule/source authority. The borrowed
    /// map never grows. A prefix outside its covered domain still uses ordinary
    /// checked evaluation. `select_round_multiples` separately checks whether
    /// this variant's plan binds the divisor before its target.
    pub(in crate::formula_support) fn rows<'a>(
        &'a self,
        variant: delta::Variant,
        domains: Option<&'a queries::Guards<'a, 'source>>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Join<'a, 'source>, FormulaFailure> {
        context
            .work
            .counters
            .work(context.work.limits, self.rule.location)?;
        context
            .computation
            .allowance(&self.lease, context.work.limits, self.rule.location)?;
        Join::variant_rule_with_total(
            self.rule,
            variant,
            self.support,
            domains,
            Some(&self.values),
            budget,
            context,
        )
    }
}
