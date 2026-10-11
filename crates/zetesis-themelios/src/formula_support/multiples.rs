//! Bounded positive multiples over one admitted relation column.
//!
//! Completed body totality precedes preparation. One flat remainder equality
//! supplies a necessary domain; other scalar comparisons remain residual. All atom-key
//! variables must be supplied by the positive join. Evidence and support-delta
//! cursors never consume this operation. No row or value is manufactured.
//!
//! Bounds name the target column, not another occurrence of the same variable.
//! A fixed plan must bind the divisor before the dividend. Each query generates
//! M possible values, borrows their canonical postings, and orders K actual row
//! IDs with O(K log K) checked work and O(K) leased space. Sparse wide intervals
//! decline before generation. Existing computed/table probes have priority; a
//! union that reaches the ordinary posting length is discarded before sorting.
//! This bounds retained rows, not the total cost of an unsuccessful attempt. Ordinary
//! matching and the original scalar comparison still check every offered row.

#[cfg(test)]
mod tests;

use themelios_program::program::Relation;
use themelios_program::term::BinaryOp;
use zetesis_core::catalog::{Atoms, TermRef};
use zetesis_core::{TemplateTerm, Value, ValueNodeRef};

use super::{
    Buffer, Computation, Context, GroundingWork, Join, PositivePattern, Probe, Support, components,
    delta, order, projections,
    relations::{RelationRows, Relations},
    sort,
};
use crate::FormulaFailure;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, RuleIr};

#[derive(Clone, Copy)]
enum Divisor {
    Constant(i32),
    Variable(usize),
}

/// Borrowed immutable evidence; its inline size belongs to the join/rule header.
#[derive(Clone, Copy)]
pub(super) struct Prepared<'source> {
    source: Atoms<'source>,
    depth: usize,
    column: usize,
    dividend: usize,
    divisor: Divisor,
    bounds: (i32, i32),
}

impl<'source> Prepared<'source> {
    /// Called only after complete body totality on this same rule and support.
    /// Decline means no selection was published; typed refusals propagate.
    pub(super) fn new(
        rule: &RuleIr,
        plan: &order::Plan<'_>,
        relations: &Relations<'source>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Self>, FormulaFailure> {
        let Some((dividend, divisor)) = comparison(rule, &mut context)? else {
            return Ok(None);
        };
        if !super::keys::covered(rule, plan, &mut context)? {
            return Ok(None);
        }
        let mut divisor_bound = matches!(divisor, Divisor::Constant(_));
        for (depth, occurrence) in plan.patterns.iter().enumerate() {
            tick(&mut context)?;
            let PositivePattern::Flat(pattern) = occurrence.pattern else {
                return Ok(None);
            };
            let mut next_divisor = divisor_bound;
            let mut target = None;
            for column in 0..pattern.terms().len() {
                tick(&mut context)?;
                if let Some(TemplateTerm::Variable(slot)) = pattern.terms().at(column) {
                    if slot == dividend {
                        target = Some(column);
                    }
                    if matches!(divisor, Divisor::Variable(input) if input == slot) {
                        next_divisor = true;
                    }
                }
            }
            if let Some(column) = target {
                // The first dividend occurrence binds X; a divisor supplied by
                // that same row is not bound before this probe.
                if !divisor_bound {
                    return Ok(None);
                }
                let Some(source) = relations.find_with(
                    pattern.predicate(),
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )?
                else {
                    return Ok(None);
                };
                let Some(bounds) = projections::positive_bounds(source, column, &mut context)?
                else {
                    return Ok(None);
                };
                return Ok(Some(Self {
                    source: source.atoms,
                    depth,
                    column,
                    dividend,
                    divisor,
                    bounds,
                }));
            }
            divisor_bound = next_divisor;
        }
        Ok(None)
    }
}

fn tick<C>(context: &mut Context<'_, C>) -> Result<(), FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)
}

