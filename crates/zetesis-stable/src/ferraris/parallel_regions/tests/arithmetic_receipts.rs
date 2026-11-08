use super::*;
use crate::ferraris::regions::arithmetic_counts::ArithmeticCounts;

#[test]
fn worker_activity_is_published_once_through_existing_counts() {
    let cancellation = Cancellation::default();
    let theory = Theory::new(
        0,
        zetesis_ferraris::FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits::default();
    let mut budget = Budget {
        quota: crate::budget::LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut search = ParallelRegions::new(
        crate::ferraris::regions::open(
            &theory,
            limits.max_reduct_bytes,
            limits.learning,
            &mut budget,
        )
        .unwrap(),
        NonZeroUsize::new(2).unwrap(),
        Engagement::Immediate,
        limits,
        cancellation.clone(),
        &mut budget,
    )
    .unwrap();
    let coordinator = ArithmeticCounts {
        parity_passes: 2,
        ..ArithmeticCounts::default()
    };
    search.counts_mut().arithmetic = coordinator;
    let mut cumulative = RegionCounts::default();
    // Two worker publications: first an applied prefix ending in failure,
    // then a completed refutation and a separate cardinality completion.
    for delta in [
        ArithmeticCounts {
            parity_forced: 1,
            ..ArithmeticCounts::default()
        },
        ArithmeticCounts {
            parity_passes: 1,
            parity_refuted: 1,
            cardinality_passes: 1,
            ..ArithmeticCounts::default()
        },
    ] {
        let before = cumulative;
        cumulative
            .add(RegionCounts {
                arithmetic: delta,
                ..RegionCounts::default()
            })
            .unwrap();
        publish_counts(&search.shared, &before, &cumulative);
    }
    // A no-change publication, as used by inactive readers, adds nothing.
    publish_counts(&search.shared, &cumulative, &cumulative);
    let expected = coordinator.saturating_add(cumulative.arithmetic);
    let first = search.statistics();
    let second = search.statistics();
    assert_eq!(first.counts.arithmetic, expected);
    assert_eq!(second.counts.arithmetic, expected);
    assert_eq!(first.parity.region_passes, 3);
    assert_eq!(first.parity.forced, 1);
    assert_eq!(first.parity.refuted, 1);
    assert_eq!(first.cardinality.region_passes, 1);
    assert_eq!(first.cardinality.refuted, 0);
    assert_eq!(search.counts_mut().arithmetic, coordinator);
}
