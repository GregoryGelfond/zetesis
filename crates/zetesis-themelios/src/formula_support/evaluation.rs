//! Flat expression evaluation over one canonical term authority.
//!
//! Numeric prefixes use machine integers. Once a nonnumeric result is needed,
//! scratch stores scoped term IDs; constructors reuse child IDs in the same DAG.
//! Missing arithmetic outputs are explicit and never enter an emitted binding.
//! Independent source branches still run after a zero divisor. Reusable scratch
//! retains bounded empty metadata, never an owned Value or copied child tree.

use crate::ProgramSite;
use themelios_program::term::{BinaryOp, EvalError};
use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::{AssignmentError, TermKey};

use super::{Computation, Counters};
use crate::formula_ir::{Expression, Operation};
use crate::grounding_observer::Event;
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits};

mod comparisons;
mod scratch;
use scratch::{Frame, Scratch};

type Input = Result<Option<TermKey>, FormulaFailure>;

#[derive(Default)]
pub(crate) struct Evaluation {
    scratch: Scratch,
    zero_divisor: bool,
}

impl Evaluation {
    /// A reused successful source value has no unavailable arithmetic output.
    pub(super) fn clear_zero_divisor(&mut self) {
        self.zero_divisor = false;
    }

    pub(crate) const fn zero_divisor(&self) -> bool {
        self.zero_divisor
    }

    pub(crate) fn zero_divisor_failure(&mut self, location: ProgramSite) -> FormulaFailure {
        self.zero_divisor = true;
        super::undefined(location)
    }

    pub(crate) fn expression(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> Result<TermKey, FormulaFailure>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        self.evaluate(
            expression,
            Mode::Strict(|slot| variable(slot).map(Some)),
            computation,
            limits,
            counters,
            location,
        )
    }

    pub(crate) fn source_expression(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> Result<TermKey, FormulaFailure>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        self.evaluate(
            expression,
            Mode::Source(|slot| variable(slot).map(Some)),
            computation,
            limits,
            counters,
            location,
        )
    }

    /// None denotes inherited arithmetic unavailability, never an unsafe variable.
    pub(crate) fn source_partial(
        &mut self,
        expression: &Expression,
        variable: impl Fn(usize) -> Input,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        self.evaluate(
            expression,
            Mode::Partial(variable),
            computation,
            limits,
            counters,
            location,
        )
    }

