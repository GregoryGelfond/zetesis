//! Checked observations of capacity decisions, separate from allocation effects.

use super::plan::{Allowance, Selection, Transition};
use crate::{GpuError, GpuErrorKind};

/// Why submitted lazy chunks could not reuse the complete old transport.
///
/// Each field counts submissions, not buffers or allocation bytes. The four
/// growth fields identify input bindings whose active size exceeded their old
/// capacity. Several fields may increase for the same replacement; their sum
/// is therefore not a replacement count. Fitting buffers can still survive a
/// replacement, as recorded by [`LazyTransportUsage`]. Initial allocation is disjoint from
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

/// Requested use of one buffer on submitted chunks. Allocation counts authored
/// requests, not driver allocations; reuse preserves a handle, not logical truth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LazyBufferUsage {
    /// Submitted chunks requesting a new buffer at this binding.
    pub allocations: u64,
    /// Submitted chunks retaining this binding's prior buffer.
    pub reuses: u64,
}

impl LazyBufferUsage {
    /// Combine observations without mutation; `None` denotes counter overflow.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            allocations: self.allocations.checked_add(other.allocations)?,
            reuses: self.reuses.checked_add(other.reuses)?,
        })
    }

    fn observed(retained: bool) -> Self {
        Self {
            allocations: u64::from(!retained),
            reuses: u64::from(retained),
        }
    }
}

/// Per-binding observations from submitted lazy chunks, including work before
/// a later failure. Each allocation/reuse pair sums to the dispatch count;
/// output and readback observations agree because their shapes remain exact.
/// The two release fields count submissions, not buffers or bytes. They concern
/// prospective retained-plus-new payload, unlike historical whole-shape reasons
/// in [`LazyTransportReplacements`]. Public observations are not allocation receipts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LazyTransportUsage {
    /// Fixed dimensions and submission identity header.
    pub uniform: LazyBufferUsage,
    /// Source-instance record offsets.
    pub offsets: LazyBufferUsage,
    /// Packed source instances.
    pub records: LazyBufferUsage,
    /// Immutable round interpretations.
    pub snapshots: LazyBufferUsage,
    /// Frozen candidate seeds.
    pub seeds: LazyBufferUsage,
    /// Exact active result buffer, cleared before each dispatch.
    pub output: LazyBufferUsage,
    /// Exact active readback buffer, unmapped before any successful reuse.
    pub readback: LazyBufferUsage,
    /// Submitted decisions releasing input slack to meet the byte ceiling.
    pub budget_releases: u64,
    /// Submitted decisions releasing input slack after capacity-sum overflow.
    pub accounting_overflow_releases: u64,
}

impl LazyTransportUsage {
    /// Combine observations without mutation; constant time and checked counts.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            uniform: self.uniform.checked_add(other.uniform)?,
            offsets: self.offsets.checked_add(other.offsets)?,
            records: self.records.checked_add(other.records)?,
            snapshots: self.snapshots.checked_add(other.snapshots)?,
            seeds: self.seeds.checked_add(other.seeds)?,
            output: self.output.checked_add(other.output)?,
            readback: self.readback.checked_add(other.readback)?,
            budget_releases: self.budget_releases.checked_add(other.budget_releases)?,
            accounting_overflow_releases: self
                .accounting_overflow_releases
                .checked_add(other.accounting_overflow_releases)?,
        })
    }

    pub(super) fn record(self, selection: Selection) -> Result<Self, GpuError> {
        let retention = selection.retention;
        self.checked_add(Self {
            uniform: LazyBufferUsage::observed(retention.uniform),
            offsets: LazyBufferUsage::observed(retention.inputs[0]),
            records: LazyBufferUsage::observed(retention.inputs[1]),
            snapshots: LazyBufferUsage::observed(retention.inputs[2]),
            seeds: LazyBufferUsage::observed(retention.inputs[3]),
            output: LazyBufferUsage::observed(retention.result),
            readback: LazyBufferUsage::observed(retention.result),
            budget_releases: u64::from(selection.slack == Allowance::Exceeded),
            accounting_overflow_releases: u64::from(selection.slack == Allowance::Overflow),
        })
        .ok_or_else(|| {
            GpuError::new(
                GpuErrorKind::Capacity,
                "lazy transport usage counter overflow",
            )
        })
    }
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
