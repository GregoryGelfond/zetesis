//! Decide identity, complete-scan bounds and cache replacement before effects.

use super::packing::{Graph, PARAM_BYTES, Packing, Plan};
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
                Some(Graph::new(certificate, packing)?)
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
        let stats = TightGpuBatchStats {
            theory_uploaded: fresh.is_some(),
            transport_allocated: fresh.is_some()
                || cached.is_none_or(|(_, shape_matches)| !shape_matches),
            resident_theory_bytes: graph.bytes,
            resident_transport_bytes: plan.transport,
            accounted_bytes: plan.accounted,
            candidates: u64::from(plan.worlds),
            work: u64::from(plan.worlds) * u64::from(plan.work),
            dispatches: 1,
            uploaded_bytes: PARAM_BYTES
                + plan.seeds
                + if fresh.is_some() { graph.bytes } else { 0 },
            downloaded_bytes: plan.results,
        };
        Ok(Self { fresh, plan, stats })
    }
}

#[cfg(test)]
mod tests;