fn comparison(
    rule: &RuleIr,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<(usize, Divisor)>, FormulaFailure> {
    if !matches!(rule.head, HeadIr::Normal(_)) || rule.variables != rule.body_variables {
        return Ok(None);
    }
    let mut selected = None;
    for literal in &rule.body {
        tick(context)?;
        match literal {
            LiteralIr::Compare(left, Relation::Eq, right) if selected.is_none() => {
                selected = match remainder(left, right, context)? {
                    Some(found) => Some(found),
                    None => remainder(right, left, context)?,
                };
            }
            // Complete totality covers every comparison. Existing computed/table
            // domains retain priority; all remaining comparisons still filter rows.
            LiteralIr::Atom(..) | LiteralIr::Compare(..) => {}
            _ => return Ok(None),
        }
    }
    Ok(selected)
}

fn remainder(
    expression: &Expression,
    zero: &Expression,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<(usize, Divisor)>, FormulaFailure> {
    tick(context)?;
    let [Operation::Constant(zero)] = zero.nodes.as_slice() else {
        return Ok(None);
    };
    let [
        Operation::Variable(dividend),
        divisor,
        Operation::Binary(BinaryOp::Mod, 0, 1),
    ] = expression.nodes.as_slice()
    else {
        return Ok(None);
    };
    if number(*zero, context)? != Some(0) {
        return Ok(None);
    }
    let divisor = match divisor {
        Operation::Variable(slot) => Divisor::Variable(*slot),
        Operation::Constant(value) => {
            let Some(value) = number(*value, context)?.filter(|value| *value > 0) else {
                return Ok(None);
            };
            Divisor::Constant(value)
        }
        _ => return Ok(None),
    };
    Ok(Some((*dividend, divisor)))
}

fn number(
    value: components::Scalar,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<i32>, FormulaFailure> {
    let value = context.computation.static_scalar(
        value,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )?;
    Ok(match value.descriptor() {
        ValueNodeRef::Number(value) => Some(value),
        _ => None,
    })
}

impl<'a, 'source> Join<'a, 'source> {
    /// Selection is useful only at the authenticated target depth. The ordinary
    /// posting remains preferable if it offers no more rows than generation.
    pub(super) fn multiples_probe(
        &self,
        source: Option<&'source RelationRows<'source>>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Probe<'a, 'source>>, FormulaFailure> {
        let Some(prepared) = self
            .multiples
            .filter(|prepared| prepared.depth == self.depth)
        else {
            return Ok(None);
        };
        if !self.certified_total || self.source_evidence || self.delta.is_some() {
            return Ok(None);
        }
        tick(&mut context)?;
        let source = source
            .filter(|source| prepared.source.same_occurrences(source.atoms))
            .ok_or(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: context.work.location,
            })?;
        let read = context.computation.read();
        let binding = self.values.view(
            read,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        let pattern = self.plan.patterns[self.depth];
        let posting = self.support.probe_at(
            Some(source),
            pattern.atom(),
            binding,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        let ordinary = self.indexed_probe(
            posting,
            Some(source),
            pattern.source,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        if self
            .values
            .is_bound(prepared.dividend, context.work.location)?
        {
            return Ok(Some(ordinary));
        }
        let divisor = match prepared.divisor {
            Divisor::Constant(value) => Some(value),
            Divisor::Variable(slot) => {
                tick(&mut context)?;
                match binding.get(slot).map(TermRef::descriptor) {
                    Some(ValueNodeRef::Number(value)) if value > 0 => Some(value),
                    _ => None,
                }
            }
        };
        let Some(divisor) = divisor else {
            return Ok(Some(ordinary));
        };
        let offered = match &ordinary {
            Probe::Indexed(delta::Rows::Interval(range)) => range.len(),
            Probe::Indexed(delta::Rows::Posting(rows)) => rows.len(),
            Probe::Table(_) | Probe::Owned(_) => unreachable!("indexed_probe returns indexed rows"),
        };
        match prepared.posting(source, divisor, offered, self.support, context)? {
            Some(rows) => Ok(Some(Probe::Owned(rows))),
            None => Ok(Some(ordinary)),
        }
    }
}

impl Prepared<'_> {
    fn posting<'source>(
        self,
        source: &'source RelationRows<'source>,
        divisor: i32,
        offered: usize,
        support: &Support<'source>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Buffer<usize>>, FormulaFailure> {
        let divisor = i64::from(divisor);
        let (low, high) = self.bounds;
        let first = (i64::from(low) + divisor - 1) / divisor;
        let last = i64::from(high) / divisor;
        let count = if first > last { 0 } else { last - first + 1 };
        let count = usize::try_from(count).expect("positive i32 interval count fits usize");
        tick(&mut context)?;
        if count >= offered {
            return Ok(None);
        }
        let mut selected = Buffer::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        for multiple in first..=last {
            tick(&mut context)?;
            let value = Value::Number(i32::try_from(multiple * divisor).expect("bounded multiple"));
            let rows = support
                .probe_value(
                    source,
                    self.column,
                    (&value).into(),
                    GroundingWork::new(
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    ),
                )?
                .expect("prepared target retains its column posting");
            tick(&mut context)?;
            // Distinct values have disjoint postings, but one value can occur
            // in many full tuples. Avoid retaining/sorting a union no smaller
            // than the ordinary posting. Earlier lookup work stays charged.
            if rows.len() >= offered - selected.len() {
                return Ok(None);
            }
            for &row in rows {
                selected.push(
                    row,
                    context.computation,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )?;
            }
        }
        let location = context.work.location;
        sort::by(selected.slice_mut(), context.work, |left, right, _| {
            Ok(left.cmp(right))
        })?;
        Ok(Some(selected.embedded(location)?))
    }
}