    pub(crate) fn source_values<const N: usize>(
        &mut self,
        expressions: [&Expression; N],
        variable: impl Fn(usize) -> Result<TermKey, FormulaFailure>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<[TermKey; N], FormulaFailure> {
        let mut failures = Failures::default();
        let mut values: [Option<TermKey>; N] = std::array::from_fn(|_| None);
        for (expression, value) in expressions.into_iter().zip(&mut values) {
            let result = self.source_expression(
                expression,
                &variable,
                computation,
                limits,
                counters,
                location,
            );
            *value = failures.value(result, self.zero_divisor())?;
        }
        failures.finish(self)?;
        Ok(values.map(|value| value.expect("every independent component is defined")))
    }

    pub(crate) fn source_tuple(
        &mut self,
        (left, right): (&[Expression], &[Expression]),
        variable: impl Fn(usize) -> Result<TermKey, FormulaFailure>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        let mut equal = left.len() == right.len();
        let mut failures = Failures::default();
        for index in 0..left.len().max(right.len()) {
            let result = match (left.get(index), right.get(index)) {
                (Some(left), Some(right)) => self
                    .source_values(
                        [left, right],
                        &variable,
                        computation,
                        limits,
                        counters,
                        location,
                    )
                    .and_then(|[left, right]| {
                        let read = computation.read();
                        let left = read.term(&left).map_err(|error| scope(error, location))?;
                        let right = read.term(&right).map_err(|error| scope(error, location))?;
                        left.equals_ref_with(right, || counters.work(limits, location))
                    }),
                (Some(value), None) | (None, Some(value)) => self
                    .source_expression(value, &variable, computation, limits, counters, location)
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

    fn evaluate(
        &mut self,
        expression: &Expression,
        mode: Mode<impl Fn(usize) -> Input>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        self.zero_divisor = false;
        counters.record(Event::ExpressionEvaluation);
        self.scratch
            .begin(computation, limits, counters, location)?;
        let needs_mask = matches!(&mode, Mode::Partial(_))
            || matches!(&mode, Mode::Source(_)) && super::family::expression(expression);
        let variable = match mode {
            Mode::Strict(variable) | Mode::Source(variable) | Mode::Partial(variable) => variable,
        };
        let frame = Frame {
            scratch: &mut self.scratch,
            location,
        };
        let mut context = Context {
            variable: &variable,
            computation,
            limits,
            counters,
            location,
            zero_divisor: &mut self.zero_divisor,
        };
        // A leaf already names its complete canonical result. Returning that
        // authenticated identity avoids resolving a numeric value only to
        // construct it again. Keep the ordinary workspace admission and frame
        // cleanup, and spend node work before reading either source kind.
        match expression.nodes.as_slice() {
            [Operation::Constant(scalar)] => {
                context.admit()?;
                return context
                    .computation
                    .static_key(*scalar, limits, context.counters, location);
            }
            [Operation::Variable(slot)] => {
                context.admit()?;
                let Some(key) = (context.variable)(*slot)? else {
                    *context.zero_divisor = true;
                    return Err(super::undefined(location));
                };
                context.counters.work(limits, location)?;
                context
                    .computation
                    .read()
                    .term(&key)
                    .map_err(|error| scope(error, location))?;
                return Ok(key);
            }
            _ => {}
        }
        if needs_mask {
            frame.scratch.mask(expression.nodes.len(), &mut context)?;
        }
        let result = match integer_prefix(frame.scratch, &expression.nodes, &mut context)? {
            Prefix::Complete(number) => {
                if frame.scratch.missing.last().copied().unwrap_or(false) {
                    None
                } else {
                    Some(
                        context
                            .computation
                            .number(number, limits, context.counters, location)?,
                    )
                }
            }
            Prefix::Ended { evaluated, first } => value_suffix(
                frame.scratch,
                &expression.nodes,
                evaluated,
                first,
                &mut context,
            )?,
        };
        if let Some(value) = result {
            Ok(value)
        } else {
            *context.zero_divisor = true;
            Err(super::undefined(location))
        }
    }
}

fn dependent(node: &Operation, scratch: &Scratch) -> bool {
    let missing = |index| scratch.missing.get(index).copied().unwrap_or(false);
    match node {
        Operation::Unary(_, argument) | Operation::Absolute(argument) => missing(*argument),
        Operation::Binary(_, left, right) => missing(*left) || missing(*right),
        Operation::Constructor(constructor) => constructor.arguments.iter().copied().any(missing),
        Operation::Constant(_) | Operation::Variable(_) => false,
    }
}

enum Mode<V> {
    Strict(V),
    Source(V),
    Partial(V),
}

/// Arithmetic failures of independent components; resource refusals are immediate.
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

struct Context<'c, 'owner, 'source, V: Fn(usize) -> Input> {
    variable: &'c V,
    computation: &'c mut Computation<'owner, 'source>,
    limits: &'c FormulaLimits,
    counters: &'c mut Counters,
    location: ProgramSite,
    zero_divisor: &'c mut bool,
}
impl<V: Fn(usize) -> Input> Context<'_, '_, '_, V> {
    fn binary(&mut self, operator: BinaryOp, left: i32, right: i32) -> Result<i32, EvalError> {
        *self.zero_divisor = matches!(operator, BinaryOp::Div | BinaryOp::Mod) && right == 0;
        crate::scalar_arithmetic::binary(operator, left, right)
    }
    fn admit(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)?;
        self.counters.record(Event::ExpressionNode);
        Ok(())
    }
    fn retain<T>(
        &mut self,
        result: Result<T, FormulaFailure>,
        index: usize,
        scratch: &mut Scratch,
    ) -> Result<Option<T>, FormulaFailure> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. }))
                if *self.zero_divisor && !scratch.missing.is_empty() =>
            {
                scratch.missing[index] = true;
                *self.zero_divisor = false;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
    fn numeric(&mut self, key: &TermKey) -> Result<i32, FormulaFailure> {
        self.counters.work(self.limits, self.location)?;
        let value = self
            .computation
            .read()
            .term(key)
            .map_err(|error| scope(error, self.location))?;
        match value.descriptor() {
            ValueNodeRef::Number(value) => Ok(value),
            _ => Err(super::undefined(self.location)),
        }
    }
    fn number(&mut self, value: Result<i32, EvalError>) -> Result<TermKey, FormulaFailure> {
        let value = value.map_err(|error| ExpansionFailure::Evaluation {
            error,
            location: self.location,
        })?;
        self.computation
            .number(value, self.limits, self.counters, self.location)
    }
}

enum Prefix {
    Complete(i32),
    Ended { evaluated: usize, first: TermKey },
}

