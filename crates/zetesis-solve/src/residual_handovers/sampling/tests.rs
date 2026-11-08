use super::super::decoded::DecodeJournal;
use super::ResidualQuantiles;
use super::collector::RunSamples;
use super::reservoir::{BATCH_TAG, Reservoir, WORLD_TAG};
use super::values::{Totals, Value};
use crate::FormulaResidualStatistics;
use zetesis_wgpu::ResidualReason;

fn finish(sampler: &mut RunSamples) -> super::ResidualRunSamples {
    sampler.settle();
    sampler.snapshot()
}

fn capture(sizes: &[usize]) -> super::ResidualRunSamples {
    let mut sampler = RunSamples::new(256 * 1024 * 1024).unwrap();
    for &n in sizes {
        let mut journal = DecodeJournal::default();
        for i in 0..n {
            journal.record(i, n, ResidualReason::FixedPoint, i as u64, u64::MAX);
        }
        sampler.commit(
            Some(FormulaResidualStatistics {
                fixed_point: n as u64,
                ..Default::default()
            }),
            &journal,
            None,
            true,
        );
    }
    finish(&mut sampler)
}

#[test]
fn empty_batches_have_their_own_population_weight() {
    let summary = capture(&[0, 1, 9]);
    assert_eq!(summary.worlds.seen, Some(10));
    assert_eq!(summary.batches.seen, Some(3));
    let group = &summary.groups.as_ref().unwrap()[0];
    assert_eq!(group.residuals.median, Some(1));
    assert_eq!(group.residuals.quantiles, ResidualQuantiles::Exact);
    assert_eq!(group.residuals.p90, Some(9));
    assert_eq!(group.open_atoms.sum, Some(36));
}

#[test]
fn two_batch_median_uses_the_integer_midpoint() {
    let summary = capture(&[0, 10]);
    assert_eq!(summary.groups.unwrap()[0].residuals.median, Some(5));
}

#[test]
fn missing_query_evidence_preserves_known_batch_counts() {
    let summary = capture(&[0, 2]);
    let groups = summary.groups.unwrap();
    assert_eq!(groups[1].work.eligible, None);
    assert_eq!(groups[1].work.quantiles, ResidualQuantiles::Unavailable);
    assert_eq!(groups[1].residuals.sum, Some(2));
    assert_eq!(groups[2].residuals.median, Some(0));
}

#[test]
fn disabled_clock_is_not_a_measured_zero() {
    let mut sampler = RunSamples::new(256 * 1024 * 1024).unwrap();
    sampler.commit(
        Some(FormulaResidualStatistics::default()),
        &DecodeJournal::default(),
        None,
        false,
    );
    let all = &finish(&mut sampler).groups.unwrap()[0];
    assert_eq!(all.batch_ns.eligible, Some(1));
    assert_eq!(all.batch_ns.known, Some(0));
    assert_eq!(all.batch_ns.median, None);
    assert_eq!(all.width.median, Some(0));
}

#[test]
fn nonexistent_byte_allowance_declines_only_the_collector() {
    assert!(RunSamples::new(0).is_none());
}

#[test]
fn live_snapshots_leave_retained_populations_untouched() {
    let mut sampler = RunSamples::new(256 * 1024 * 1024).unwrap();
    let counts = Some(FormulaResidualStatistics::default());
    sampler.commit(counts, &DecodeJournal::default(), None, true);
    assert_eq!(sampler.snapshot(), sampler.snapshot());
    assert!(sampler.snapshot().groups.is_none());
    let summary = finish(&mut sampler);
    assert_eq!(summary.batches.seen, Some(1));
    assert!(summary.peak_bytes <= summary.byte_limit);
}

#[test]
fn streams_use_separate_replayable_draws() {
    let sample = |tag| {
        let mut rows = Reservoir::new(4, 123, tag);
        for i in 0..100 {
            rows.insert(i, 0, &mut 0);
        }
        (rows.population(), rows.records)
    };
    assert_eq!(sample(WORLD_TAG).1, [79, 56, 10, 51]);
    assert_eq!(sample(BATCH_TAG).1, [23, 62, 7, 82]);
    assert_ne!(sample(WORLD_TAG).1, sample(BATCH_TAG).1);
    assert_eq!(sample(BATCH_TAG).0.seen, Some(100));
}

#[test]
fn truncated_sample_keeps_exact_reductions() {
    let mut total = Totals::default();
    for v in 0..10 {
        total.observe(Value::Known(v));
    }
    let result = total.finish(&mut [1, 9], |v| Some(*v));
    assert_eq!(result.quantiles, ResidualQuantiles::Estimated);
    assert_eq!((result.median, result.p90), (Some(5), Some(9)));
    assert_eq!((result.sum, result.maximum), (Some(45), Some(9)));
}

#[test]
fn overflown_sum_preserves_independent_ranks() {
    let mut total = Totals::default();
    for v in [u128::MAX - 1, u128::MAX] {
        total.observe(Value::Known(v));
    }
    let result = total.finish(&mut [u128::MAX - 1, u128::MAX], |v| Some(*v));
    assert_eq!(result.sum, None);
    assert_eq!(result.median, Some(u128::MAX - 1));
    assert_eq!(result.maximum, Some(u128::MAX));
}

#[test]
fn missing_world_count_does_not_remove_a_batch() {
    let mut journal = DecodeJournal::default();
    journal.record(1, 1, ResidualReason::RoundLimit, 3, 0);
    let mut sampler = RunSamples::new(256 * 1024 * 1024).unwrap();
    sampler.commit(None, &journal, None, true);
    let result = finish(&mut sampler);
    assert_eq!(result.batches.seen, Some(1));
    assert_eq!(result.worlds.seen, None);
}

#[test]
fn refused_snapshot_storage_preserves_the_cached_terminal_ranks() {
    let summary = capture(&[0, 1, 9]);
    let copy = summary.snapshot_with_limit(0);
    assert!(copy.groups.is_none());
    assert_eq!(
        (copy.worlds, copy.batches),
        (summary.worlds, summary.batches)
    );
    assert_eq!(
        summary.groups.as_ref().unwrap()[0].residuals.median,
        Some(1)
    );
    assert_eq!(summary, summary.clone());
}

#[test]
fn repeated_terminal_snapshots_leave_the_first_result_unchanged() {
    let mut sampler = RunSamples::new(256 * 1024 * 1024).unwrap();
    sampler.commit(
        Some(FormulaResidualStatistics::default()),
        &DecodeJournal::default(),
        None,
        true,
    );
    sampler.settle();
    let first = sampler.snapshot();
    assert_eq!(first.batches.seen, Some(1));
    sampler.commit(
        Some(FormulaResidualStatistics::default()),
        &DecodeJournal::default(),
        None,
        true,
    );
    sampler.settle();
    assert_eq!(sampler.snapshot(), first);
}
