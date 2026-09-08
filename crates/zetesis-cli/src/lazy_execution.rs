//! Typed ordinary lazy execution accounting, independent of rendered output.

/// Cumulative work from the explicitly selected lazy device executor. A selected
/// adapter does not establish execution: actual dispatch and transfer counts do.
/// Source work is shared across candidate occurrences, not a per-world CPU cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LazyExecutionStatistics {
    /// User's explicit hardware request.
    pub requested_backend: crate::Backend,
    /// Reported selected device name.
    pub adapter: String,
    /// Actual native API reported by the selected adapter.
    pub backend: String,
    /// Nonempty attempted seed batches.
    pub batches: u64,
    /// Submitted seed occurrences, including duplicates.
    pub submitted_candidates: u64,
    /// Seed occurrences with complete returned reduct checks.
    pub completed_candidates: u64,
    /// Submitted occurrences without complete results after a failed batch.
    pub stopped_candidates: u64,
    /// Fully checked results still awaiting ordinary-session consumption.
    pub queued_results: usize,
    /// Exhaustively scanned and fully evaluated rounds, including failed-batch
    /// prefixes. Completed rounds alone do not establish a completed candidate.
    pub source_rounds: u64,
    /// Charged host source operations across all batches and rounds.
    pub source_work: u64,
    /// Filter-valid source bindings offered to the bounded instance consumer.
    pub source_instances: u64,
    /// Maximum demanded catalog atoms in any attempted batch.
    pub peak_catalog_atoms: usize,
    /// Actual submitted nonempty device chunks.
    pub dispatches: u64,
    /// Submitted instance/candidate pairs, including later failed submissions.
    pub world_instances: u64,
    /// Actual uploaded uniform, rule, snapshot and frozen-seed bytes.
    pub uploaded_bytes: u64,
    /// Successfully decoded result bytes.
    pub downloaded_bytes: u64,
    /// Submitted chunks requesting a new complete transport buffer set.
    pub transport_allocations: u64,
    /// Submitted chunks reusing capacity within the same source batch.
    pub transport_reuses: u64,
    /// Maximum requested device buffer payload, including inactive capacity.
    /// This excludes host payload, driver storage and process RSS.
    pub peak_transport_bytes: u64,
    /// Host wait plus readback decoding; this is not a kernel-only timer.
    pub host_wait: std::time::Duration,
}

#[cfg(test)]
#[path = "../tests/support/lazy_accounting.rs"]
pub(crate) mod tests;

#[cfg(feature = "gpu")]
impl LazyExecutionStatistics {
    pub(crate) fn new(
        requested_backend: crate::Backend,
        metadata: zetesis_wgpu::AdapterMetadata<'_>,
    ) -> Self {
        Self {
            requested_backend,
            adapter: metadata.name.to_owned(),
            backend: metadata.backend.label().to_owned(),
            batches: 0,
            submitted_candidates: 0,
            completed_candidates: 0,
            stopped_candidates: 0,
            queued_results: 0,
            source_rounds: 0,
            source_work: 0,
            source_instances: 0,
            peak_catalog_atoms: 0,
            dispatches: 0,
            world_instances: 0,
            uploaded_bytes: 0,
            downloaded_bytes: 0,
            transport_allocations: 0,
            transport_reuses: 0,
            peak_transport_bytes: 0,
            host_wait: std::time::Duration::ZERO,
        }
    }

    pub(crate) fn record(
        &mut self,
        candidates: usize,
        complete: bool,
        source: zetesis_cpu::lazy::Progress,
        device: zetesis_wgpu::LazyGpuStatistics,
    ) -> Result<(), crate::RunError> {
        let add = |left: u64, right: u64| {
            left.checked_add(right)
                .ok_or(crate::RunError::LazyStatisticsOverflow)
        };
        let candidates =
            u64::try_from(candidates).map_err(|_| crate::RunError::LazyStatisticsOverflow)?;
        // Construct the complete next record before mutating retained evidence.
        let next = Self {
            batches: add(self.batches, 1)?,
            submitted_candidates: add(self.submitted_candidates, candidates)?,
            completed_candidates: add(
                self.completed_candidates,
                if complete { candidates } else { 0 },
            )?,
            stopped_candidates: add(
                self.stopped_candidates,
                if complete { 0 } else { candidates },
            )?,
            source_rounds: add(self.source_rounds, source.rounds)?,
            source_work: add(self.source_work, source.source_work)?,
            source_instances: add(self.source_instances, source.instances)?,
            peak_catalog_atoms: self.peak_catalog_atoms.max(source.catalog_atoms),
            dispatches: add(self.dispatches, device.dispatches)?,
            world_instances: add(self.world_instances, device.world_instances)?,
            uploaded_bytes: add(self.uploaded_bytes, device.uploaded_bytes)?,
            downloaded_bytes: add(self.downloaded_bytes, device.downloaded_bytes)?,
            transport_allocations: add(self.transport_allocations, device.transport_allocations)?,
            transport_reuses: add(self.transport_reuses, device.transport_reuses)?,
            peak_transport_bytes: self.peak_transport_bytes.max(device.peak_transport_bytes),
            host_wait: self
                .host_wait
                .checked_add(device.host_wait)
                .ok_or(crate::RunError::LazyStatisticsOverflow)?,
            ..self.clone()
        };
        *self = next;
        Ok(())
    }
}

/// Convert complete checks or an incomplete source batch into ordinary-session
/// results. Device/protocol failures remain errors; they never become rejections.
#[cfg(feature = "gpu")]
pub(crate) fn batch_results(
    result: Result<zetesis_cpu::lazy::Batch, zetesis_cpu::lazy::Failure<zetesis_wgpu::GpuError>>,
) -> Result<Vec<Result<Option<zetesis_core::Model>, zetesis_cpu::Stop>>, crate::RunError> {
    match result {
        Ok(batch) => Ok(batch
            .checks
            .into_iter()
            .map(|check| Ok(check.accepted().then(|| check.closure().clone())))
            .collect()),
        Err(failure) => match failure.cause {
            zetesis_cpu::lazy::Cause::Source(stop) => Ok(vec![Err(stop)]),
            _ => Err(crate::RunError::LazyGpu(failure)),
        },
    }
}
