//! Conservative intervals for checked scalar arithmetic.
//!
//! Endpoints and every admitted intermediate lie within i32. Transfers are
//! calculated in i64 and decline when they cannot establish a defined i32
//! result for every pair of operands. Callers establish the numeric source
//! domains independently; this operation supplies neither bindings nor truth.

use themelios_program::term::{BinaryOp, UnaryOp};

/// A nonempty interval of language integers. A successful transfer encloses
/// every checked result over its inputs; a declined transfer makes no claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    low: i32,
    high: i32,
}

impl Range {
    /// Construct inclusive bounds, declining a descending interval.
    #[must_use]
    pub fn new(low: i32, high: i32) -> Option<Self> {
        Self::checked(i64::from(low), i64::from(high))
    }

    /// Inclusive endpoints, each within the language's integer representation.
    #[must_use]
    pub fn bounds(self) -> (i32, i32) {
        (self.low, self.high)
    }

    /// The interval containing exactly one integer.
    #[must_use]
    pub fn point(value: i32) -> Self {
        Self {
            low: value,
            high: value,
        }
    }

    fn checked(low: i64, high: i64) -> Option<Self> {
        if low > high {
            return None;
        }
        Some(Self {
            low: i32::try_from(low).ok()?,
            high: i32::try_from(high).ok()?,
        })
    }

    /// Enlarge a numeric bound by one admitted source value.
    pub fn include(&mut self, value: i32) {
        self.low = self.low.min(value);
        self.high = self.high.max(value);
    }

    /// Checked positive bounds, without changing scalar arithmetic semantics.
    #[must_use]
    pub fn positive_bounds(self) -> Option<(i32, i32)> {
        (self.low > 0).then_some(self.bounds())
    }

    /// Enclose absolute values, declining when `i32::MIN` is possible.
    #[must_use]
    pub fn absolute(self) -> Option<Self> {
        let (low, high) = (i64::from(self.low), i64::from(self.high));
        let upper = low.abs().max(high.abs());
        let lower = if low <= 0 && high >= 0 {
            0
        } else {
            low.abs().min(high.abs())
        };
        Self::checked(lower, upper)
    }

    /// Enclose a unary operation when every input has a defined i32 result.
    #[must_use]
    pub fn unary(self, operator: UnaryOp) -> Option<Self> {
        let (low, high) = (i64::from(self.low), i64::from(self.high));
        match operator {
            UnaryOp::Negate => Self::checked(-high, -low),
            UnaryOp::BitwiseNot => Self::checked(!high, !low),
        }
    }

    /// Enclose a binary operation over all pairs in the two intervals.
    /// Decline undefined, overflowing or unsupported interval transfers.
    #[must_use]
    pub fn binary(self, operator: BinaryOp, right: Self) -> Option<Self> {
        let (low, high) = (i64::from(self.low), i64::from(self.high));
        let (right_low, right_high) = (i64::from(right.low), i64::from(right.high));
        match operator {
            BinaryOp::Add => Self::checked(low + right_low, high + right_high),
            BinaryOp::Sub => Self::checked(low - right_high, high - right_low),
            BinaryOp::Mul => self.corners(right, |left, right| Some(left * right)),
            BinaryOp::Div | BinaryOp::Mod
                if (right_low <= 0 && right_high >= 0)
                    || (low == i64::from(i32::MIN) && right_low <= -1 && right_high >= -1) =>
            {
                None
            }
            BinaryOp::Div => self.corners(right, |left, right| Some(left / right)),
            BinaryOp::Mod => {
                let magnitude = right_low.abs().max(right_high.abs()) - 1;
                Self::checked(low.min(0).max(-magnitude), high.max(0).min(magnitude))
            }
            BinaryOp::Pow if right_low == right_high && right_low >= 0 => {
                let exponent = u32::try_from(right_low).ok()?;
                let bound = self.corners(right, |left, _| left.checked_pow(exponent))?;
                Some(Self {
                    low: if exponent > 0 && low <= 0 && high >= 0 {
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
        let (low, high) = (i64::from(self.low), i64::from(self.high));
        let (right_low, right_high) = (i64::from(right.low), i64::from(right.high));
        let corners = [
            operation(low, right_low)?,
            operation(low, right_high)?,
            operation(high, right_low)?,
            operation(high, right_high)?,
        ];
        Self::checked(*corners.iter().min()?, *corners.iter().max()?)
    }
}
