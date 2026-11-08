//! Total integer expressions over complete-binding domains.
//!
//! This is separate from defined-value transfer: a narrow result of that
//! transfer does not imply that each intermediate is defined. The postorder
//! stack checks every child before its parent, including both sides of e-e.
//! Nodes refer only to earlier nodes, so traversal terminates; shared children
//! may be revisited, and every visit is charged. Scratch is bounded by the
//! admitted expression depth and work, without another expression graph.
use super::{
    BinaryOp, Bound, Error, Expression, ExpressionId, Input, Integrality, Interval, UnaryOp, Values,
};
use crate::Range;

/// One requested expression's certificate or conservative decline.
#[derive(Debug)]
pub(super) struct Evaluation {
    pub(super) expression: ExpressionId,
    pub(super) range: Option<Range>,
}

fn slot(bound: Bound) -> Option<Range> {
    if bound.integrality != Integrality::Integer {
        return None;
    }
    match bound.interval {
        Interval::Bounded { low, high } => Range::new(low, high),
        Interval::Bottom | Interval::Unbounded => None,
    }
}

fn binary(operator: BinaryOp, left: Option<Range>, right: Option<Range>) -> Option<Range> {
    // Every child has already been certified over the complete-binding domains.
    // Range checks all operand pairs, including zero/MIN/-1 and intermediate
    // overflow. A power additionally needs one nonnegative exponent throughout
    // its domain; its syntax need not be a literal.
    let covered = matches!(
        operator,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::Pow
    );
    if !covered {
        return None;
    }
    left?.binary(operator, right?)
}

fn expression<R: Values, C: Copy>(
    input: &Input<'_, R, C>,
    root: ExpressionId,
    domains: &[Bound],
) -> Result<Option<Range>, Error<R::Error>> {
    let mut pending = Vec::new();
    let mut values: Vec<Option<Range>> = Vec::new();
    pending.try_reserve(1)?;
    pending.push((root, false));
    while let Some((id, ready)) = pending.pop() {
        input.budget.work(1)?;
        let node = *input.expressions.get(id.0).ok_or(Error::InvalidInput)?;
        if !ready {
            match node {
                Expression::Unary(_, child) | Expression::Absolute(child) => {
                    pending.try_reserve(2)?;
                    pending.extend([(id, true), (child, false)]);
                    continue;
                }
                Expression::Binary(_, left, right) => {
                    pending.try_reserve(3)?;
                    pending.extend([(id, true), (right, false), (left, false)]);
                    continue;
                }
                Expression::Constant(_) | Expression::Slot(_) | Expression::Unknown { .. } => {}
            }
        }
        let value = match node {
            Expression::Constant(id) => input
                .constants
                .get(id.0)
                .ok_or(Error::InvalidInput)?
                .number
                .map(Range::point),
            Expression::Slot(id) => slot(*domains.get(id).ok_or(Error::InvalidInput)?),
            Expression::Unknown { .. } => None,
            Expression::Unary(operator, _) => {
                let child = values.pop().ok_or(Error::InvalidInput)?;
                if operator == UnaryOp::Negate {
                    child.and_then(|range| range.unary(operator))
                } else {
                    None
                }
            }
            Expression::Absolute(_) => values
                .pop()
                .ok_or(Error::InvalidInput)?
                .and_then(Range::absolute),
            Expression::Binary(operator, _, _) => {
                let right = values.pop().ok_or(Error::InvalidInput)?;
                let left = values.pop().ok_or(Error::InvalidInput)?;
                binary(operator, left, right)
            }
        };
        values.try_reserve(1)?;
        values.push(value);
    }
    values.pop().ok_or(Error::InvalidInput)
}

pub(super) fn queries<R: Values, C: Copy>(
    input: &Input<'_, R, C>,
    roots: &[ExpressionId],
    domains: &[Bound],
) -> Result<Vec<Evaluation>, Error<R::Error>> {
    // Requests were admitted with their producer. Result cells are separate
    // retained metadata, reserved before allocation even for declined queries.
    input.links(roots.len())?;
    let mut results = Vec::new();
    results.try_reserve(roots.len())?;
    for &root in roots {
        input.budget.work(1)?;
        let range = expression(input, root, domains)?;
        input.budget.work(1)?;
        results.push(Evaluation {
            expression: root,
            range,
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests;
