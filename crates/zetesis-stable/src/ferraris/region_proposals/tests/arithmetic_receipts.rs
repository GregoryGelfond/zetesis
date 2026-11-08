use super::*;
use crate::RegionCounts;
use crate::ferraris::regions::arithmetic_counts::ArithmeticCounts;

#[test]
fn slate_snapshots_project_the_joined_receipt_without_consuming_it() {
    let mut proposals = proposals(&tautologies(1), 2);
    let activity = ArithmeticCounts {
        parity_passes: 3,
        parity_forced: 2,
        parity_refuted: 1,
        cardinality_passes: 4,
        cardinality_refuted: 1,
    };
    proposals
        .counts_mut()
        .add(RegionCounts {
            arithmetic: activity,
            ..RegionCounts::default()
        })
        .unwrap();
    for _ in 0..2 {
        let snapshot = proposals.statistics();
        assert_eq!(snapshot.counts.arithmetic, activity);
        assert_eq!(snapshot.parity.region_passes, 3);
        assert_eq!(snapshot.parity.forced, 2);
        assert_eq!(snapshot.parity.refuted, 1);
        assert_eq!(snapshot.cardinality.region_passes, 4);
        assert_eq!(snapshot.cardinality.refuted, 1);
    }
    assert_eq!(proposals.counts_mut().arithmetic, activity);
}
