//! Necessary multiples over one original closed range, after family admission.
//!
//! The full original interval and complete positive columns establish every
//! scalar intermediate before a cursor may omit a value. No projection table,
//! family evidence, support-round permission or incremental certificate follows.
//! The scalar equality remains in the ordinary filter. Only positive flat body
//! atoms are eligible: rejected rows can name no undiscovered negative atom.
//!
//! Preparation reuses the numeric totality workspace: O(variables + longest
//! expression) leased scratch, plus charged distinct-column scans. The retained
//! descriptor is inline in Join and Cursor; iteration uses constant space and
//! the original ascending order. Every original range width is admitted before
//! selection, and all emitted terms use the ordinary canonical number operation.

#[cfg(test)]
mod tests;

use std::ops::RangeInclusive;

use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::ValueNodeRef;

use super::multiples::{Divisor, number, positive_factors, remainder};
use super::{Computation, Context, Counters, Join, keys, projections};
use crate::formula_binding::Binding;
use crate::formula_ir::{HeadIr, LiteralIr, Operation, RuleIr};
use crate::{FormulaFailure, FormulaLimits};

/// Exact literal identity and divisor readiness belong to this rule's cursor.
/// Construction remains private to successful full-domain preparation below.
#[derive(Clone, Copy)]
pub(crate) struct Prepared<'a> {
    literal: &'a LiteralIr,
    divisor: Divisor,
}

struct Candidate<'a> {
    prepared: Prepared<'a>,
    target: usize,
    bounds: (i32, i32),
}

fn tick<C>(context: &mut Context<'_, C>) -> Result<(), FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)
}

impl<'a> Join<'a, '_> {
    /// Opt in only after original-family validation, immediately before ordinary
    /// rule emission. The completed support, exact rule and full arithmetic hull
    /// are authenticated here; no broad join/coverage flag changes on success.
    pub(crate) fn select_range_multiples(
        &mut self,
        rule: &'a RuleIr,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        #[cfg(test)]
        if tests::disabled() {
            return Ok(false);
        }
        if !self.generated || self.source_evidence || self.delta.is_some() || self.pending.is_some()
        {
            return Ok(false);
        }
        if !std::ptr::eq(self.literals, rule.body.as_slice()) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            });
        }
        let Some(candidate) = candidate(
            rule,
            &mut Context::new(&*computation, limits, counters, rule.location),
        )?
        else {
            return Ok(false);
        };
        if !keys::covered_with_range(
            rule,
            &self.plan,
            Some(candidate.target),
            &mut Context::new(&*computation, limits, counters, rule.location),
        )? {
            return Ok(false);
        }
        let divisor = match candidate.prepared.divisor {
            Divisor::Constant(_) => None,
            Divisor::Variable(slot) => Some(slot),
        };
        if !projections::range_body(
            rule,
            self.support,
            candidate.target,
            candidate.bounds,
            divisor,
            Context::new(computation, limits, counters, rule.location),
        )? {
            return Ok(false);
        }
        // Cancellation or refusal before this final charged boundary publishes
        // no selection. Temporary numeric buffers have already been released.
        counters.work(limits, rule.location)?;
        self.range_selection = Some(candidate.prepared);
        #[cfg(test)]
        tests::selected();
        Ok(true)
    }
}

fn candidate<'a>(
    rule: &'a RuleIr,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<Candidate<'a>>, FormulaFailure> {
    if !matches!(rule.head, HeadIr::Normal(_)) || rule.variables != rule.body_variables {
        return Ok(None);
    }
    let mut range = None;
    for literal in &rule.body {
        tick(context)?;
        match literal {
            LiteralIr::Range {
                target,
                lower,
                upper,
                binder: true,
            } if range.is_none() => {
                let ([Operation::Constant(lower)], [Operation::Constant(upper)]) =
                    (lower.nodes.as_slice(), upper.nodes.as_slice())
                else {
                    return Ok(None);
                };
                let (Some(lower), Some(upper)) =
                    (number(*lower, context)?, number(*upper, context)?)
                else {
                    return Ok(None);
                };
                if lower <= 0 || upper < lower {
                    return Ok(None);
                }
                range = Some((literal, *target, (lower, upper)));
            }
            LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::Compare(..) => {}
            _ => return Ok(None),
        }
    }
    let Some((literal, target, bounds)) = range else {
        return Ok(None);
    };
    for comparison in &rule.body {
        tick(context)?;
        let LiteralIr::Compare(left, Relation::Eq, right) = comparison else {
            continue;
        };
        for (expression, zero) in [(left, right), (right, left)] {
            if let Some((dividend, divisor)) = remainder(expression, zero, context)?
                && dividend == target
                // A constant-one equality cannot remove any range value. Keep
                // looking for a selective remainder before allocating evidence.
                && !matches!(divisor, Divisor::Constant(1))
                && !matches!(divisor, Divisor::Variable(slot) if slot == target)
            {
                return Ok(Some(Candidate {
                    prepared: Prepared { literal, divisor },
                    target,
                    bounds,
                }));
            }
        }
    }
    Ok(None)
}

impl Prepared<'_> {
    /// The original endpoint evaluation and width admission precede this call.
    /// A constant-one divisor keeps the existing cursor. Variable divisors are
    /// re-read on every relational restart; no row's value survives backtracking.
    pub(crate) fn factors(
        self,
        literal: &LiteralIr,
        bounds: &RangeInclusive<i32>,
        binding: &Binding<'_>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<(RangeInclusive<i64>, i32)>, FormulaFailure> {
        tick(&mut context)?;
        if !std::ptr::eq(self.literal, literal) {
            return Ok(None);
        }
        let divisor = match self.divisor {
            Divisor::Constant(value) => value,
            Divisor::Variable(slot) => {
                tick(&mut context)?;
                let value =
                    binding.read(slot, context.computation.read(), context.work.location)?;
                let ValueNodeRef::Number(value) = value.descriptor() else {
                    return Ok(None);
                };
                value
            }
        };
        if divisor <= 1 {
            return Ok(None);
        }
        #[cfg(test)]
        tests::stepped();
        Ok(Some((
            positive_factors((*bounds.start(), *bounds.end()), divisor),
            divisor,
        )))
    }
}
