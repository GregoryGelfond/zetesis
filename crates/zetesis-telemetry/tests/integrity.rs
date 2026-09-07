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
fn driver_duration_is_rechecked() {
    let mut snapshot = StageRecorder::new(true).snapshot().unwrap();
    assert!(snapshot.is_complete());
    let recorded = snapshot.driver_elapsed;
    snapshot.driver_elapsed += Duration::from_nanos(1);
    assert!(!snapshot.is_complete());
    snapshot.driver_elapsed = recorded;
    assert!(snapshot.is_complete());
}

#[test]
fn unattributed_duration_is_rechecked() {
    let mut snapshot = StageRecorder::new(true).snapshot().unwrap();
    let recorded = snapshot.unattributed.unwrap();
    snapshot.unattributed = Some(recorded + Duration::from_nanos(1));
    assert!(!snapshot.is_complete());
    snapshot.unattributed = Some(recorded);
    assert!(snapshot.is_complete());
}
