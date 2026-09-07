//! Public snapshots must check their current duration partition.

use std::time::Duration;

use zetesis_telemetry::{StageRecorder, StageTimings};

#[test]
fn contradictory_unattributed_duration_is_incomplete() {
    let mut snapshot = StageTimings::default();
    snapshot.unattributed = Some(Duration::from_nanos(1));
    assert!(!snapshot.is_complete());
}

#[test]
fn completeness_rechecks_the_current_driver_duration() {
    let mut snapshot = StageRecorder::new(true).snapshot().unwrap();
    assert!(snapshot.is_complete());
    let recorded = snapshot.driver_elapsed;
    snapshot.driver_elapsed += Duration::from_nanos(1);
    assert!(!snapshot.is_complete());
    snapshot.driver_elapsed = recorded;
    assert!(snapshot.is_complete());
}

#[test]
fn completeness_rechecks_the_current_unattributed_duration() {
    let mut snapshot = StageRecorder::new(true).snapshot().unwrap();
    let recorded = snapshot.unattributed.unwrap();
    snapshot.unattributed = Some(recorded + Duration::from_nanos(1));
    assert!(!snapshot.is_complete());
    snapshot.unattributed = Some(recorded);
    assert!(snapshot.is_complete());
}
