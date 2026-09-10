//! Flat plan evaluation with private, bounded reuse of empty value storage.
//!
//! Before node `i`, the live prefix holds precisely nodes `0..i` in source-plan
//! order. Operands refer only to that prefix; an intermediate result is appended
//! only after its work, operand-copy and result checks succeed. The root uses the
//! same checked operation and returns directly because no later node consumes it.
//! Failure never evaluates a later node. Clearing the prefix preserves capacity,
//! never a previous result.

use themelios_base::span::Location;
use zetesis_core::Value;

use super::{Counters, copy, numeric, scalar_value};
use crate::expansion::Budget;
use crate::formula_ir::{Expression, Operation};
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits};

/// At most this many empty cells survive an evaluation in one join cursor.
/// This is a storage policy, independent of copied-payload and logical-work
/// ceilings. Large expressions still use the existing finite plan and release
/// their entire workspace afterward. No strings or structures remain live.
const RETAINED_VALUE_CELLS: usize = 32;

/// One cursor's evaluation workspace; never shared across workers or scopes.
///
/// Scratch retains only intermediate results; a one-node plan allocates no
/// scratch cells. Peak live scratch is linear in the expression's proper prefix.
/// Between evaluations it contains zero values and at most 32 allocated cells.
/// A worker with `j` simultaneous join cursors therefore retains at most `32*j`
/// cells; cursor lifetime follows the existing bounded source-scope traversal.
/// The join lends this same workspace to binding generators and final filters;
/// those operations return before another borrow begins. Reuse changes neither
/// the join schedule nor authored resource accounting.
#[derive(Default)]
pub(crate) struct Evaluation {
    values: Vec<Value>,
}

impl Evaluation {
    pub(crate) fn expression<'a>(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> &'a Value,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        counters.record(Event::ExpressionEvaluation);
        let (root, prefix) = expression.nodes.split_last().expect("expression has root");
        let frame = Frame {
            values: &mut self.values,
        };
        let mut evaluate = |node: &Operation, values: &[Value]| -> Result<Value, FormulaFailure> {
            counters.work(limits, location)?;
            counters.record(Event::ExpressionNode);
            let value = match *node {
                Operation::Constructor(ref constructor) => {
                    constructor.evaluate(values, limits, budget, counters, location)?
                }
                Operation::Constant(ref value) => copy(value, budget, location)?,
                Operation::Variable(index) => copy(variable(index), budget, location)?,
                Operation::Unary(operator, argument) => scalar_value(
                    crate::scalar_arithmetic::unary(
                        operator,
                        numeric(&values[argument], location)?,
                    ),
                    location,
                )?,
                Operation::Binary(operator, left, right) => scalar_value(
                    crate::scalar_arithmetic::binary(
                        operator,
                        numeric(&values[left], location)?,
                        numeric(&values[right], location)?,
                    ),
                    location,
                )?,
                Operation::Absolute(argument) => scalar_value(
                    crate::scalar_arithmetic::absolute(numeric(&values[argument], location)?),
                    location,
                )?,
            };
            if let Value::Structured(structure) = &value {
                for _ in 0..structure.payload_bytes() {
                    counters.work(limits, location)?;
                }
            }
            Ok(value)
        };
        for node in prefix {
            let value = evaluate(node, frame.values)?;
            frame.values.push(value);
        }
        evaluate(root, frame.values)
    }
}

/// Drop the live prefix on success, typed failure and unwinding alike. The root
/// is returned separately on success; no reference into storage escapes.
struct Frame<'a> {
    values: &'a mut Vec<Value>,
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        self.values.clear();
        if self.values.capacity() > RETAINED_VALUE_CELLS {
            *self.values = Vec::new();
        }
    }
}

#[cfg(test)]
mod tests;
