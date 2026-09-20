//! Flat plan evaluation with private, bounded reuse of empty value storage.
//!
//! Before node `i`, the live prefix holds precisely nodes `0..i` in source-plan
//! order: as integers while every value so far is a number, and as value
//! cells from the first value that is not. Operands refer only to that prefix;
//! an intermediate result is appended only after its work, operand-copy and
//! result checks succeed. The root uses the same checked operation and returns
//! directly because no later node consumes it. Strict evaluation stops at the
//! first failure. Source-family evaluation retains numeric zero-divisor
//! failures, visits independent later nodes, and skips every operation with a
//! missing operand. A missing-mask cell prevents any placeholder scratch cell
//! from being read as a semantic value. Clearing the prefix preserves capacity,
//! never a previous result.

use themelios_base::span::Location;
use themelios_program::term::BinaryOp;
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
/// another borrow begins. Reuse itself changes neither the join schedule nor
/// authored resource accounting. Source-family mode additionally owns one
/// transient missing-node mask, bounded by the already-admitted expression's
/// node count and reserved fallibly. It is released before the next evaluation,
/// rather than charged to cumulative scalar payload. Independent-branch
/// validation retains the same per-node work charges.
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
    zero_divisor: bool,
}

impl Evaluation {
    /// Cause of the most recent failed expression; reset before every attempt.
    pub(crate) const fn zero_divisor(&self) -> bool {
        self.zero_divisor
    }

    pub(crate) fn zero_divisor_failure(&mut self, location: Location) -> FormulaFailure {
        self.zero_divisor = true;
        super::undefined(location)
    }

    pub(crate) fn expression<'a>(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> Result<&'a Value, FormulaFailure>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        self.evaluate(
            expression,
            Mode::Strict(|slot| variable(slot).map(Some)),
            limits,
            budget,
            counters,
            location,
        )
    }

    /// Source families validate independent branches after a zero divisor.
    /// An operation depending on a missing value is not evaluated. The scalar
    /// operations themselves are shared with strict, first-failure evaluation.
    pub(crate) fn source_expression<'a>(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> Result<&'a Value, FormulaFailure>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        self.evaluate(
            expression,
            Mode::Source(|slot| variable(slot).map(Some)),
            limits,
            budget,
            counters,
            location,
        )
    }

    /// `None` is an inherited arithmetic-unavailable input, never an unbound
    /// source variable. Independent branches remain subject to checked arithmetic.
    pub(crate) fn source_partial<'a>(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> Result<Option<&'a Value>, FormulaFailure>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        self.evaluate(
            expression,
            Mode::Partial(variable),
            limits,
            budget,
            counters,
            location,
        )
    }

    /// Evaluate independent expression components without allowing a zero in
    /// one component to conceal a fatal arithmetic failure in another.
    pub(crate) fn source_values<'a, const N: usize>(
        &mut self,
        expressions: [&Expression; N],
        variable: impl Fn(usize) -> Result<&'a Value, FormulaFailure>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<[Value; N], FormulaFailure> {
        let mut failures = Failures::default();
        let mut values: [Option<Value>; N] = std::array::from_fn(|_| None);
        for (expression, value) in expressions.into_iter().zip(&mut values) {
            let result =
                self.source_expression(expression, &variable, limits, budget, counters, location);
            *value = failures.value(result, self.zero_divisor())?;
        }
        failures.finish(self)?;
        Ok(values.map(|value| value.expect("every independent component is defined")))
    }

    pub(crate) fn source_tuple<'a>(
        &mut self,
        (left, right): (&[Expression], &[Expression]),
        variable: impl Fn(usize) -> Result<&'a Value, FormulaFailure>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let mut equal = left.len() == right.len();
        let mut failures = Failures::default();
        for index in 0..left.len().max(right.len()) {
            let result = match (left.get(index), right.get(index)) {
                (Some(left), Some(right)) => self
                    .source_values([left, right], &variable, limits, budget, counters, location)
                    .map(|[left, right]| left == right),
                (Some(value), None) | (None, Some(value)) => self
                    .source_expression(value, &variable, limits, budget, counters, location)
                    .map(|_| false),
                (None, None) => unreachable!("one tuple has a component at this index"),
            };
            if let Some(value) = failures.value(result, self.zero_divisor())? {
                equal &= value;
            }
        }
        failures.finish(self)?;
        Ok(equal)
    }

    fn evaluate<'a>(
        &mut self,
        expression: &Expression,
        mode: Mode<impl Fn(usize) -> Result<Option<&'a Value>, FormulaFailure>>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        self.zero_divisor = false;
        counters.record(Event::ExpressionEvaluation);
        let mut missing = Vec::new();
        if matches!(&mode, Mode::Partial(_))
            || (matches!(&mode, Mode::Source(_)) && super::family::expression(expression))
        {
            // The compiled plan already bounds this one-attempt mask. Like
            // ordinary evaluation/frame scratch, it retains no scalar payload
            // between evaluations; a long enumeration cannot accumulate it.
            missing
                .try_reserve_exact(expression.nodes.len())
                .map_err(|_| FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Allocation,
                    location,
                })?;
            missing.resize(expression.nodes.len(), false);
        }
        let variable = match mode {
            Mode::Strict(variable) | Mode::Source(variable) | Mode::Partial(variable) => variable,
        };
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
            zero_divisor: &mut self.zero_divisor,
            missing,
        };
        let result = (|| match integer_prefix(&mut frame, &expression.nodes, &mut context)? {
            Prefix::Complete(value) => Ok(value),
            Prefix::Ended { evaluated, first } => value_suffix(
                &mut frame,
                &expression.nodes,
                evaluated,
                first,
                &mut context,
            ),
        })();
        if result.is_ok() && context.missing.last().copied().unwrap_or(false) {
            drop(context);
            drop(frame);
            return Err(self.zero_divisor_failure(location));
        }
        result
    }
}

