//! Ordinary shared CPU accounting, separate from device execution.

#[cfg(test)]
#[path = "../tests/support/shared_accounting.rs"]
mod tests;

use zetesis_cpu::lazy::shared::{Cause, Statistics};

/// Cumulative ordinary relational checks through shared source rounds on Rayon.
/// Source and world operations have different units and are reported separately.
/// These counters describe attempted work, not process memory or GPU activity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedExecutionStatistics {
    /// Requested backend; auto is resolved to CPU for an explicit shared policy.
    pub requested_backend: crate::Backend,
    /// Selected shared source policy.
    pub selection: zetesis_cpu::lazy::SourceSelection,
    /// Owned Rayon worker count.
    pub workers: usize,
    /// Nonempty attempted batches.
    pub batches: u64,
    /// Submitted occurrences, including duplicate seeds.
    pub submitted_candidates: u64,
    /// Occurrences with complete returned reduct checks.
    pub completed_candidates: u64,
    /// Submitted occurrences with no complete check after a failed batch.
    pub stopped_candidates: u64,
    /// Complete checks still awaiting session consumption.
    pub queued_results: usize,
    /// Completed immutable source/evaluation rounds, including failed-batch prefixes.
    pub source_rounds: u64,
    /// Collective charged source operations.
    pub source_work: u64,
    /// Source bindings offered to the bounded instance consumer.
    pub source_instances: u64,
    /// Maximum demanded atoms in a batch catalog.
    pub peak_catalog_atoms: usize,
    /// Source membership-mask words inspected.
    pub mask_words: u64,
    /// Source binding prefixes pruned for lack of a common world.
    pub pruned_prefixes: u64,
    /// Maximum source membership-mask payload, excluding allocator overhead.
    pub peak_mask_bytes: usize,
    /// Sum of per-world record visits and antecedent tests.
    pub world_work: u64,
    /// Per-world record visits; included in `world_work`.
    pub world_instances: u64,
    /// Exact most recent incomplete batch cause, including its input index.
    pub last_stop: Option<Cause>,
}

impl SharedExecutionStatistics {
    pub(crate) fn new(
        config: &crate::SolveConfig,
        selection: zetesis_cpu::lazy::SourceSelection,
    ) -> Self {
        Self {
            requested_backend: config.backend,
            selection,
            workers: config.workers.get(),
            batches: 0,
            submitted_candidates: 0,
            completed_candidates: 0,
            stopped_candidates: 0,
            queued_results: 0,
            source_rounds: 0,
            source_work: 0,
            source_instances: 0,
            peak_catalog_atoms: 0,
            mask_words: 0,
            pruned_prefixes: 0,
            peak_mask_bytes: 0,
            world_work: 0,
            world_instances: 0,
            last_stop: None,
        }
    }

    pub(crate) fn record(
        &mut self,
        statistics: &Statistics,
        stop: Option<Cause>,
    ) -> Result<(), crate::SolveError> {
        let add = |left: u64, right: u64| {
            left.checked_add(right)
                .ok_or(crate::SolveError::LazyStatisticsOverflow)
        };
        let candidates = u64::try_from(statistics.submitted_candidates)
            .map_err(|_| crate::SolveError::LazyStatisticsOverflow)?;
        let source = statistics.source;
        let (work, instances) =
            statistics
                .worlds
                .iter()
                .try_fold((0, 0), |(work, instances), world| {
                    Ok::<_, crate::SolveError>((
                        add(work, world.work)?,
                        add(instances, world.instances)?,
                    ))
                })?;
        // Replace only after every cumulative field has been checked.
        let next = Self {
            batches: add(self.batches, 1)?,
            submitted_candidates: add(self.submitted_candidates, candidates)?,
            completed_candidates: add(
                self.completed_candidates,
                if stop.is_none() { candidates } else { 0 },
            )?,
            stopped_candidates: add(
                self.stopped_candidates,
                if stop.is_some() { candidates } else { 0 },
            )?,
            source_rounds: add(self.source_rounds, source.rounds)?,
            source_work: add(self.source_work, source.source_work)?,
            source_instances: add(self.source_instances, source.instances)?,
            peak_catalog_atoms: self.peak_catalog_atoms.max(source.catalog_atoms),
            mask_words: add(self.mask_words, source.mask_words)?,
            pruned_prefixes: add(self.pruned_prefixes, source.pruned_prefixes)?,
            peak_mask_bytes: self.peak_mask_bytes.max(source.peak_mask_bytes),
            world_work: add(self.world_work, work)?,
            world_instances: add(self.world_instances, instances)?,
            last_stop: stop.or(self.last_stop),
            ..self.clone()
        };
        *self = next;
        Ok(())
    }
}

pub(crate) fn batch_results(
    result: Result<zetesis_cpu::lazy::shared::Batch, zetesis_cpu::lazy::shared::Error>,
    statistics: &mut SharedExecutionStatistics,
) -> Result<Vec<Result<Option<zetesis_core::Model>, zetesis_cpu::Stop>>, crate::SolveError> {
    match result {
        Ok(batch) => {
            statistics.record(&batch.statistics, None)?;
            Ok(batch
                .checks
                .into_iter()
                .map(|check| Ok(check.accepted().then(|| check.into_closure())))
                .collect())
        }
        Err(zetesis_cpu::lazy::shared::Error::Admission(error)) => {
            Err(crate::SolveError::Batch(error))
        }
        Err(zetesis_cpu::lazy::shared::Error::Incomplete(failure)) => {
            statistics.record(&failure.statistics, Some(failure.cause))?;
            match failure.cause {
                Cause::Source(stop) | Cause::World { stop, .. } => Ok(vec![Err(stop)]),
                Cause::InvalidOutput => Err(crate::SolveError::SharedCpu(failure.cause)),
            }
        }
    }
}
