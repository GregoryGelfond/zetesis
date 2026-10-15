//! Decide identity, complete-scan bounds and cache replacement before effects.

use super::packing::{Graph, PARAM_BYTES, Packed, Packing, Plan};
use super::{TightGpuBatchStats, TightGpuLimits, poll};
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, TightPlan};

pub(super) struct Admission {
    pub(super) fresh: Option<Graph>,
    pub(super) plan: Plan,
    pub(super) stats: TightGpuBatchStats,
}

impl Admission {
    // No device handles are mutated here. Refusal preserves existing residency;
    // an accepted replacement must evict its old buffers before host packing.
    pub(super) fn new(
        certificate: &TightPlan,
        candidates: &[Interpretation],
        limits: TightGpuLimits,
        packing: Packing<'_>,
        cached: Option<(&Graph, bool)>,
        prior_epoch: u32,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        poll(cancellation)?;
        let epoch = prior_epoch
            .checked_add(1)
            .ok_or_else(|| GpuError::new(GpuErrorKind::Capacity, "tight epoch exhausted"))?;
        let fresh =
            if cached.is_some_and(|(graph, _)| graph.theory.same_instance(certificate.theory())) {
                None
            } else {
                Some(Graph::with_control(
                    certificate,
                    packing,
                    limits.max_batch_bytes,
                    cancellation,
                )?)
            };
        let graph = fresh
            .as_ref()
            .or_else(|| cached.map(|(graph, _)| graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing tight graph"))?;
        let plan = Plan::new(
            graph,
            candidates.len(),
            limits,
            packing.device,
            fresh.is_some(),
            epoch,
        )?;
        for candidate in candidates {
            poll(cancellation)?;
            if !certificate.theory().same_instance(candidate.theory()) {
                return Err(GpuError::new(
                    GpuErrorKind::Seed,
                    "candidate belongs to another Theory",
                ));
            }
        }
        let stats = batch_stats(
            graph,
            &plan,
            fresh.is_some(),
            cached.is_none_or(|(_, matches)| !matches),
        );
        Ok(Self { fresh, plan, stats })
    }
    pub(super) fn pack(
        &mut self,
        certificate: &TightPlan,
        limits: TightGpuLimits,
        device: &wgpu::Limits,
        cancellation: &Cancellation,
    ) -> Result<Option<Packed>, GpuError> {
        let Some(graph) = self.fresh.as_mut() else {
            return Ok(None);
        };
        let packed = graph.prepare(certificate, limits, device, &mut self.plan, cancellation)?;
        self.stats = batch_stats(graph, &self.plan, true, true);
        Ok(Some(packed))
    }
}

pub(super) fn batch_stats(
    graph: &Graph,
    plan: &Plan,
    fresh: bool,
    transport_allocated: bool,
) -> TightGpuBatchStats {
    TightGpuBatchStats {
        theory_uploaded: fresh,
        transport_allocated: fresh || transport_allocated,
        resident_theory_bytes: graph.bytes,
        resident_transport_bytes: plan.transport,
        accounted_bytes: plan.accounted,
        candidates: u64::from(plan.worlds),
        work: u64::from(plan.worlds) * u64::from(plan.work),
        dispatches: 1,
        truth_levels: plan.levels,
        original_truth_reused: plan.original_truth_reused,
        uploaded_bytes: PARAM_BYTES + plan.seeds + if fresh { graph.bytes } else { 0 },
        downloaded_bytes: plan.results,
    }
}

#[cfg(test)]
mod tests;
