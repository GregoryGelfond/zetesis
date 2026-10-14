//! Integer comparison forms: a comparison literal's two operands compiled
//! once, when the compiler constructs the literal, for deciding it with
//! machine integers.
//!
//! An operand admits a form when it has between one and [`MAX_NODES`] nodes
//! and every node is a variable whose slot fits a `u32`, a numeric constant,
//! or a unary, binary or absolute-value operation whose arguments name earlier
//! nodes of the same operand. The form keeps both operands' nodes in order as
//! steps, the left operand's then the right's: a constant becomes its `i32`
//! value, a variable keeps its slot and an operation keeps its argument
//! positions. Attempting it on a row computes, node by node, the integers the
//! general evaluator's integer prefix computes, with the same checked scalar
//! operations, and constructs no term. [`WithoutForm`] names why a literal has
//! none.
//!
//! A form is built only with its literal ([`super::Comparison`]) and is never
//! changed afterwards, so it is valid for the literal's lifetime, the prepared
//! program's. It borrows nothing and retains `8·n` heap bytes for `n` steps.

use std::cmp::Ordering;

use themelios_program::term::{BinaryOp, UnaryOp};

use super::Operation;
use crate::formula_support::components::Scalar;

/// Operands with more nodes keep the general path: an operand evaluates in a
/// fixed array of this many integers, so no row allocates.
pub(crate) const MAX_NODES: usize = 16;

/// Why a comparison literal has no integer comparison form. A literal counts
/// under the first failure the compiler meets: the left operand's shape, the
/// right operand's shape, then the constants, left operand before right.
/// Within an operand's shape, its node count comes before the first failing
/// node in node order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WithoutForm {
    /// An operand has more than [`MAX_NODES`] nodes, or none.
    LongOperand,
    /// A variable slot does not fit a `u32`.
    WideSlot,
    /// A node constructs a function or tuple term.
    Constructor,
    /// A constant is not a number.
    NonnumericConstant,
    /// An operation's argument names itself or a later node.
    ArgumentPosition,
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

/// A comparison literal's operands compiled for machine integers: the left
/// operand's steps then the right's, where the right operand begins, and the
/// constant an attempt is checked for.
#[derive(Debug)]
pub(crate) struct IntegerComparison {
    steps: Box<[Step]>,
    /// The position of the right operand's first step.
    split: u8,
    /// One unit per step and one per variable step, of both operands.
    constant: u8,
}

/// What one attempt of a form performed on a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Attempt {
    /// The order of the two operands' integers, or `None` when the attempt
    /// declined: a variable without a numeric value, or a failing operation.
    pub(crate) order: Option<Ordering>,
    /// The operands it entered: one when it declined in the left operand.
    pub(crate) entered: u8,
    /// The steps it reached, the failing one included.
    pub(crate) steps: u8,
    /// The work those steps cost: one unit per step and one per variable
    /// read. Never more than [`IntegerComparison::constant`].
    pub(crate) units: u8,
}

impl IntegerComparison {
    /// The heap bytes a form of `steps` steps retains.
    pub(super) const fn bytes(steps: usize) -> usize {
        steps * size_of::<Step>()
    }

    /// The form of `left` and `right`, which satisfy the shape rule
    /// ([`shape`]), where `numbers[operand][index]` is the value of node
    /// `index` of that operand when it is a constant.
    pub(super) fn new([left, right]: [&[Operation]; 2], numbers: [&[i32; MAX_NODES]; 2]) -> Self {
        let steps: Box<[Step]> = left
            .iter()
            .zip(numbers[0])
            .chain(right.iter().zip(numbers[1]))
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
            split: u8::try_from(left.len()).expect("an operand has at most MAX_NODES steps"),
            constant: u8::try_from(steps.len() + variables)
                .expect("two operands have at most 2·MAX_NODES steps"),
            steps,
        }
    }

    /// A copy of this form, for a copy of its literal.
    pub(super) fn copy(&self) -> Self {
        Self {
            steps: self.steps.clone(),
            split: self.split,
            constant: self.constant,
        }
    }

    /// The steps of both operands.
    pub(crate) fn steps(&self) -> usize {
        self.steps.len()
    }

    /// The work an attempt is checked for before it runs: one unit per step
    /// and one per variable read, of both operands. An attempt performs at
    /// most this much.
    pub(crate) fn constant(&self) -> u8 {
        self.constant
    }

    /// Attempt the form on a row, reading each variable's slot through
    /// `number`: the order of the two integers, or a decline when a variable
    /// has no numeric value there or an operation fails under the checked
    /// scalar contract, with what the attempt performed either way. A decline
    /// stops the attempt at the failing step.
    pub(crate) fn attempt(&self, number: impl Fn(usize) -> Option<i32>) -> Attempt {
        let (left, right) = self.steps.split_at(usize::from(self.split));
        let mut attempt = Attempt {
            order: None,
            entered: 1,
            steps: 0,
            units: 0,
        };
        let Some(left) = operand(left, &number, &mut attempt) else {
            return attempt;
        };
        attempt.entered = 2;
        let Some(right) = operand(right, &number, &mut attempt) else {
            return attempt;
        };
        attempt.order = Some(left.cmp(&right));
        attempt
    }
}

/// One operand's value, counting into `attempt` each step reached, the failing
/// one included.
fn operand(
    steps: &[Step],
    number: &impl Fn(usize) -> Option<i32>,
    attempt: &mut Attempt,
) -> Option<i32> {
    let mut values = [0_i32; MAX_NODES];
    for (index, step) in steps.iter().enumerate() {
        attempt.steps += 1;
        attempt.units += 1;
        values[index] = match *step {
            Step::Number(value) => value,
            Step::Variable(slot) => {
                attempt.units += 1;
                number(slot as usize)?
            }
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
    steps.len().checked_sub(1).map(|root| values[root])
}

/// Whether an operand's `nodes` admit a form, judged from the nodes alone:
/// the compiler reads each constant's value afterwards. Its node count is
/// judged first, then each node in order.
pub(super) fn shape(nodes: &[Operation]) -> Result<(), WithoutForm> {
    if nodes.is_empty() || nodes.len() > MAX_NODES {
        return Err(WithoutForm::LongOperand);
    }
    nodes
        .iter()
        .enumerate()
        .find_map(|(position, node)| match node {
            Operation::Constant(_) => None,
            Operation::Variable(slot) => u32::try_from(*slot)
                .is_err()
                .then_some(WithoutForm::WideSlot),
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
/// `number`: `None` when a constant is not a number, after which no further
/// constant is read.
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