/// A variable reader together with its arithmetic-availability policy.
enum Mode<V> {
    Strict(V),
    Source(V),
    Partial(V),
}

/// Arithmetic failures of independent components; resources are never deferred.
#[derive(Default)]
pub(crate) struct Failures {
    zero: Option<ExpansionFailure>,
    fatal: Option<ExpansionFailure>,
}

impl Failures {
    pub(crate) fn value<T>(
        &mut self,
        result: Result<T, FormulaFailure>,
        zero: bool,
    ) -> Result<Option<T>, FormulaFailure> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. })) => {
                if zero {
                    self.zero.get_or_insert(error);
                } else {
                    self.fatal.get_or_insert(error);
                }
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn finish(self, evaluation: &mut Evaluation) -> Result<(), FormulaFailure> {
        evaluation.zero_divisor = self.fatal.is_none() && self.zero.is_some();
        match self.fatal.or(self.zero) {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }
}

/// The borrowed inputs one evaluation reads and charges.
struct Context<'c, 'a, V: Fn(usize) -> Result<Option<&'a Value>, FormulaFailure>> {
    variable: &'c V,
    limits: &'c FormulaLimits,
    budget: &'c mut Budget,
    counters: &'c mut Counters,
    location: Location,
    zero_divisor: &'c mut bool,
    missing: Vec<bool>,
}

impl<'a, V: Fn(usize) -> Result<Option<&'a Value>, FormulaFailure>> Context<'_, 'a, V> {
    fn dependent(&self, node: &Operation) -> bool {
        let missing = |index| self.missing.get(index).copied().unwrap_or(false);
        match node {
            Operation::Unary(_, argument) | Operation::Absolute(argument) => missing(*argument),
            Operation::Binary(_, left, right) => missing(*left) || missing(*right),
            Operation::Constructor(constructor) => {
                constructor.arguments.iter().copied().any(missing)
            }
            Operation::Constant(_) | Operation::Variable(_) => false,
        }
    }

    fn retain<T>(
        &mut self,
        result: Result<T, FormulaFailure>,
        index: usize,
        placeholder: T,
    ) -> Result<T, FormulaFailure> {
        match result {
            Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. }))
                if *self.zero_divisor && !self.missing.is_empty() =>
            {
                self.missing[index] = true;
                *self.zero_divisor = false;
                Ok(placeholder)
            }
            other => other,
        }
    }

    fn binary(
        &mut self,
        operator: BinaryOp,
        left: i32,
        right: i32,
    ) -> Result<i32, themelios_program::term::EvalError> {
        *self.zero_divisor = matches!(operator, BinaryOp::Div | BinaryOp::Mod) && right == 0;
        crate::scalar_arithmetic::binary(operator, left, right)
    }

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
    /// not a number, already admitted.
    Ended { evaluated: usize, first: Value },
}

