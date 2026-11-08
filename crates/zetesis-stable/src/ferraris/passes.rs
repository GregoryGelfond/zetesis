//! The receipts of a search an optimizer runs in passes over one theory: a
//! later pass's stream reports its own work after the earlier passes'.

use zetesis_ferraris::SupportExplanationStatistics;

use super::regions::{RegionCounts, RegionFrontierStatistics, RegionSearchStatistics};
use super::{BatchStatistics, CertifiedStatistics, Statistics};

/// The final receipts of the passes before this stream's.
#[derive(Clone, Copy, Debug)]
pub(super) struct Earlier {
    pub(super) statistics: Statistics,
    pub(super) batch: BatchStatistics,
}

/// `earlier` followed by `later`: work, counts and installations summed, peaks
/// the larger, and what describes the route as it stands (the frontier's
/// order and current size, its producers and workers, the certificate plan,
/// the generator order's status and retained bytes) the later pass's. Sums
/// saturate at the counters' maximum, which no search reaches.
pub(super) fn statistics(earlier: &Statistics, later: &Statistics) -> Statistics {
    Statistics {
        search: crate::SearchStatistics {
            work: earlier.search.work.saturating_add(later.search.work),
            decisions: earlier
                .search
                .decisions
                .saturating_add(later.search.decisions),
        },
        candidate_restrictions: earlier
            .candidate_restrictions
            .saturating_add(later.candidate_restrictions),
        candidates: earlier.candidates.saturating_add(later.candidates),
        countermodel_queries: earlier
            .countermodel_queries
            .saturating_add(later.countermodel_queries),
        countermodels: earlier.countermodels.saturating_add(later.countermodels),
        stable_models: earlier.stable_models.saturating_add(later.stable_models),
        phase_timings: crate::timing::combined(earlier.phase_timings, later.phase_timings),
        certified: match (&earlier.certified, &later.certified) {
            (Some(earlier), Some(later)) => Some(certified(earlier, later)),
            (earlier, later) => later.or(*earlier),
        },
        regions: match (earlier.regions, later.regions) {
            (Some(earlier), Some(later)) => Some(regions(&earlier, later)),
            (earlier, later) => later.or(earlier),
        },
        region_filter: match (earlier.region_filter, later.region_filter) {
            (Some(earlier), Some(later)) => Some(crate::RegionFilterStatistics {
                preparations: earlier.preparations.saturating_add(later.preparations),
                checks: earlier.checks.saturating_add(later.checks),
                refuted: earlier.refuted.saturating_add(later.refuted),
                failed: earlier.failed.saturating_add(later.failed),
                overflowed: earlier.overflowed || later.overflowed,
            }),
            (earlier, later) => later.or(earlier),
        },
        reduct: crate::ReductStatistics {
            original_work: earlier
                .reduct
                .original_work
                .saturating_add(later.reduct.original_work),
            frozen_positive_attempts: earlier
                .reduct
                .frozen_positive_attempts
                .saturating_add(later.reduct.frozen_positive_attempts),
            frozen_positive_completed: earlier
                .reduct
                .frozen_positive_completed
                .saturating_add(later.reduct.frozen_positive_completed),
            frozen_positive_preparation_work: earlier
                .reduct
                .frozen_positive_preparation_work
                .saturating_add(later.reduct.frozen_positive_preparation_work),
            frozen_positive_work: earlier
                .reduct
                .frozen_positive_work
                .saturating_add(later.reduct.frozen_positive_work),
            peak_workspace_bytes: earlier
                .reduct
                .peak_workspace_bytes
                .max(later.reduct.peak_workspace_bytes),
            regions: counts(&earlier.reduct.regions, later.reduct.regions),
            inner_preparation: earlier
                .reduct
                .inner_preparation
                .saturating_add(later.reduct.inner_preparation),
        },
    }
}

/// Batch receipts in the same sense: the pending candidates and the current
/// validation workspace are the later pass's.
pub(super) fn batch(earlier: &BatchStatistics, later: BatchStatistics) -> BatchStatistics {
    BatchStatistics {
        original_validation_work: earlier
            .original_validation_work
            .saturating_add(later.original_validation_work),
        checker_calls: earlier.checker_calls.saturating_add(later.checker_calls),
        completion_calls: earlier
            .completion_calls
            .saturating_add(later.completion_calls),
        committed: earlier.committed.saturating_add(later.committed),
        propagated: earlier.propagated.saturating_add(later.propagated),
        residuals: earlier.residuals.saturating_add(later.residuals),
        lookahead_slates: earlier
            .lookahead_slates
            .saturating_add(later.lookahead_slates),
        ..later
    }
}

