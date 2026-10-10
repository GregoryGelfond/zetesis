//! Conservative intervals for checked scalar arithmetic.
//!
//! Endpoints and every admitted intermediate lie within i32. Transfers are
//! calculated in i64 and decline when they cannot establish a defined i32
//! result for every pair of operands. Callers establish the numeric source
//! domains independently; this operation supplies neither bindings nor truth.

use themelios_program::term::{BinaryOp, UnaryOp};

#[derive(Clone, Copy)]
pub(crate) struct Range {
    low: i64,
    high: i64,
}

impl Range {
    pub(crate) fn point(value: i32) -> Self {
        Self {
            low: i64::from(value),
            high: i64::from(value),
        }
    }

    fn checked(low: i64, high: i64) -> Option<Self> {
        (low >= i64::from(i32::MIN) && high <= i64::from(i32::MAX)).then_some(Self { low, high })
    }

    /// Enlarge a numeric bound by one admitted source value.
    pub(crate) fn include(&mut self, value: i32) {
        self.low = self.low.min(i64::from(value));
        self.high = self.high.max(i64::from(value));
    }

    pub(crate) fn absolute(self) -> Option<Self> {
        let low = if self.low <= 0 && self.high >= 0 {
            0
        } else {
            self.low.abs().min(self.high.abs())
        };
        Self::checked(low, self.low.abs().max(self.high.abs()))
    }

    pub(crate) fn unary(self, operator: UnaryOp) -> Option<Self> {
        match operator {
            UnaryOp::Negate => Self::checked(-self.high, -self.low),
            UnaryOp::BitwiseNot => Self::checked(!self.high, !self.low),
        }
    }

    pub(crate) fn binary(self, operator: BinaryOp, right: Self) -> Option<Self> {
        match operator {
            BinaryOp::Add => Self::checked(self.low + right.low, self.high + right.high),
            BinaryOp::Sub => Self::checked(self.low - right.high, self.high - right.low),
            BinaryOp::Mul => self.corners(right, |left, right| Some(left * right)),
            BinaryOp::Div | BinaryOp::Mod
                if (right.low <= 0 && right.high >= 0)
                    || (self.low == i64::from(i32::MIN) && right.low <= -1 && right.high >= -1) =>
            {
                None
            }
            BinaryOp::Div => self.corners(right, |left, right| Some(left / right)),
            BinaryOp::Mod => {
                let magnitude = right.low.abs().max(right.high.abs()) - 1;
                Self::checked(
                    self.low.min(0).max(-magnitude),
                    self.high.max(0).min(magnitude),
                )
            }
            BinaryOp::Pow if right.low == right.high && right.low >= 0 => {
                let exponent = u32::try_from(right.low).ok()?;
                let bound = self.corners(right, |left, _| left.checked_pow(exponent))?;
                Some(Self {
                    low: if exponent > 0 && self.low <= 0 && self.high >= 0 {
                        bound.low.min(0)
                    } else {
                        bound.low
                    },
                    high: bound.high,
                })
            }
            BinaryOp::Pow => None,
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                Self::checked(i64::from(i32::MIN), i64::from(i32::MAX))
            }
        }
    }

    fn corners(self, right: Self, operation: impl Fn(i64, i64) -> Option<i64>) -> Option<Self> {
        let corners = [
            operation(self.low, right.low)?,
            operation(self.low, right.high)?,
            operation(self.high, right.low)?,
            operation(self.high, right.high)?,
        ];
        Self::checked(*corners.iter().min()?, *corners.iter().max()?)
    }
}
