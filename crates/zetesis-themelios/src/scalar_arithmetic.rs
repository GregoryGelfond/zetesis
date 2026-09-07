//! Checked arithmetic on already evaluated numeric operands.
//!
//! This is the scalar contract of the pinned `themelios-program` evaluator,
//! without constructing and cloning temporary `Term` trees. The upstream
//! operator helpers are private; differential tests use its public evaluator.
//! Non-numeric values, source locations, operation order and resource accounting
//! remain with the caller. No constructor or source syntax is interpreted here.
//!
//! Operations allocate no storage. Unary, absolute and non-power binary work is
//! constant; checked power uses exponentiation by squaring on a nonnegative i32
//! exponent. A negative exponent is undefined, including powers of zero or one.

use themelios_program::term::{BinaryOp, EvalError, UnaryOp};

pub(super) fn unary(operator: UnaryOp, operand: i32) -> Result<i32, EvalError> {
    match operator {
        UnaryOp::Negate => operand.checked_neg().ok_or(EvalError::Overflow),
        UnaryOp::BitwiseNot => Ok(!operand),
    }
}

pub(super) fn binary(operator: BinaryOp, left: i32, right: i32) -> Result<i32, EvalError> {
    match operator {
        BinaryOp::Add => left.checked_add(right).ok_or(EvalError::Overflow),
        BinaryOp::Sub => left.checked_sub(right).ok_or(EvalError::Overflow),
        BinaryOp::Mul => left.checked_mul(right).ok_or(EvalError::Overflow),
        BinaryOp::Div | BinaryOp::Mod if right == 0 => Err(EvalError::Undefined),
        BinaryOp::Div => left.checked_div(right).ok_or(EvalError::Overflow),
        // MIN % -1 refuses even though the mathematical remainder is zero:
        // the pinned machine operation checks the unrepresentable quotient.
        BinaryOp::Mod => left.checked_rem(right).ok_or(EvalError::Overflow),
        BinaryOp::Pow if right < 0 => Err(EvalError::Undefined),
        BinaryOp::Pow => {
            let exponent = u32::try_from(right).map_err(|_| EvalError::Overflow)?;
            left.checked_pow(exponent).ok_or(EvalError::Overflow)
        }
        BinaryOp::BitAnd => Ok(left & right),
        BinaryOp::BitOr => Ok(left | right),
        BinaryOp::BitXor => Ok(left ^ right),
    }
}

pub(super) fn absolute(operand: i32) -> Result<i32, EvalError> {
    operand.checked_abs().ok_or(EvalError::Overflow)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;
