//! Flat plan evaluation with private, bounded reuse of empty value storage.
//!
//! Before node `i`, the live prefix holds precisely nodes `0..i` in source-plan
//! order: as integers while every value so far is a number, and as value
//! cells from the first value that is not. Operands refer only to that prefix;
//! an intermediate result is appended only after its work, operand-copy and
//! result checks succeed. The root uses the same checked operation and returns
//! directly because no later node consumes it. Failure never evaluates a later
//! node. Clearing the prefix preserves capacity, never a previous result.

use themelios_base::span::Location;
use zetesis_core::Value;

use super::{Counters, copy, numeric, scalar_value};
use crate::expansion::Budget;
use crate::formula_ir::{Expression, Operation};
use crate::grounding_observer::Event;
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits};

/// At most this many empty cells survive an evaluation in one join cursor.
/// This is a storage policy, independent of copied-payload and logical-work
/// ceilings. Large expressions still use the existing finite plan and release
/// their entire workspace afterward. No strings or structures remain live.
const RETAINED_VALUE_CELLS: usize = 32;

/// One cursor's evaluation workspace; never shared across workers or scopes.
///
/// Scratch retains only intermediate results; a one-node plan allocates no
/// scratch cells. Peak live scratch is linear in the expression's proper prefix.
/// Between evaluations it contains zero values and at most 32 allocated cells
/// of each kind. A worker with `j` simultaneous join cursors therefore retains
/// at most `32*j` cells of each kind; cursor lifetime follows the existing
/// bounded source-scope traversal. The join lends this same workspace to
/// binding generators and final filters; those operations return before
/// another borrow begins. Reuse changes neither the join schedule nor authored
/// resource accounting.
///
/// A plan over numbers, the common case of a comparison or a binder, runs
/// entirely in the integer cells: no value is constructed or copied and no
/// payload is charged, since a number has none. The first value that is not a
/// number moves the integer prefix into value cells, and the plan continues
/// there, charging each value's payload as the value cells do.
#[derive(Default)]
pub(crate) struct Evaluation {
    values: Vec<Value>,
    integers: Vec<i32>,
}

impl Evaluation {
    pub(crate) fn expression<'a>(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> Result<&'a Value, FormulaFailure>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        counters.record(Event::ExpressionEvaluation);
        let mut frame = Frame {
            values: &mut self.values,
            integers: &mut self.integers,
        };
        let mut context = Context {
            variable: &variable,
            limits,
            budget,
            counters,
            location,
        };
        let (evaluated, first) = match integer_prefix(&mut frame, &expression.nodes, &mut context)?
        {
            Prefix::Complete(value) => return Ok(value),
            Prefix::Ended { evaluated, first } => (evaluated, first),
        };
        value_suffix(
            &mut frame,
            &expression.nodes,
            evaluated,
            first,
            &mut context,
        )
    }
}

/// The borrowed inputs one evaluation reads and charges.
struct Context<'c, 'a, V: Fn(usize) -> Result<&'a Value, FormulaFailure>> {
    variable: &'c V,
    limits: &'c FormulaLimits,
    budget: &'c mut Budget,
    counters: &'c mut Counters,
    location: Location,
}

impl<'a, V: Fn(usize) -> Result<&'a Value, FormulaFailure>> Context<'_, 'a, V> {
    fn admit(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)?;
        self.counters.record(Event::ExpressionNode);
        Ok(())
    }
    /// Charge a value's payload at once, so a refusal states that
    /// requirement, as the pattern match and the row match charge theirs.
    fn payload(&mut self, value: &Value) -> Result<(), FormulaFailure> {
        if let Value::Structured(structure) = value {
            self.counters.charge_work(
                structure.payload_bytes() as u128,
                self.limits,
                self.location,
            )?;
        }
        Ok(())
    }
}

/// How the integer prefix ended.
enum Prefix {
    /// Every node was a number, the root included.
    Complete(Value),
    /// The node at `evaluated - 1` produced `first`, the first value that is
    /// not a number, already admitted; or the plan ran out with `first`
    /// absent, which cannot happen since the root ends the integer prefix.
    Ended {
        evaluated: usize,
        first: Option<Value>,
    },
}

