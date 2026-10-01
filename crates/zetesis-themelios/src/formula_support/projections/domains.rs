//! Necessary finite input domains derived from covered successful projections.
//!
//! A completed source column covers every possible complete binding of its
//! variable. Reversing that finite input/result mapping for one known equality
//! operand therefore preserves every matching complete row. The maps remain
//! the sole expression results; this operation owns only transient borrowed
//! domain lists for the existing finite-table selector.

use std::ops::Range;

use zetesis_core::catalog::{TermKey, TermRef};
use zetesis_core::{PatternRef, TemplateTerm};

use super::{Projection, Projections, tick};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::{
    Buffer, Computation, Context, Join, PositivePattern, Probe, delta, queries,
    relations::RelationRows,
};
use themelios_program::program::Relation;

pub(in crate::formula_support) struct Domains<'read> {
    values: Buffer<TermRef<'read>>,
    variables: Buffer<(usize, Range<usize>)>,
}

impl Domains<'_> {
    pub(in crate::formula_support) fn borrowed(&self) -> queries::FiniteDomains<'_> {
        queries::FiniteDomains {
            values: self.values.slice(),
            variables: self.variables.slice(),
        }
    }

    fn excludes_all<C>(&self, context: &mut Context<'_, C>) -> Result<bool, FormulaFailure> {
        for (_, range) in self.variables.iter() {
            tick(context)?;
            if range.is_empty() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl<'a, 'source> Join<'a, 'source> {
    pub(in crate::formula_support) fn computed_probe(
        &self,
        pattern: PositivePattern<'_>,
        source: Option<&'source RelationRows<'source>>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Probe<'a, 'source>>, FormulaFailure> {
        if !self.certified_total || source.is_none() {
            return Ok(None);
        }
        let PositivePattern::Flat(flat) = pattern else {
            return Ok(None);
        };
        let Some(domains) = self.projections.domains(
            self.literals,
            flat,
            &self.values,
            Context::new(
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            ),
        )?
        else {
            return Ok(None);
        };
        if domains.excludes_all(&mut context)? {
            return Ok(Some(Probe::Indexed(delta::Rows::Posting(&[]))));
        }
        let binding = self.values.view(
            context.computation.read(),
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        self.support
            .select_domains_at(source, pattern, binding, domains.borrowed(), context.work)
            .map(|rows| rows.map(Probe::Table))
    }
}

impl Projections<'_> {
    /// Called only under the complete constraint's totality witness. At most
    /// one restricting equality per unbound variable is selected; all remaining
    /// comparisons stay with the ordinary residual prefix/full-row checks.
    pub(in crate::formula_support) fn domains<'read>(
        &self,
        literals: &[LiteralIr],
        pattern: PatternRef<'_>,
        binding: &Binding<'_>,
        mut context: Context<'_, &'read Computation<'_, '_>>,
    ) -> Result<Option<Domains<'read>>, FormulaFailure> {
        context
            .computation
            .allowance(&self.lease, context.work.limits, context.work.location)?;
        let mut domains: Option<Domains<'read>> = None;
        for projection in &self.values {
            tick(&mut context)?;
            if projection.covered.is_none()
                || binding.is_bound(projection.slot, context.work.location)?
                || !occurs(pattern, projection.slot, &mut context)?
                || already_selected(domains.as_ref(), projection.slot, &mut context)?
            {
                continue;
            }
            let LiteralIr::Compare(left, Relation::Eq, right) = &literals[projection.literal]
            else {
                continue;
            };
            let other = 1 - projection.side;
            let expression = [left, right][other];
            let Some(wanted) = self.known(
                (projection.literal, other),
                expression,
                binding,
                &mut context,
            )?
            else {
                continue;
            };
            let wanted = context.computation.read().term(&wanted).map_err(|error| {
                crate::formula_binding::assignment(error.into(), context.work.location)
            })?;
            if !projection.restricts(wanted, &mut context)? {
                continue;
            }
            if domains.is_none() {
                domains = Some(Domains {
                    values: Buffer::new(
                        context.computation,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )?,
                    variables: Buffer::new(
                        context.computation,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )?,
                });
            }
            let domains = domains
                .as_mut()
                .expect("a restricting equality owns its lists");
            let start = domains.values.len();
            for position in 0..projection.covered.expect("covered projection checked") {
                if projection.matches(position, wanted, &mut context)? {
                    let input = projection.inputs.value(
                        position,
                        context.computation.read(),
                        context.work.location,
                    )?;
                    domains.values.push(
                        input,
                        context.computation,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )?;
                }
            }
            domains.variables.push(
                (projection.slot, start..domains.values.len()),
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
        }
        Ok(domains)
    }

    /// A cache miss supplies no restriction. In particular, a reached prefix
    /// input need not belong to the covering column of a later occurrence.
    fn known(
        &self,
        source: (usize, usize),
        expression: &Expression,
        binding: &Binding<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<TermKey>, FormulaFailure> {
        tick(context)?;
        match expression.nodes.as_slice() {
            [Operation::Constant(value)] => {
                return context
                    .computation
                    .static_key(
                        *value,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )
                    .map(Some);
            }
            [Operation::Variable(slot)] if binding.is_bound(*slot, context.work.location)? => {
                return binding.key(*slot, context.work.location).map(Some);
            }
            _ => {}
        }
        let Some(index) = self.find(source, context)? else {
            return Ok(None);
        };
        let projection = &self.values[index];
        let Some(covered) = projection.covered else {
            return Ok(None);
        };
        if !binding.is_bound(projection.slot, context.work.location)? {
            return Ok(None);
        }
        let input = binding.key(projection.slot, context.work.location)?;
        let Some(position) = projection.inputs.find(
            &input,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?
        else {
            return Ok(None);
        };
        if position >= covered {
            return Ok(None);
        }
        projection
            .results
            .key(position, context.work.location)
            .map(Some)
    }
}

impl Projection<'_> {
    /// If every covered input is allowed, keep the ordinary probe and avoid
    /// constructing a table merely because source totality was established.
    fn restricts(
        &self,
        wanted: TermRef<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        for position in 0..self.covered.expect("covered projection checked") {
            if !self.matches(position, wanted, context)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn matches(
        &self,
        position: usize,
        wanted: TermRef<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        tick(context)?;
        let value =
            self.results
                .read(position, context.computation.read(), context.work.location)?;
        crate::formula_support::compare(
            value,
            Relation::Eq,
            wanted,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )
    }
}

fn occurs<C>(
    pattern: PatternRef<'_>,
    slot: usize,
    context: &mut Context<'_, C>,
) -> Result<bool, FormulaFailure> {
    let terms = pattern.terms();
    for column in 0..terms.len() {
        tick(context)?;
        if matches!(terms.at(column), Some(TemplateTerm::Variable(variable)) if variable == slot) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn already_selected<C>(
    domains: Option<&Domains<'_>>,
    slot: usize,
    context: &mut Context<'_, C>,
) -> Result<bool, FormulaFailure> {
    if let Some(domains) = domains {
        for (variable, _) in domains.variables.iter() {
            tick(context)?;
            if *variable == slot {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
