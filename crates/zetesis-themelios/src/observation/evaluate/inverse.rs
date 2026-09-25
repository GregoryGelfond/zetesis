//! Invert one admitted arithmetic path, then check the authored expression.
//!
//! Inverse calculations are widened only to find an ordinary scalar candidate.
//! Nonintegral/out-of-range candidates do not match. Authored arithmetic still
//! uses the common checked evaluator after capture; no failed output escapes.

use themelios_program::term::BinaryOp;

use super::{Binding, Error, ErrorKind, Interpreter, Template, UnaryOp, Work};

fn contains(term: &Template, slot: usize, work: &mut Work<'_>) -> Result<bool, Error> {
    work.step(1)?;
    Ok(match term {
        Template::Variable(found) => *found == slot,
        Template::Unary(_, inner) => contains(inner, slot, work)?,
        Template::Binary(_, left, right) => {
            contains(left, slot, work)? || contains(right, slot, work)?
        }
        _ => false,
    })
}
fn candidate<'input>(
    term: &Template,
    slot: usize,
    target: i128,
    binding: &Binding<'input>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<Option<i128>, Error> {
    context.work.step(1)?;
    match term {
        Template::Variable(found) if *found == slot => Ok(Some(target)),
        Template::Unary(UnaryOp::Negate, inner) => {
            candidate(inner, slot, -target, binding, context)
        }
        Template::Binary(operator, left, right) => {
            let unknown_left = contains(left, slot, context.work)?;
            let (unknown, known) = if unknown_left {
                (left, right)
            } else {
                (right, left)
            };
            let known = i128::from(context.numeric(known, binding)?);
            let target = match operator {
                BinaryOp::Add => target - known,
                BinaryOp::Sub if unknown_left => target + known,
                BinaryOp::Sub => known - target,
                BinaryOp::Mul if known == 0 => {
                    return Err(context.work.error(ErrorKind::Unsupported(
                        super::super::Feature::UnsafeVariable,
                    )));
                }
                BinaryOp::Mul if target % known != 0 => return Ok(None),
                BinaryOp::Mul => target / known,
                _ => unreachable!("compiler admits only single-candidate inverse operators"),
            };
            candidate(unknown, slot, target, binding, context)
        }
        _ => unreachable!("compiler admits exactly one unknown occurrence"),
    }
}
pub(super) fn bind<'input>(
    slot: usize,
    expression: &Template,
    target: i32,
    binding: &mut Binding<'input>,
    undo: &mut Vec<usize>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    if binding.is_bound(slot) {
        return Ok(context.numeric(expression, binding)? == target);
    }
    let Some(value) = candidate(expression, slot, i128::from(target), binding, context)? else {
        return Ok(false);
    };
    let Ok(value) = i32::try_from(value) else {
        return Ok(false);
    };
    let mut metric = super::Metric::default();
    context.work.step(1)?;
    context.work.node(&mut metric)?;
    context.work.check(
        super::Resource::Depth,
        1,
        context.work.limits.max_symbol_depth as u128,
    )?;
    context.work.construction_check(metric)?;
    context.work.check(
        super::Resource::LocalBytes,
        context.work.local_bytes + metric.payload(),
        context.work.limits.max_local_bytes as u128,
    )?;
    let key = context.number(value)?;
    binding.bind_term(slot, &key, metric, context.work)?;
    undo.push(slot);
    Ok(context.numeric(expression, binding)? == target)
}