/// Evaluate nodes as integers while every value is a number. Each node is
/// admitted before it is read, in plan order, as in the value phase.
fn integer_prefix<'a, V: Fn(usize) -> Result<&'a Value, FormulaFailure>>(
    frame: &mut Frame<'_>,
    nodes: &[Operation],
    context: &mut Context<'_, 'a, V>,
) -> Result<Prefix, FormulaFailure> {
    let mut evaluated = 0;
    for node in nodes {
        context.admit()?;
        evaluated += 1;
        let integers: &[i32] = frame.integers;
        let result = match *node {
            Operation::Constant(Value::Number(number)) => Ok(number),
            Operation::Constant(ref value) => {
                let first = copy(value, context.budget, context.location)?;
                return Ok(Prefix::Ended {
                    evaluated,
                    first: Some(first),
                });
            }
            Operation::Variable(index) => match (context.variable)(index)? {
                Value::Number(number) => Ok(*number),
                value => {
                    let first = copy(value, context.budget, context.location)?;
                    return Ok(Prefix::Ended {
                        evaluated,
                        first: Some(first),
                    });
                }
            },
            Operation::Unary(operator, argument) => {
                crate::scalar_arithmetic::unary(operator, integers[argument])
            }
            Operation::Binary(operator, left, right) => {
                crate::scalar_arithmetic::binary(operator, integers[left], integers[right])
            }
            Operation::Absolute(argument) => crate::scalar_arithmetic::absolute(integers[argument]),
            Operation::Constructor(ref constructor) => {
                frame.materialize();
                let first = constructor.evaluate(
                    frame.values,
                    context.limits,
                    context.budget,
                    context.counters,
                    context.location,
                )?;
                return Ok(Prefix::Ended {
                    evaluated,
                    first: Some(first),
                });
            }
        };
        let number = result.map_err(|error| ExpansionFailure::Evaluation {
            error,
            location: context.location,
        })?;
        if evaluated == nodes.len() {
            return Ok(Prefix::Complete(Value::Number(number)));
        }
        frame.integers.push(number);
    }
    Ok(Prefix::Ended {
        evaluated,
        first: None,
    })
}

/// Continue on value cells: the integer prefix moves into them, the first
/// non-numeric value takes its place, and the remaining nodes follow.
fn value_suffix<'a, V: Fn(usize) -> Result<&'a Value, FormulaFailure>>(
    frame: &mut Frame<'_>,
    nodes: &[Operation],
    evaluated: usize,
    first: Option<Value>,
    context: &mut Context<'_, 'a, V>,
) -> Result<Value, FormulaFailure> {
    frame.materialize();
    if let Some(value) = first {
        context.payload(&value)?;
        if evaluated == nodes.len() {
            return Ok(value);
        }
        frame.values.push(value);
    }
    let (root, prefix) = nodes.split_last().expect("expression has root");
    for node in &prefix[evaluated..] {
        let value = evaluate(node, frame.values, context)?;
        frame.values.push(value);
    }
    evaluate(root, frame.values, context)
}

fn evaluate<'a, V: Fn(usize) -> Result<&'a Value, FormulaFailure>>(
    node: &Operation,
    values: &[Value],
    context: &mut Context<'_, 'a, V>,
) -> Result<Value, FormulaFailure> {
    context.admit()?;
    let location = context.location;
    let value = match *node {
        Operation::Constructor(ref constructor) => constructor.evaluate(
            values,
            context.limits,
            context.budget,
            context.counters,
            location,
        )?,
        Operation::Constant(ref value) => copy(value, context.budget, location)?,
        Operation::Variable(index) => copy((context.variable)(index)?, context.budget, location)?,
        Operation::Unary(operator, argument) => scalar_value(
            crate::scalar_arithmetic::unary(operator, numeric(&values[argument], location)?),
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
    context.payload(&value)?;
    Ok(value)
}

/// Drop the live prefix on success, typed failure and unwinding alike. The root
/// is returned separately on success; no reference into storage escapes.
struct Frame<'a> {
    values: &'a mut Vec<Value>,
    integers: &'a mut Vec<i32>,
}

impl Frame<'_> {
    /// Move the integer prefix into value cells, in plan order.
    fn materialize(&mut self) {
        self.values
            .extend(self.integers.drain(..).map(Value::Number));
    }
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        self.values.clear();
        if self.values.capacity() > RETAINED_VALUE_CELLS {
            *self.values = Vec::new();
        }
        self.integers.clear();
        if self.integers.capacity() > RETAINED_VALUE_CELLS {
            *self.integers = Vec::new();
        }
    }
}

#[cfg(test)]
mod tests;
