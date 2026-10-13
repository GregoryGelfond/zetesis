//! Integer operand forms: comparison operands compiled once, with the prepared
//! program, for evaluation with machine integers.
//!
//! An expression has a form when it has between one and [`MAX_NODES`] nodes
//! and every node is a variable whose slot fits a `u32`, a numeric constant,
//! or a unary, binary or absolute-value operation whose arguments name earlier
//! nodes. The form keeps the nodes in order as steps: a constant becomes its
//! `i32` value, a variable keeps its slot and an operation keeps its argument
//! positions. Evaluating it on a row computes, node by node, the integer the
//! general evaluator's integer prefix computes, with the same checked scalar
//! operations, and constructs no term. [`WithoutForm`] names why an
//! expression has none.
//!
//! A form is built only by the compiler, for the operands of the comparison
//! literals it compiles, is copied only with its expression, and is never
//! changed afterwards: no code mutates an expression's nodes after
//! construction, so a form is valid for its expression's lifetime, the
//! prepared program's. It borrows nothing and retains `8·n` heap bytes for `n`
//! steps.

use themelios_program::term::{BinaryOp, UnaryOp};

use super::Operation;
use crate::formula_support::components::Scalar;

/// Operands with more nodes keep the general path: an operand evaluates in a
/// fixed array of this many integers, so no row allocates.
pub(crate) const MAX_NODES: usize = 16;

/// Why an expression has no integer operand form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WithoutForm {
    /// It has no node or more than [`MAX_NODES`], or a variable slot that
    /// does not fit a `u32`.
    Bound,
    /// A node constructs a function or tuple term.
    Constructor,
    /// A constant is not a number.
    NonnumericConstant,
    /// An operation's argument names itself or a later node.
    ArgumentPosition,
    /// It was built by [`super::Expression::new`], which compiles no form.
    NotCompiled,
}

/// One compiled node. Argument positions name earlier steps of the same
/// operand.
#[derive(Clone, Copy, Debug)]
enum Step {
    Number(i32),
    Variable(u32),
    Unary(UnaryOp, u8),
    Binary(BinaryOp, u8, u8),
    Absolute(u8),
}

/// An expression compiled for machine integers: its steps in node order and
/// the constant work an evaluation of it is charged.
#[derive(Debug)]
pub(crate) struct IntegerOperand {
    steps: Box<[Step]>,
    /// One unit per step and one per variable step.
    charge: u8,
}

impl IntegerOperand {
    /// The heap bytes a form of `steps` steps retains.
    pub(super) const fn bytes(steps: usize) -> usize {
        steps * size_of::<Step>()
    }

    /// The form of `nodes`, which satisfy the shape rule ([`shape`]), where
    /// `numbers[index]` is the value of node `index` when it is a constant.
    pub(super) fn new(nodes: &[Operation], numbers: &[i32]) -> Self {
        let steps: Box<[Step]> = nodes
            .iter()
            .zip(numbers)
            .map(|(node, number)| match node {
                Operation::Constant(_) => Step::Number(*number),
                Operation::Variable(slot) => {
                    Step::Variable(u32::try_from(*slot).expect("the shape rule bounds slots"))
                }
                Operation::Unary(operator, argument) => Step::Unary(*operator, position(*argument)),
                Operation::Binary(operator, left, right) => {
                    Step::Binary(*operator, position(*left), position(*right))
                }
                Operation::Absolute(argument) => Step::Absolute(position(*argument)),
                Operation::Constructor(_) => unreachable!("the shape rule excludes constructors"),
            })
            .collect();
        let variables = steps
            .iter()
            .filter(|step| matches!(step, Step::Variable(_)))
            .count();
        Self {
            charge: u8::try_from(steps.len() + variables)
                .expect("an operand has at most MAX_NODES steps"),
            steps,
        }
    }

    /// A copy of this form, for a copy of its expression.
    pub(super) fn copy(&self) -> Self {
        Self {
            steps: self.steps.clone(),
            charge: self.charge,
        }
    }

    /// Its node count.
    pub(crate) fn nodes(&self) -> usize {
        self.steps.len()
    }

    /// The work an evaluation is charged, whether it then decides or
    /// declines: one unit per node and one per variable read.
    pub(crate) fn charge(&self) -> u8 {
        self.charge
    }

    /// The operand's value, reading each variable's slot through `number`:
    /// `None` when a variable has no numeric value there, or an operation
    /// fails under the checked scalar contract.
    pub(crate) fn evaluate(&self, number: impl Fn(usize) -> Option<i32>) -> Option<i32> {
        let mut values = [0_i32; MAX_NODES];
        for (index, step) in self.steps.iter().enumerate() {
            values[index] = match *step {
                Step::Number(value) => value,
                Step::Variable(slot) => number(slot as usize)?,
                Step::Unary(operator, argument) => {
                    crate::scalar_arithmetic::unary(operator, values[usize::from(argument)]).ok()?
                }
                Step::Binary(operator, left, right) => crate::scalar_arithmetic::binary(
                    operator,
                    values[usize::from(left)],
                    values[usize::from(right)],
                )
                .ok()?,
                Step::Absolute(argument) => {
                    crate::scalar_arithmetic::absolute(values[usize::from(argument)]).ok()?
                }
            };
        }
        self.steps.len().checked_sub(1).map(|root| values[root])
    }
}

/// Whether `nodes` admit a form, judged from the nodes alone: the compiler
/// reads each constant's value afterwards.
pub(super) fn shape(nodes: &[Operation]) -> Result<(), WithoutForm> {
    if nodes.is_empty() || nodes.len() > MAX_NODES {
        return Err(WithoutForm::Bound);
    }
    nodes
        .iter()
        .enumerate()
        .find_map(|(position, node)| match node {
            Operation::Constant(_) => None,
            Operation::Variable(slot) => {
                u32::try_from(*slot).is_err().then_some(WithoutForm::Bound)
            }
            Operation::Unary(_, argument) | Operation::Absolute(argument) => {
                (*argument >= position).then_some(WithoutForm::ArgumentPosition)
            }
            Operation::Binary(_, left, right) => {
                (*left >= position || *right >= position).then_some(WithoutForm::ArgumentPosition)
            }
            Operation::Constructor(_) => Some(WithoutForm::Constructor),
        })
        .map_or(Ok(()), Err)
}

/// Each constant node's numeric value at its position, read through
/// `number`: `None` when a constant is not a number.
///
/// # Errors
/// `number`'s refusal.
pub(super) fn numbers<E>(
    nodes: &[Operation],
    mut number: impl FnMut(Scalar) -> Result<Option<i32>, E>,
) -> Result<Option<[i32; MAX_NODES]>, E> {
    let mut numbers = [0; MAX_NODES];
    for (value, node) in numbers.iter_mut().zip(nodes) {
        if let Operation::Constant(scalar) = node {
            let Some(number) = number(*scalar)? else {
                return Ok(None);
            };
            *value = number;
        }
    }
    Ok(Some(numbers))
}

/// An earlier node's position, which the shape rule bounds below
/// [`MAX_NODES`].
fn position(argument: usize) -> u8 {
    u8::try_from(argument).expect("the shape rule bounds argument positions")
}

#[cfg(test)]
mod tests;
