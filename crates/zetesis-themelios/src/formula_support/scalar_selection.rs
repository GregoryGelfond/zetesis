//! Necessary interval masks for certified scalar assignment continuations.
//!
//! Full columns of the same immutable support snapshot cover every extension.
//! Bound prefix values replace their column hulls by points. Every arithmetic
//! transfer encloses the authored result, so disjoint comparison bounds prove
//! that no extension passes. Unknown/mixed values and declined transfers never
//! reject a prefix. The caller separately certifies every original body scalar
//! before enabling these masks; head phases and diagnostics keep their order.

use themelios_program::program::Relation;
use zetesis_core::{TemplateTerm, ValueNodeRef};

use super::{Buffer, Computation, Context, PatternOccurrence, Support, comparison, projections};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_binding_plan::expanded::View;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::scalar_arithmetic::Range;

/// One cursor publishes both the selected comparison DAG and its domain hulls
/// together, after totality succeeds over its exact immutable support snapshot.
pub(super) enum Preparation {
    Pending,
    Unavailable,
    Ready { comparisons: View, domains: Domains },
}

impl Preparation {
    pub(super) const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub(super) const fn comparisons(&self) -> Option<&View> {
        match self {
            Self::Ready { comparisons, .. } => Some(comparisons),
            Self::Pending | Self::Unavailable => None,
        }
    }

    pub(super) const fn leased_header(&self) -> usize {
        match self {
            Self::Ready { .. } => View::leased_header() + Domains::leased_header(),
            Self::Pending | Self::Unavailable => 0,
        }
    }

    pub(super) fn permits(
        &mut self,
        literals: &[LiteralIr],
        binding: &Binding<'_>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        match self {
            Self::Ready {
                comparisons,
                domains,
            } => domains.permits(literals, comparisons, binding, context),
            Self::Pending | Self::Unavailable => Ok(true),
        }
    }
}

pub(super) struct Domains {
    values: Buffer<Option<Range>>,
    scratch: Buffer<Option<Range>>,
}

impl Domains {
    /// These buffer leases retain their headers when the join owns this value.
    pub(super) const fn leased_header() -> usize {
        2 * size_of::<Buffer<Option<Range>>>()
    }

    pub(super) fn prepare(
        patterns: &[PatternOccurrence<'_>],
        variables: usize,
        support: &Support<'_>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let mut values = buffer(&mut context)?;
        values.resize(
            variables,
            None,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        for occurrence in patterns {
            tick(&mut context)?;
            let pattern = occurrence.atom();
            let Some(rows) = support.resolve(
                pattern,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?
            else {
                continue;
            };
            for column in 0..pattern.terms().len() {
                tick(&mut context)?;
                let Some(TemplateTerm::Variable(slot)) = pattern.terms().at(column) else {
                    continue;
                };
                let Some(bound) = values.slice_mut().get_mut(slot) else {
                    continue;
                };
                if bound.is_none() {
                    *bound = projections::column_bounds(rows, column, &mut context)?;
                }
            }
        }
        Ok(Self {
            values,
            scratch: buffer(&mut context)?,
        })
    }

    pub(super) fn permits(
        &mut self,
        literals: &[LiteralIr],
        view: &View,
        binding: &Binding<'_>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        for (index, literal) in literals.iter().enumerate() {
            tick(&mut context)?;
            let Some(([left, right], relation)) = view
                .operands(index)
                .or_else(|| comparison(literal).map(crate::formula_ir::Comparison::operands))
            else {
                continue;
            };
            let left = self.range(left, binding, &mut context)?;
            let right = self.range(right, binding, &mut context)?;
            if let (Some(left), Some(right)) = (left, right)
                && !possible(left, relation, right)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn range(
        &mut self,
        expression: &Expression,
        binding: &Binding<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Range>, FormulaFailure> {
        self.scratch.clear();
        self.scratch.reserve(
            expression.nodes.len(),
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        for operation in &expression.nodes {
            tick(context)?;
            let value = match operation {
                Operation::Constant(value) => match context
                    .computation
                    .static_scalar(
                        *value,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )?
                    .descriptor()
                {
                    ValueNodeRef::Number(value) => Some(Range::point(value)),
                    _ => None,
                },
                Operation::Variable(slot) => {
                    if binding.is_bound(*slot, context.work.location)? {
                        match binding
                            .read(*slot, context.computation.read(), context.work.location)?
                            .descriptor()
                        {
                            ValueNodeRef::Number(value) => Some(Range::point(value)),
                            _ => None,
                        }
                    } else {
                        self.values.slice().get(*slot).copied().flatten()
                    }
                }
                Operation::Unary(operator, child) => {
                    self.scratch.slice()[*child].and_then(|value| value.unary(*operator))
                }
                Operation::Absolute(child) => {
                    self.scratch.slice()[*child].and_then(Range::absolute)
                }
                Operation::Binary(operator, left, right) => self.scratch.slice()[*left]
                    .zip(self.scratch.slice()[*right])
                    .and_then(|(left, right)| left.binary(*operator, right)),
                Operation::Constructor(_) => None,
            };
            self.scratch.push(
                value,
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
        }
        Ok(self.scratch.slice().last().copied().flatten())
    }
}

/// Whether these independent interval supersets can contain a satisfying pair.
fn possible(left: Range, relation: Relation, right: Range) -> bool {
    let ((low, high), (other_low, other_high)) = (left.bounds(), right.bounds());
    match relation {
        Relation::Eq => low <= other_high && other_low <= high,
        Relation::Neq => low != high || other_low != other_high || low != other_low,
        Relation::Lt => low < other_high,
        Relation::Le => low <= other_high,
        Relation::Gt => high > other_low,
        Relation::Ge => high >= other_low,
    }
}

fn buffer<T>(context: &mut Context<'_, &Computation<'_, '_>>) -> Result<Buffer<T>, FormulaFailure> {
    Buffer::new(
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )
}
fn tick(context: &mut Context<'_, &Computation<'_, '_>>) -> Result<(), FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)
}

#[cfg(test)]
mod tests;