fn integer_prefix<V: Fn(usize) -> Input>(
    scratch: &mut Scratch,
    nodes: &[Operation],
    context: &mut Context<'_, '_, '_, V>,
) -> Result<Prefix, FormulaFailure> {
    for (index, node) in nodes.iter().enumerate() {
        context.admit()?;
        if dependent(node, scratch) {
            scratch.missing[index] = true;
            if index + 1 == nodes.len() {
                return Ok(Prefix::Complete(0));
            }
            scratch.integer(0, context)?;
            continue;
        }
        let result = match node {
            Operation::Constant(scalar) => {
                let value = context.computation.static_scalar(
                    *scalar,
                    context.limits,
                    context.counters,
                    context.location,
                )?;
                if let ValueNodeRef::Number(number) = value.descriptor() {
                    Ok(number)
                } else {
                    let first = context.computation.static_key(
                        *scalar,
                        context.limits,
                        context.counters,
                        context.location,
                    )?;
                    return Ok(Prefix::Ended {
                        evaluated: index + 1,
                        first,
                    });
                }
            }
            Operation::Variable(variable) => {
                if let Some(key) = (context.variable)(*variable)? {
                    context.counters.work(context.limits, context.location)?;
                    if let ValueNodeRef::Number(number) = context
                        .computation
                        .read()
                        .term(&key)
                        .map_err(|error| scope(error, context.location))?
                        .descriptor()
                    {
                        Ok(number)
                    } else {
                        return Ok(Prefix::Ended {
                            evaluated: index + 1,
                            first: key,
                        });
                    }
                } else {
                    scratch.missing[index] = true;
                    Ok(0)
                }
            }
            Operation::Unary(operator, argument) => {
                crate::scalar_arithmetic::unary(*operator, scratch.integers[*argument])
            }
            Operation::Binary(operator, left, right) => {
                context.binary(*operator, scratch.integers[*left], scratch.integers[*right])
            }
            Operation::Absolute(argument) => {
                crate::scalar_arithmetic::absolute(scratch.integers[*argument])
            }
            Operation::Constructor(constructor) => {
                scratch.materialize(context)?;
                let first = constructor.evaluate(
                    scratch.terms().as_slice(),
                    context.computation,
                    context.limits,
                    context.counters,
                    context.location,
                )?;
                return Ok(Prefix::Ended {
                    evaluated: index + 1,
                    first,
                });
            }
        }
        .map_err(|error| {
            FormulaFailure::from(ExpansionFailure::Evaluation {
                error,
                location: context.location,
            })
        });
        let number = context.retain(result, index, scratch)?.unwrap_or(0);
        if index + 1 == nodes.len() {
            return Ok(Prefix::Complete(number));
        }
        scratch.integer(number, context)?;
    }
    unreachable!("an admitted expression has a root")
}

fn value_suffix<V: Fn(usize) -> Input>(
    scratch: &mut Scratch,
    nodes: &[Operation],
    evaluated: usize,
    first: TermKey,
    context: &mut Context<'_, '_, '_, V>,
) -> Result<Option<TermKey>, FormulaFailure> {
    scratch.materialize(context)?;
    if evaluated == nodes.len() {
        return Ok(Some(first));
    }
    scratch.push(Some(&first), context)?;
    let (root, prefix) = nodes.split_last().expect("expression has a root");
    for (index, node) in prefix.iter().enumerate().skip(evaluated) {
        let value = source_node(node, index, scratch, context)?;
        scratch.push(value.as_ref(), context)?;
    }
    source_node(root, nodes.len() - 1, scratch, context)
}

fn source_node<V: Fn(usize) -> Input>(
    node: &Operation,
    index: usize,
    scratch: &mut Scratch,
    context: &mut Context<'_, '_, '_, V>,
) -> Result<Option<TermKey>, FormulaFailure> {
    context.admit()?;
    if dependent(node, scratch) {
        scratch.missing[index] = true;
        return Ok(None);
    }
    let result = match node {
        Operation::Constructor(constructor) => constructor.evaluate(
            scratch.terms().as_slice(),
            context.computation,
            context.limits,
            context.counters,
            context.location,
        ),
        Operation::Constant(scalar) => context.computation.static_key(
            *scalar,
            context.limits,
            context.counters,
            context.location,
        ),
        Operation::Variable(variable) => {
            if let Some(value) = (context.variable)(*variable)? {
                Ok(value)
            } else {
                scratch.missing[index] = true;
                return Ok(None);
            }
        }
        Operation::Unary(operator, argument) => {
            let argument = numeric_slot(scratch, *argument, context)?;
            context.number(crate::scalar_arithmetic::unary(*operator, argument))
        }
        Operation::Binary(operator, left, right) => {
            let left = numeric_slot(scratch, *left, context)?;
            let right = numeric_slot(scratch, *right, context)?;
            let result = context.binary(*operator, left, right);
            context.number(result)
        }
        Operation::Absolute(argument) => {
            let argument = numeric_slot(scratch, *argument, context)?;
            context.number(crate::scalar_arithmetic::absolute(argument))
        }
    };
    context.retain(result, index, scratch)
}

fn numeric_slot<V: Fn(usize) -> Input>(
    scratch: &Scratch,
    slot: usize,
    context: &mut Context<'_, '_, '_, V>,
) -> Result<i32, FormulaFailure> {
    let key = scratch
        .terms()
        .key(slot)
        .map_err(|error| crate::formula_binding::assignment(error, context.location))?
        .ok_or_else(|| {
            crate::formula_binding::assignment(AssignmentError::Unbound { slot }, context.location)
        })?;
    context.numeric(&key)
}

fn scope(error: zetesis_core::catalog::ReadError, location: ProgramSite) -> FormulaFailure {
    crate::formula_binding::assignment(AssignmentError::Read(error), location)
}
