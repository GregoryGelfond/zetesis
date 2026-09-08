//! Checked observations of capacity decisions, separate from allocation effects.

use super::plan::{Allowance, Transition};
use crate::{GpuError, GpuErrorKind};

/// Why submitted lazy chunks requested a complete new transport allocation.
///
/// Each field counts submissions, not buffers or allocation bytes. The four
/// growth fields identify input bindings whose active size exceeded their old
/// capacity. Several fields may increase for the same replacement; their sum
/// is therefore not a replacement count. Initial allocation is disjoint from
/// the other reasons, and reuse increases none of them. All observations refer
/// to authored buffer payload and the configured host allowance, not driver
/// allocation or process memory. Public fields are observations, not receipts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LazyTransportReplacements {
    /// No transport was retained from an earlier submission in this batch.
    pub initial: u64,
    /// The source-offset binding needed more bytes than its old capacity.
    pub offsets_growth: u64,
    /// The source-instance binding needed more bytes than its old capacity.
    pub records_growth: u64,
    /// The immutable-snapshot binding needed more bytes than its old capacity.
    pub snapshots_growth: u64,
    /// The frozen-seed binding needed more bytes than its old capacity.
    pub seeds_growth: u64,
    /// Output/readback length changed; both retain exact active capacity.
    pub result_shape: u64,
    /// Old GPU capacity plus active host allowance exceeded the byte ceiling.
    pub budget: u64,
    /// That retained-capacity sum could not be represented as a `u64`.
    /// The already admitted exact active shape remains available for replacement.
    pub accounting_overflow: u64,
}

impl LazyTransportReplacements {
    /// Combine observations without changing either operand; constant time.
    /// Returns `None` if any field cannot represent the combined count.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            initial: self.initial.checked_add(other.initial)?,
            offsets_growth: self.offsets_growth.checked_add(other.offsets_growth)?,
            records_growth: self.records_growth.checked_add(other.records_growth)?,
            snapshots_growth: self.snapshots_growth.checked_add(other.snapshots_growth)?,
            seeds_growth: self.seeds_growth.checked_add(other.seeds_growth)?,
            result_shape: self.result_shape.checked_add(other.result_shape)?,
            budget: self.budget.checked_add(other.budget)?,
            accounting_overflow: self
                .accounting_overflow
                .checked_add(other.accounting_overflow)?,
        })
    }

    pub(super) fn record(self, transition: Transition) -> Result<Self, GpuError> {
        let observation = match transition {
            Transition::Initial => Self {
                initial: 1,
                ..Self::default()
            },
            Transition::Reuse => Self::default(),
            Transition::Replace(causes) => Self {
                offsets_growth: u64::from(causes.growth[0]),
                records_growth: u64::from(causes.growth[1]),
                snapshots_growth: u64::from(causes.growth[2]),
                seeds_growth: u64::from(causes.growth[3]),
                result_shape: u64::from(causes.result_shape),
                budget: u64::from(causes.allowance == Allowance::Exceeded),
                accounting_overflow: u64::from(causes.allowance == Allowance::Overflow),
                ..Self::default()
            },
        };
        self.checked_add(observation).ok_or_else(|| {
            GpuError::new(
                GpuErrorKind::Capacity,
                "lazy transport replacement counter overflow",
            )
        })
    }
}
