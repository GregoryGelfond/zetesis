//! Flat atom keys covered by the complete positive join.

use zetesis_core::TemplateTerm;

use super::{Buffer, Computation, Context, PositivePattern, order};
use crate::FormulaFailure;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};

fn tick(context: &mut Context<'_, &Computation<'_, '_>>) -> Result<(), FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)
}

/// Whole flat keys, including negative literals and a plain head, read only
/// admitted constants or slots supplied by the complete positive join. No head
/// evaluation occurs; false-body head suppression remains with the caller.
pub(super) fn covered(
    rule: &RuleIr,
    plan: &order::Plan<'_>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    covered_with_range(rule, plan, None, context)
}

/// A single authenticated range binder may supply a slot absent from every
/// positive pattern. All other key slots still require the complete join.
pub(super) fn covered_with_range(
    rule: &RuleIr,
    plan: &order::Plan<'_>,
    generated: Option<usize>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    let mut bound = Buffer::new(
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )?;
    bound.resize(
        rule.body_variables,
        false,
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )?;
    for occurrence in &plan.patterns {
        tick(context)?;
        let PositivePattern::Flat(pattern) = occurrence.pattern else {
            return Ok(false);
        };
        for column in 0..pattern.terms().len() {
            tick(context)?;
            if let Some(TemplateTerm::Variable(slot)) = pattern.terms().at(column) {
                let Some(entry) = bound.slice_mut().get_mut(slot) else {
                    return Ok(false);
                };
                if Some(slot) == generated {
                    return Ok(false);
                }
                *entry = true;
            }
        }
    }
    if let Some(slot) = generated {
        tick(context)?;
        let Some(entry) = bound.slice_mut().get_mut(slot) else {
            return Ok(false);
        };
        *entry = true;
    }
    let HeadIr::Normal(head) = &rule.head else {
        return Ok(false);
    };
    for atom in rule
        .body
        .iter()
        .map(|literal| match literal {
            LiteralIr::Atom(_, atom) => Some(*atom),
            _ => None,
        })
        .chain(head.iter().copied().map(Some))
    {
        tick(context)?;
        let Some(atom) = atom else {
            continue;
        };
        let pattern = context.computation.static_pattern(
            atom,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        for column in 0..pattern.terms().len() {
            tick(context)?;
            if matches!(pattern.terms().at(column), Some(TemplateTerm::Variable(slot))
                if !bound.slice().get(slot).copied().unwrap_or(false))
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