fn certified(earlier: &CertifiedStatistics, later: &CertifiedStatistics) -> CertifiedStatistics {
    CertifiedStatistics {
        construction_work: earlier
            .construction_work
            .saturating_add(later.construction_work),
        checks: earlier.checks.saturating_add(later.checks),
        stable: earlier.stable.saturating_add(later.stable),
        refuted: earlier.refuted.saturating_add(later.refuted),
        failed: earlier.failed.saturating_add(later.failed),
        checking_work: earlier.checking_work.saturating_add(later.checking_work),
        tight_check_peak_bytes: earlier
            .tight_check_peak_bytes
            .max(later.tight_check_peak_bytes),
        positive_check_peak_bytes: earlier
            .positive_check_peak_bytes
            .max(later.positive_check_peak_bytes),
        stratified_check_peak_bytes: earlier
            .stratified_check_peak_bytes
            .max(later.stratified_check_peak_bytes),
        ..*later
    }
}

fn regions(
    earlier: &RegionSearchStatistics,
    mut later: RegionSearchStatistics,
) -> RegionSearchStatistics {
    later.counts = counts(&earlier.counts, later.counts);
    later.generator_order.work = earlier
        .generator_order
        .work
        .saturating_add(later.generator_order.work);
    later.generator_order.peak_bytes = earlier
        .generator_order
        .peak_bytes
        .max(later.generator_order.peak_bytes);
    later.parity.preparation_work = earlier
        .parity
        .preparation_work
        .saturating_add(later.parity.preparation_work);
    later.parity.preparation_bytes = earlier
        .parity
        .preparation_bytes
        .max(later.parity.preparation_bytes);
    later.parity.phases = preparation_work(earlier.parity.phases, later.parity.phases);
    later.parity.region_passes = earlier
        .parity
        .region_passes
        .saturating_add(later.parity.region_passes);
    later.parity.forced = earlier.parity.forced.saturating_add(later.parity.forced);
    later.parity.refuted = earlier.parity.refuted.saturating_add(later.parity.refuted);
    later.cardinality.preparation_work = earlier
        .cardinality
        .preparation_work
        .saturating_add(later.cardinality.preparation_work);
    later.cardinality.preparation_bytes = earlier
        .cardinality
        .preparation_bytes
        .max(later.cardinality.preparation_bytes);
    later.cardinality.phases =
        preparation_work(earlier.cardinality.phases, later.cardinality.phases);
    later.cardinality.region_passes = earlier
        .cardinality
        .region_passes
        .saturating_add(later.cardinality.region_passes);
    later.cardinality.refuted = earlier
        .cardinality
        .refuted
        .saturating_add(later.cardinality.refuted);
    if let (Some(earlier), Some(later)) = (earlier.frontier, later.frontier.as_mut()) {
        *later = RegionFrontierStatistics {
            peak_regions: earlier.peak_regions.max(later.peak_regions),
            peak_capacity: earlier.peak_capacity.max(later.peak_capacity),
            peak_retained_bytes: earlier.peak_retained_bytes.max(later.peak_retained_bytes),
            ..*later
        };
    }
    later
}

fn preparation_work(
    earlier: zetesis_ferraris::PreparationWork,
    later: zetesis_ferraris::PreparationWork,
) -> zetesis_ferraris::PreparationWork {
    zetesis_ferraris::PreparationWork {
        recognition: earlier.recognition.saturating_add(later.recognition),
        provenance: earlier.provenance.saturating_add(later.provenance),
        reduction: earlier.reduction.saturating_add(later.reduction),
        discovery: earlier.discovery.saturating_add(later.discovery),
        candidate_checks: earlier
            .candidate_checks
            .saturating_add(later.candidate_checks),
        index: earlier.index.saturating_add(later.index),
        root_seed: earlier.root_seed.saturating_add(later.root_seed),
    }
}

fn counts(earlier: &RegionCounts, later: RegionCounts) -> RegionCounts {
    let (e, l) = (&earlier.support_explanations, later.support_explanations);
    RegionCounts {
        regions: earlier.regions.saturating_add(later.regions),
        refuted: earlier.refuted.saturating_add(later.refuted),
        leaves: earlier.leaves.saturating_add(later.leaves),
        propagations: earlier.propagations.saturating_add(later.propagations),
        held: earlier.held.saturating_add(later.held),
        cut: earlier.cut.saturating_add(later.cut),
        work: earlier.work.saturating_add(later.work),
        support_work: earlier.support_work.saturating_add(later.support_work),
        founded_work: earlier.founded_work.saturating_add(later.founded_work),
        arithmetic: earlier.arithmetic.saturating_add(later.arithmetic),
        support_explanations: SupportExplanationStatistics {
            attempts: e.attempts.saturating_add(l.attempts),
            certified: e.certified.saturating_add(l.certified),
            matched: e.matched.saturating_add(l.matched),
            cut: e.cut.saturating_add(l.cut),
            forced: e.forced.saturating_add(l.forced),
            refuted: e.refuted.saturating_add(l.refuted),
            declined: e.declined.saturating_add(l.declined),
            work: e.work.saturating_add(l.work),
            peak_bytes: e.peak_bytes.max(l.peak_bytes),
        },
    }
}

#[cfg(test)]
mod tests;