/// Evaluate nodes as integers while every value is a number. Each node is
/// admitted before it is read, in plan order, as in the value phase.
fn integer_prefix<'a, V: Fn(usize) -> Result<Option<&'a Value>, FormulaFailure>>(
    frame: &mut Frame<'_>,
    nodes: &[Operation],
    context: &mut Context<'_, 'a, V>,
) -> Result<Prefix, FormulaFailure> {
    let mut evaluated = 0;
    for node in nodes {
        context.admit()?;
        evaluated += 1;
        if context.dependent(node) {
            context.missing[evaluated - 1] = true;
            if evaluated == nodes.len() {
                return Ok(Prefix::Complete(Value::Number(0)));
            }
            frame.integers.push(0);
            continue;
        }
        let integers: &[i32] = frame.integers;
        let result = match *node {
            Operation::Constant(Value::Number(number)) => Ok(number),
            Operation::Constant(ref value) => {
                let first = copy(value, context.budget, context.location)?;
                return Ok(Prefix::Ended { evaluated, first });
            }
            Operation::Variable(index) => match (context.variable)(index)? {
                Some(Value::Number(number)) => Ok(*number),
                Some(value) => {
                    let first = copy(value, context.budget, context.location)?;
                    return Ok(Prefix::Ended { evaluated, first });
                }
                None => {
                    context.missing[evaluated - 1] = true;
                    Ok(0)
                }
            },
            Operation::Unary(operator, argument) => {
                crate::scalar_arithmetic::unary(operator, integers[argument])
            }
            Operation::Binary(operator, left, right) => {
                context.binary(operator, integers[left], integers[right])
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
                return Ok(Prefix::Ended { evaluated, first });
            }
        };
        let result = result.map_err(|error| {
            FormulaFailure::from(ExpansionFailure::Evaluation {
                error,
                location: context.location,
            })
        });
        let number = context.retain(result, evaluated - 1, 0)?;
        if evaluated == nodes.len() {
            return Ok(Prefix::Complete(Value::Number(number)));
        }
        frame.integers.push(number);
    }
    unreachable!("the root ends the integer prefix: an expression has a root")
}

/// Continue on value cells: the integer prefix moves into them, the first
/// non-numeric value takes its place, and the remaining nodes follow.
fn value_suffix<'a, V: Fn(usize) -> Result<Option<&'a Value>, FormulaFailure>>(
    frame: &mut Frame<'_>,
    nodes: &[Operation],
    evaluated: usize,
    first: Value,
    context: &mut Context<'_, 'a, V>,
) -> Result<Value, FormulaFailure> {
    frame.materialize();
    context.payload(&first)?;
    if evaluated == nodes.len() {
        return Ok(first);
    }
    frame.values.push(first);
    let (root, prefix) = nodes.split_last().expect("expression has root");
    for (index, node) in prefix.iter().enumerate().skip(evaluated) {
        let value = source_node(node, index, frame.values, context)?;
        frame.values.push(value);
    }
    source_node(root, nodes.len() - 1, frame.values, context)
}

fn source_node<'a, V: Fn(usize) -> Result<Option<&'a Value>, FormulaFailure>>(
    node: &Operation,
    index: usize,
    values: &[Value],
    context: &mut Context<'_, 'a, V>,
) -> Result<Value, FormulaFailure> {
    if context.dependent(node) {
        context.admit()?;
        context.missing[index] = true;
        return Ok(Value::Number(0));
    }
    let result = evaluate(node, index, values, context);
    context.retain(result, index, Value::Number(0))
}

fn evaluate<'a, V: Fn(usize) -> Result<Option<&'a Value>, FormulaFailure>>(
    node: &Operation,
    index: usize,
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
        Operation::Variable(variable) => {
            if let Some(value) = (context.variable)(variable)? {
                copy(value, context.budget, location)?
            } else {
                context.missing[index] = true;
                Value::Number(0)
            }
        }
        Operation::Unary(operator, argument) => scalar_value(
            crate::scalar_arithmetic::unary(operator, numeric(&values[argument], location)?),
            location,
        )?,
        Operation::Binary(operator, left, right) => scalar_value(
            context.binary(
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
