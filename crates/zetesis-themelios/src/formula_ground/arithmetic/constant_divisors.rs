//! A flat constraint with literal nonzero divisors needs no zero-family pass.
//!
//! This gate alone does not establish totality. Ordinary complete-row filters
//! preserve independent fatal failures unless a separate certificate proves
//! all checked expressions defined over every complete binding.

use themelios_program::term::BinaryOp;
use zetesis_core::ValueNodeRef;

use crate::FormulaFailure;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, Prepared, RuleIr};
use crate::formula_support::{Computation, Context};

/// Every normalized fragment at the original source location must satisfy
/// this gate. One unchecked sibling keeps the whole family pass. The existing
/// syntactic `family::partial` classification is never changed.
pub(super) fn deferred(
    rule: &RuleIr,
    prepared: &Prepared,
    mut context: Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    for fragment in prepared.rules.iter().chain(&prepared.projection) {
        context
            .work
            .counters
            .work(context.work.limits, context.work.location)?;
        if fragment.location == rule.location && !constraint(fragment, &mut context)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn constraint(
    rule: &RuleIr,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)?;
    if !matches!(rule.head, HeadIr::Normal(None)) || rule.variables != rule.body_variables {
        return Ok(false);
    }
    let mut has_divisor = false;
    for literal in &rule.body {
        context
            .work
            .counters
            .work(context.work.limits, context.work.location)?;
        let safe = match literal {
            LiteralIr::Atom(..) | LiteralIr::PatternAtom(_) => true,
            LiteralIr::Compare(left, _, right)
            | LiteralIr::ArgumentCheck {
                captured: left,
                value: right,
            } => {
                expression(left, &mut has_divisor, context)?
                    && expression(right, &mut has_divisor, context)?
            }
            LiteralIr::TupleCompare(left, _, right) => {
                expressions(left.iter().chain(right), &mut has_divisor, context)?
            }
            LiteralIr::Guard(guard) | LiteralIr::HeadGuard(guard) => {
                expressions(guard.expressions(), &mut has_divisor, context)?
            }
            // Generated values and nested scopes retain authoritative family
            // traversal. Their ordinary consumers need a separate coverage
            // argument before any hard-error validation can be deferred.
            _ => false,
        };
        if !safe {
            return Ok(false);
        }
    }
    // Syntactic partiality keeps ordinary coverage complete by default. Only
    // a separate successful totality certificate permits ordinary selection;
    // this gate alone never permits pruning. Arithmetic also excludes factoring.
    Ok(has_divisor)
}

fn expressions<'a>(
    values: impl Iterator<Item = &'a Expression>,
    has_divisor: &mut bool,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    for value in values {
        if !expression(value, has_divisor, context)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn expression(
    expression: &Expression,
    has_divisor: &mut bool,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    for node in &expression.nodes {
        context
            .work
            .counters
            .work(context.work.limits, context.work.location)?;
        let Operation::Binary(BinaryOp::Div | BinaryOp::Mod, _, right) = node else {
            continue;
        };
        *has_divisor = true;
        let Some(Operation::Constant(scalar)) = expression.nodes.get(*right) else {
            return Ok(false);
        };
        let value = context.computation.static_scalar(
            *scalar,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        if !matches!(value.descriptor(), ValueNodeRef::Number(value) if value != 0) {
            return Ok(false);
        }
    }
    Ok(true)
}
