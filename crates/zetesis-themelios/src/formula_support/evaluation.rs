//! Flat plan evaluation with private, bounded reuse of empty value storage.
//!
//! Before node `i`, the live prefix holds precisely nodes `0..i` in source-plan
//! order. Operands refer only to that prefix; a node is appended only after its
//! work, operand-copy and result checks succeed. Failure never evaluates a later
//! node. Clearing the prefix preserves capacity, never a previous result.
//!
//! A short numeric prefix uses stack integers under the current source scalar
//! contract. Encountering a constructor, a nonnumeric leaf or the stack boundary
//! transfers the already evaluated prefix to the ordinary value workspace;
//! evaluation resumes without replaying a node, lookup or resource charge.

use themelios_base::span::Location;
use themelios_program::term::EvalError;
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

/// Private execution policy, never a language or numeric-width limit. The i32
/// elements implement the existing checked source carrier; widening that carrier
/// would change this private storage together with the scalar helpers.
const NUMERIC_PREFIX_CELLS: usize = 32;

/// One cursor's evaluation workspace; never shared across workers or scopes.
///
/// Peak live storage is linear in the current expression's evaluated nodes.
/// Between evaluations it contains zero values and at most 32 allocated cells.
/// A worker with `j` simultaneous join cursors therefore retains at most `32*j`
/// cells; cursor lifetime follows the existing bounded source-scope traversal.
/// Reuse changes neither the join schedule nor authored resource accounting.
/// A call additionally uses at most 32 stack integers for its numeric prefix.
#[derive(Default)]
pub(super) struct Evaluation {
    values: Vec<Value>,
}

impl Evaluation {
    pub(super) fn expression<'a>(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> &'a Value,
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        counters.record(Event::ExpressionEvaluation);
        let frame = Frame {
            values: &mut self.values,
        };
        let mut execution = Execution {
            limits,
            budget,
            counters,
            location,
        };
        match execution.numeric_prefix(&expression.nodes, &variable, frame.values)? {
            Prefix::Complete(value) => Ok(Value::Number(value)),
            Prefix::Promoted(next) => {
                execution.value_suffix(&expression.nodes[next..], &variable, frame.values)?;
                Ok(frame.values.pop().expect("expression has root"))
            }
        }
    }
}

/// A complete numeric result or the first node still needing generic evaluation.
enum Prefix {
    Complete(i32),
    Promoted(usize),
}

/// Shared accounting and source location for both representations of one plan.
struct Execution<'a> {
    limits: FormulaLimits,
    budget: &'a mut Budget,
    counters: &'a mut Counters,
    location: Location,
}

impl Execution<'_> {
    fn numeric_prefix<'a>(
        &mut self,
        nodes: &[Operation],
        variable: &impl Fn(usize) -> &'a Value,
        values: &mut Vec<Value>,
    ) -> Result<Prefix, FormulaFailure> {
        let mut numbers = [0; NUMERIC_PREFIX_CELLS];
        for (index, node) in nodes.iter().enumerate() {
            let prefix = &numbers[..index.min(NUMERIC_PREFIX_CELLS)];
            if index == NUMERIC_PREFIX_CELLS || matches!(node, Operation::Constructor(_)) {
                promote(prefix, values);
                return Ok(Prefix::Promoted(index));
            }
            self.node_work()?;
            let number = match *node {
                Operation::Constant(ref value) => self.leaf(value, prefix, values)?,
                Operation::Variable(index) => self.leaf(variable(index), prefix, values)?,
                Operation::Unary(operator, argument) => {
                    Some(self.scalar(crate::scalar_arithmetic::unary(operator, prefix[argument]))?)
                }
                Operation::Binary(operator, left, right) => Some(self.scalar(
                    crate::scalar_arithmetic::binary(operator, prefix[left], prefix[right]),
                )?),
                Operation::Absolute(argument) => {
                    Some(self.scalar(crate::scalar_arithmetic::absolute(prefix[argument]))?)
                }
                Operation::Constructor(_) => unreachable!("constructor promoted before evaluation"),
            };
            let Some(number) = number else {
                return Ok(Prefix::Promoted(index + 1));
            };
            numbers[index] = number;
        }
        Ok(numbers[..nodes.len()]
            .last()
            .copied()
            .map_or(Prefix::Promoted(0), Prefix::Complete))
    }

    /// Copy once in plan order. A nonnumeric leaf promotes the previous prefix
    /// and completes this same node before generic evaluation resumes.
    fn leaf(
        &mut self,
        value: &Value,
        prefix: &[i32],
        values: &mut Vec<Value>,
    ) -> Result<Option<i32>, FormulaFailure> {
        match copy(value, self.budget, self.location)? {
            Value::Number(number) => Ok(Some(number)),
            value => {
                promote(prefix, values);
                self.append(value, values)?;
                Ok(None)
            }
        }
    }

    fn scalar(&self, result: Result<i32, EvalError>) -> Result<i32, FormulaFailure> {
        result.map_err(|error| {
            ExpansionFailure::Evaluation {
                error,
                location: self.location,
            }
            .into()
        })
    }

    fn node_work(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)?;
        self.counters.record(Event::ExpressionNode);
        Ok(())
    }

    /// The original generic operation evaluator, starting at an already
    /// established prefix. Constructors and nonnumeric values keep this path.
    fn value_suffix<'a>(
        &mut self,
        nodes: &[Operation],
        variable: &impl Fn(usize) -> &'a Value,
        values: &mut Vec<Value>,
    ) -> Result<(), FormulaFailure> {
        for node in nodes {
            self.node_work()?;
            let value = match *node {
                Operation::Constructor(ref constructor) => constructor.evaluate(
                    values,
                    self.limits,
                    self.budget,
                    self.counters,
                    self.location,
                )?,
                Operation::Constant(ref value) => copy(value, self.budget, self.location)?,
                Operation::Variable(index) => copy(variable(index), self.budget, self.location)?,
                Operation::Unary(operator, argument) => scalar_value(
                    crate::scalar_arithmetic::unary(
                        operator,
                        numeric(&values[argument], self.location)?,
                    ),
                    self.location,
                )?,
                Operation::Binary(operator, left, right) => scalar_value(
                    crate::scalar_arithmetic::binary(
                        operator,
                        numeric(&values[left], self.location)?,
                        numeric(&values[right], self.location)?,
                    ),
                    self.location,
                )?,
                Operation::Absolute(argument) => scalar_value(
                    crate::scalar_arithmetic::absolute(numeric(&values[argument], self.location)?),
                    self.location,
                )?,
            };
            self.append(value, values)?;
        }
        Ok(())
    }

    fn append(&mut self, value: Value, values: &mut Vec<Value>) -> Result<(), FormulaFailure> {
        if let Value::Structured(structure) = &value {
            for _ in 0..structure.payload_bytes() {
                self.counters.work(self.limits, self.location)?;
            }
        }
        values.push(value);
        Ok(())
    }
}

/// Representation transfer only: no expression operation or semantic value copy
/// is repeated, so this carries no additional authored node/payload charge.
fn promote(numbers: &[i32], values: &mut Vec<Value>) {
    values.extend(numbers.iter().copied().map(Value::Number));
}

/// Drop the live prefix on success, typed failure and unwinding alike. The root
/// has already moved to the caller on success; no reference into storage escapes.
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

#[cfg(test)]
mod numeric_tests;
