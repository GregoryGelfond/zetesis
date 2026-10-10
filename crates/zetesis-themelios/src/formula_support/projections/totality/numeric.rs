//! Total addition and subtraction over complete positive source columns.
//!
//! Each selected column contains every value of its variable in a complete
//! binding. Its numeric hull may include extra values: proving every arithmetic
//! intermediate defined on that hull therefore covers the binding. Correlations
//! and scalar guards provide no premises. Unknown bounds decline this optional
//! certificate; ordinary evaluation remains responsible for diagnostics.

use themelios_program::term::{BinaryOp, UnaryOp};
use zetesis_core::ValueNodeRef;

use super::{
    Computation, Context, Expression, FormulaFailure, LiteralIr, Operation, Postings, Support,
    domain, tick,
};
use crate::formula_support::Buffer;
use crate::scalar_arithmetic::Range;

pub(super) fn total(
    expression: &Expression,
    literals: &[LiteralIr],
    support: &Support<'_>,
    context: &mut Context<'_, &mut Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    if expression.nodes.iter().any(|node| {
        !matches!(
            node,
            Operation::Constant(_)
                | Operation::Variable(_)
                | Operation::Unary(UnaryOp::Negate, _)
                | Operation::Binary(BinaryOp::Add | BinaryOp::Sub, _, _)
        )
    }) {
        return Ok(false);
    }
    let mut values = Buffer::<Range>::new(
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )?;
    values.reserve(
        expression.nodes.len(),
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )?;
    for operation in &expression.nodes {
        tick(context)?;
        let bound = match operation {
            Operation::Constant(scalar) => match context
                .computation
                .static_scalar(
                    *scalar,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )?
                .descriptor()
            {
                ValueNodeRef::Number(value) => Some(Range::point(value)),
                _ => None,
            },
            Operation::Variable(slot) => numeric_domain(*slot, literals, support, context)?,
            Operation::Unary(UnaryOp::Negate, argument) => {
                values.slice()[*argument].unary(UnaryOp::Negate)
            }
            Operation::Binary(operator @ (BinaryOp::Add | BinaryOp::Sub), left, right) => {
                values.slice()[*left].binary(*operator, values.slice()[*right])
            }
            // A larger arithmetic grammar can be admitted separately. In
            // particular, no division or constructor follows from this proof.
            _ => None,
        };
        let Some(bound) = bound else {
            return Ok(false);
        };
        values.push(
            bound,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
    }
    Ok(!expression.nodes.is_empty())
}

fn numeric_domain(
    slot: usize,
    literals: &[LiteralIr],
    support: &Support<'_>,
    context: &mut Context<'_, &mut Computation<'_, '_>>,
) -> Result<Option<Range>, FormulaFailure> {
    let Some(domain) = domain(slot, literals, support, context)? else {
        return Ok(None);
    };
    let Postings::Indexed(postings) = domain.rows.postings(domain.column) else {
        unreachable!("a selected domain column keeps postings");
    };
    let mut bound: Option<Range> = None;
    for posting in postings.values() {
        tick(context)?;
        let position = *posting.first().expect("a distinct input has a source row");
        let value = domain
            .rows
            .row(position)
            .expect("retained source row")
            .value(domain.column)
            .expect("checked source column");
        let ValueNodeRef::Number(value) = value.descriptor() else {
            return Ok(None);
        };
        match &mut bound {
            Some(bound) => bound.include(value),
            None => bound = Some(Range::point(value)),
        }
    }
    Ok(bound)
}
