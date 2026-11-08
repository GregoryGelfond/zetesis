use super::*;
use crate::{RegionCounts, RegionSearchStatistics};
use zetesis_ferraris::NarrowingStatistics;

#[test]
fn completed_batches_include_unchanged_and_absorbing_results() {
    for result in [
        SubjectStep::Unchanged,
        SubjectStep::Changed,
        SubjectStep::Refuted,
    ] {
        let attempt = ArithmeticAttempt::<Incomplete> {
            result: Ok(result),
            statistics: NarrowingStatistics::default(),
        };
        let parity = ArithmeticKind::Parity.counts(&attempt);
        let cardinality = ArithmeticKind::Cardinality.counts(&attempt);
        assert_eq!(parity.parity_passes, 1);
        assert_eq!(cardinality.cardinality_passes, 1);
        assert_eq!(
            parity.parity_refuted,
            u64::from(result == SubjectStep::Refuted)
        );
        assert_eq!(
            cardinality.cardinality_refuted,
            u64::from(result == SubjectStep::Refuted)
        );
        assert_eq!(parity.cardinality_passes, 0);
        assert_eq!(cardinality.parity_passes, 0);
        assert_eq!(parity.parity_forced, 0);
    }
}

#[test]
fn failed_batches_retain_decisions_without_claiming_completion() {
    let mut statistics = NarrowingStatistics::default();
    statistics.held = 2;
    statistics.cut = 3;
    statistics.work = 41;
    let attempt = ArithmeticAttempt {
        result: Err(Incomplete::WorkLimit),
        statistics,
    };
    assert_eq!(
        ArithmeticKind::Parity.counts(&attempt),
        ArithmeticCounts {
            parity_forced: 5,
            ..ArithmeticCounts::default()
        }
    );
    assert_eq!(
        ArithmeticKind::Cardinality.counts(&attempt),
        ArithmeticCounts::default()
    );
}

#[test]
fn activity_overflow_leaves_the_whole_regional_receipt_unchanged() {
    for field in 0..5 {
        let mut counts = RegionCounts {
            work: 17,
            ..RegionCounts::default()
        };
        let counter = match field {
            0 => &mut counts.arithmetic.parity_passes,
            1 => &mut counts.arithmetic.parity_forced,
            2 => &mut counts.arithmetic.parity_refuted,
            3 => &mut counts.arithmetic.cardinality_passes,
            _ => &mut counts.arithmetic.cardinality_refuted,
        };
        *counter = u64::MAX;
        let before = counts;
        let added = RegionCounts {
            work: 19,
            arithmetic: ArithmeticCounts {
                parity_passes: 1,
                parity_forced: 1,
                parity_refuted: 1,
                cardinality_passes: 1,
                cardinality_refuted: 1,
            },
            ..RegionCounts::default()
        };
        assert_eq!(counts.add(added), Err(Incomplete::CounterOverflow));
        assert_eq!(counts, before);
    }
}

#[test]
fn projection_reads_totals_without_adding_them_again() {
    let mut receipt = RegionSearchStatistics::default();
    receipt.parity.region_passes = 999;
    receipt.parity.root_refuted = true;
    receipt.parity.units = 2;
    receipt.counts.arithmetic = ArithmeticCounts {
        parity_passes: 3,
        parity_forced: 5,
        parity_refuted: 1,
        cardinality_passes: 7,
        cardinality_refuted: 2,
    };
    let projected = receipt.with_runtime_arithmetic();
    assert_eq!(projected.parity.region_passes, 3);
    assert_eq!(projected.parity.forced, 5);
    assert_eq!(projected.parity.refuted, 1);
    assert_eq!(projected.cardinality.region_passes, 7);
    assert_eq!(projected.cardinality.refuted, 2);
    assert!(projected.parity.root_refuted);
    assert_eq!(projected.parity.units, 2);
    assert_eq!(projected.with_runtime_arithmetic(), projected);
    assert_eq!(receipt.parity.region_passes, 999);
}

#[test]
fn common_narrowing_conversion_does_not_attribute_arithmetic() {
    use crate::subject::SubjectAttempt;
    use zetesis_algebra::region::Narrowing;
    use zetesis_ferraris::NarrowingAttempt;

    let mut statistics = NarrowingStatistics::default();
    statistics.held = 2;
    statistics.cut = 1;
    statistics.work = 17;
    let attempt = NarrowingAttempt {
        result: Ok(Narrowing::Fixed { changed: true }),
        statistics,
        preferred: None,
    };
    // Candidate Original/Restriction/Bound and frozen membership use this
    // common conversion. The two explicit arithmetic arms alone attribute it.
    let receipt = SubjectAttempt::from_narrowing(&attempt);
    assert_eq!(receipt.counts.arithmetic, ArithmeticCounts::default());
    assert_eq!(
        (receipt.counts.held, receipt.counts.cut, receipt.counts.work),
        (2, 1, 17)
    );
}

#[test]
fn scalar_snapshot_projects_its_existing_cumulative_receipt() {
    use crate::budget::{Budget, LocalQuota};
    use crate::{Cancellation, SearchLimits, SearchStatistics};
    use zetesis_ferraris::{AdmissionLimits, FormulaParts, LearningLimits, Theory};

    let theory = Theory::new(
        0,
        FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut search = super::super::RegionSearch::new(
        super::super::open(&theory, u64::MAX, LearningLimits::DEFAULT, &mut budget).unwrap(),
        u64::MAX,
    )
    .unwrap();
    let activity = ArithmeticCounts {
        parity_passes: 2,
        parity_forced: 1,
        cardinality_passes: 3,
        ..ArithmeticCounts::default()
    };
    search
        .counts_mut()
        .add(RegionCounts {
            arithmetic: activity,
            ..RegionCounts::default()
        })
        .unwrap();
    let snapshot = search.statistics();
    assert_eq!(snapshot.counts.arithmetic, activity);
    assert_eq!(snapshot.parity.region_passes, 2);
    assert_eq!(snapshot.parity.forced, 1);
    assert_eq!(snapshot.cardinality.region_passes, 3);
    assert_eq!(search.statistics(), snapshot);
    assert_eq!(search.counts_mut().arithmetic, activity);
}
