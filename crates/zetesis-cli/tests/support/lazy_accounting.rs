//! Accounting and rendering fixtures are not physical device evidence.

use crate::{Backend, LazyExecutionStatistics};

pub(crate) fn fixture() -> LazyExecutionStatistics {
    LazyExecutionStatistics {
        requested_backend: Backend::Metal,
        adapter: "FORMAT FIXTURE: no physical execution".into(),
        backend: "Metal".into(),
        batches: 2,
        submitted_candidates: 7,
        completed_candidates: 4,
        stopped_candidates: 3,
        queued_results: 2,
        source_rounds: 3,
        source_work: 41,
        source_instances: 13,
        peak_catalog_atoms: 65,
        dispatches: 5,
        world_instances: 19,
        uploaded_bytes: 67,
        downloaded_bytes: 23,
        host_wait: std::time::Duration::from_nanos(123),
    }
}

#[cfg(feature = "gpu")]
#[test]
fn completed_batches_count_duplicate_candidate_occurrences() {
    let mut stats = fixture();
    stats
        .record(
            3,
            true,
            zetesis_cpu::lazy::Progress::default(),
            zetesis_wgpu::LazyGpuStatistics::default(),
        )
        .unwrap();
    assert_eq!(stats.completed_candidates, 7);
    assert_eq!(stats.submitted_candidates, 10);
    assert_eq!(stats.stopped_candidates, 3);
}

#[cfg(feature = "gpu")]
#[test]
fn failed_batches_retain_submitted_work() {
    let mut stats = fixture();
    stats
        .record(
            3,
            false,
            zetesis_cpu::lazy::Progress {
                rounds: 1,
                source_work: 2,
                instances: 3,
                chunks: 1,
                catalog_atoms: 66,
            },
            zetesis_wgpu::LazyGpuStatistics {
                dispatches: 2,
                world_instances: 9,
                uploaded_bytes: 11,
                downloaded_bytes: 7,
                host_wait: std::time::Duration::from_nanos(9),
            },
        )
        .unwrap();
    assert_eq!(stats.completed_candidates, 4);
    assert_eq!(stats.stopped_candidates, 6);
    assert_eq!(
        (
            stats.source_rounds,
            stats.source_work,
            stats.source_instances
        ),
        (4, 43, 16)
    );
    assert_eq!(stats.peak_catalog_atoms, 66);
    assert_eq!((stats.dispatches, stats.world_instances), (7, 28));
    assert_eq!((stats.uploaded_bytes, stats.downloaded_bytes), (78, 30));
    assert_eq!(stats.host_wait.as_nanos(), 132);
}

#[cfg(feature = "gpu")]
#[test]
fn counter_overflow_preserves_the_previous_record() {
    let mut stats = fixture();
    stats.downloaded_bytes = u64::MAX;
    let before = stats.clone();
    let error = stats
        .record(
            3,
            true,
            zetesis_cpu::lazy::Progress::default(),
            zetesis_wgpu::LazyGpuStatistics {
                downloaded_bytes: 1,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, crate::RunError::LazyStatisticsOverflow));
    assert_eq!(stats, before);
}

#[cfg(feature = "gpu")]
#[test]
fn duration_overflow_preserves_the_previous_record() {
    let mut stats = fixture();
    stats.host_wait = std::time::Duration::MAX;
    let before = stats.clone();
    let error = stats
        .record(
            3,
            true,
            zetesis_cpu::lazy::Progress::default(),
            zetesis_wgpu::LazyGpuStatistics {
                host_wait: std::time::Duration::from_nanos(1),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, crate::RunError::LazyStatisticsOverflow));
    assert_eq!(stats, before);
}
