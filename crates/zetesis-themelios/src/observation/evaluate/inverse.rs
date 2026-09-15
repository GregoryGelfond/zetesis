//! Invert one admitted arithmetic path, then check the authored expression.
//!
//! Inverse calculations are widened only to find an ordinary scalar candidate.
//! Nonintegral/out-of-range candidates do not match. Authored arithmetic still
//! uses the common checked evaluator after capture; no failed output escapes.

use themelios_program::term::BinaryOp;

use super::{Bound, Error, ErrorKind, Symbol, Template, UnaryOp, Work};

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

fn candidate(
    term: &Template,
    slot: usize,
    target: i128,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<Option<i128>, Error> {
    work.step(1)?;
    match term {
        Template::Variable(found) if *found == slot => Ok(Some(target)),
        Template::Unary(UnaryOp::Negate, inner) => candidate(inner, slot, -target, binding, work),
        Template::Binary(operator, left, right) => {
            let unknown_left = contains(left, slot, work)?;
            let (unknown, known) = if unknown_left {
                (left, right)
            } else {
                (right, left)
            };
            let known = i128::from(work.numeric(known, binding)?);
            let target = match operator {
                BinaryOp::Add => target - known,
                BinaryOp::Sub if unknown_left => target + known,
                BinaryOp::Sub => known - target,
                BinaryOp::Mul if known == 0 => {
                    // Zero does not determine a finite inverse binding.
                    return Err(work.error(ErrorKind::Unsupported(
                        super::super::Feature::UnsafeVariable,
                    )));
                }
                BinaryOp::Mul if target % known != 0 => return Ok(None),
                BinaryOp::Mul => target / known,
                _ => unreachable!("compiler admits only single-candidate inverse operators"),
            };
            candidate(unknown, slot, target, binding, work)
        }
        _ => unreachable!("compiler admits exactly one unknown occurrence"),
    }
}

pub(super) fn bind(
    slot: usize,
    expression: &Template,
    value: &Symbol,
    binding: &mut [Option<Bound<'_>>],
    undo: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    let Symbol::Number(target) = value else {
        return Ok(false);
    };
    if binding[slot].is_some() {
        return Ok(work.numeric(expression, binding)? == *target);
    }
    let Some(value) = candidate(expression, slot, i128::from(*target), binding, work)? else {
        return Ok(false);
    };
    let Ok(value) = i32::try_from(value) else {
        return Ok(false);
    };
    let (value, metric) = super::patterns::own(&Symbol::Number(value), work)?;
    binding[slot] = Some(Bound::Owned(value, metric));
    undo.push(slot);
    Ok(work.numeric(expression, binding)? == *target)
}
