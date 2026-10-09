//! Consume immutable comparison leaves without retaining temporary owner keys.
//! General expressions keep owned results across later mutable computation.

use super::{Evaluation, Expression, Failures, Frame, Operation, scope};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_support::{Computation, Context, GroundingWork, compare};
use crate::grounding_observer::Event;
use themelios_program::program::Relation;
use zetesis_core::catalog::{TermRead, TermRef};

impl Evaluation {
    pub(crate) fn source_comparison(
        &mut self,
        comparison: ([&Expression; 2], Relation),
        binding: &Binding<'_>,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        let (expressions, relation) = comparison;
        let Context {
            computation,
            mut work,
        } = context;
        if let Some(leaves) = leaves(expressions) {
            // Both operands are read-only: an earlier view stays valid while
            // the next expression admits and resets only evaluation metadata.
            let mut failures = Failures::default();
            let left = self.comparison_leaf(leaves[0], binding, computation, &mut work);
            let left = failures.value(left, self.zero_divisor())?;
            let right = self.comparison_leaf(leaves[1], binding, computation, &mut work);
            let right = failures.value(right, self.zero_divisor())?;
            failures.finish(self)?;
            return compare(
                left.expect("both leaf values are defined"),
                relation,
                right.expect("both leaf values are defined"),
                work.limits,
                work.counters,
                work.location,
            );
        }
        self.source_values(
            expressions,
            |variable| binding.key(variable, work.location),
            computation,
            work.limits,
            work.counters,
            work.location,
        )
        .and_then(|[left, right]| {
            let read = computation.read();
            let left = read
                .term(&left)
                .map_err(|error| scope(error, work.location))?;
            let right = read
                .term(&right)
                .map_err(|error| scope(error, work.location))?;
            compare(
                left,
                relation,
                right,
                work.limits,
                work.counters,
                work.location,
            )
        })
    }

    fn comparison_leaf<'read>(
        &mut self,
        node: &Operation,
        binding: &Binding<'_>,
        computation: &'read Computation<'_, '_>,
        work: &mut GroundingWork<'_>,
    ) -> Result<TermRef<'read>, FormulaFailure> {
        self.zero_divisor = false;
        work.counters.record(Event::ExpressionEvaluation);
        self.scratch
            .begin(computation, work.limits, work.counters, work.location)?;
        let _frame = Frame {
            scratch: &mut self.scratch,
            location: work.location,
        };
        work.counters.work(work.limits, work.location)?;
        work.counters.record(Event::ExpressionNode);
        match node {
            Operation::Constant(scalar) => {
                let value = computation.static_scalar(
                    *scalar,
                    work.limits,
                    work.counters,
                    work.location,
                )?;
                work.counters.work(work.limits, work.location)?;
                TermRead::from(computation.read())
                    .borrow_term(value)
                    .map_err(|error| scope(error, work.location))
            }
            Operation::Variable(slot) => {
                binding.read_with(*slot, computation.read(), work.location, || {
                    work.counters.work(work.limits, work.location)
                })
            }
            _ => unreachable!("only two read-only leaves take this path"),
        }
    }
}

fn leaves(expressions: [&Expression; 2]) -> Option<[&Operation; 2]> {
    let [left] = expressions[0].nodes.as_slice() else {
        return None;
    };
    let [right] = expressions[1].nodes.as_slice() else {
        return None;
    };
    if [left, right]
        .iter()
        .all(|node| matches!(node, Operation::Constant(_) | Operation::Variable(_)))
    {
        Some([left, right])
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
