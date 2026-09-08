//! Feature-independent views of submitted per-binding transport observations.

/// Requests associated with one buffer binding in submitted lazy chunks.
/// Allocation and reuse sum to the observed dispatch count for this binding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LazyBufferUsage {
    /// Submitted chunks requesting a new buffer for this binding.
    pub allocations: u64,
    /// Submitted chunks retaining this binding's existing buffer.
    pub reuses: u64,
}

/// Per-binding observations, separate from whole-set replacement causes.
/// Counts describe authored buffer requests, not driver allocations or RSS.
/// Views remain available without the optional GPU dependency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LazyTransportUsage {
    /// Dispatch dimensions.
    pub uniform: LazyBufferUsage,
    /// Source-row offsets.
    pub offsets: LazyBufferUsage,
    /// Source-instance records.
    pub records: LazyBufferUsage,
    /// Immutable candidate snapshots.
    pub snapshots: LazyBufferUsage,
    /// Frozen candidate seeds.
    pub seeds: LazyBufferUsage,
    /// Exact-sized device output.
    pub output: LazyBufferUsage,
    /// Exact-sized host readback.
    pub readback: LazyBufferUsage,
    /// Submissions releasing input slack to fit the prospective byte ceiling.
    pub budget_releases: u64,
    /// Submissions releasing input slack after prospective accounting overflow.
    pub accounting_overflow_releases: u64,
}

#[cfg(feature = "gpu")]
impl LazyBufferUsage {
    fn record(self, observed: zetesis_wgpu::LazyBufferUsage) -> Option<Self> {
        Some(Self {
            allocations: self.allocations.checked_add(observed.allocations)?,
            reuses: self.reuses.checked_add(observed.reuses)?,
        })
    }
}

#[cfg(feature = "gpu")]
impl LazyTransportUsage {
    pub(super) fn record(self, observed: zetesis_wgpu::LazyTransportUsage) -> Option<Self> {
        Some(Self {
            uniform: self.uniform.record(observed.uniform)?,
            offsets: self.offsets.record(observed.offsets)?,
            records: self.records.record(observed.records)?,
            snapshots: self.snapshots.record(observed.snapshots)?,
            seeds: self.seeds.record(observed.seeds)?,
            output: self.output.record(observed.output)?,
            readback: self.readback.record(observed.readback)?,
            budget_releases: self.budget_releases.checked_add(observed.budget_releases)?,
            accounting_overflow_releases: self
                .accounting_overflow_releases
                .checked_add(observed.accounting_overflow_releases)?,
        })
    }
}
