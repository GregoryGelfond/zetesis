//! Bounded publication of committed GPU sampling statistics.
use super::*;

#[test]
fn committed_batch_sample_ranks_round_trip_without_losing_wide_values() {
    use zetesis_solve::{
        ResidualHandoverStatistics, ResidualQuantiles, ResidualRunSamples, ResidualSampleGroup,
    };
    let mut samples = ResidualRunSamples::default();
    samples.seed = 23;
    samples.batches.seen = Some(10);
    samples.batches.capacity = 2;
    samples.batches.retained = 2;
    samples.worlds.seen = Some(0);
    let mut all = ResidualSampleGroup::default();
    let value = u128::from(u64::MAX) + 17;
    let times = &mut all.batch_ns;
    times.eligible = Some(10);
    times.known = Some(10);
    times.retained = 2;
    times.sum = Some(value * 10);
    times.maximum = Some(value);
    times.median = Some(value);
    times.p90 = Some(value);
    times.quantiles = ResidualQuantiles::Estimated;
    samples.groups = Some(vec![
        all,
        ResidualSampleGroup::default(),
        ResidualSampleGroup::default(),
        ResidualSampleGroup::default(),
    ]);
    let mut stats = ResidualHandoverStatistics::default();
    stats.samples = Some(samples);
    let mut out = Buffer::new(16384);
    super::handovers::write(&mut out, Some(&stats)).unwrap();
    let receipt: zetesis_validation::performance::matrix::ResidualHandoverStatistics =
        serde_json::from_slice(&out.bytes).unwrap();
    let samples = receipt.samples.unwrap();
    let metric = samples.groups.unwrap().all.batch_ns;
    assert_eq!((samples.batches.seen, samples.seed), (Some(10), 23));
    assert_eq!(
        (metric.sum, metric.median, metric.p90),
        (Some(value * 10), Some(value), Some(value))
    );
    assert_eq!(
        metric.quantiles,
        zetesis_validation::performance::matrix::ResidualQuantiles::Estimated
    );
}
