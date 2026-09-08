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
                ..zetesis_cpu::lazy::Progress::default()
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

#[cfg(feature = "gpu")]
mod classification {
    use std::error::Error;
    use zetesis_core::{
        AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Seed, Template,
    };
    use zetesis_cpu::{Control, Stop, lazy};
    use zetesis_wgpu::GpuError;

    fn atom(name: &str) -> Atom {
        Atom::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
    }

    fn program() -> Program {
        let pattern = |name| AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap();
        Program::new(
            vec![
                Template::new(
                    Some(pattern("a")),
                    vec![],
                    vec![],
                    vec![pattern("b")],
                    vec![],
                ),
                Template::new(
                    Some(pattern("b")),
                    vec![],
                    vec![],
                    vec![pattern("a")],
                    vec![],
                ),
            ],
            AdmissionLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn adapter_observation_does_not_claim_execution() {
        let stats = crate::LazyExecutionStatistics::new(
            crate::Backend::Nvidia,
            zetesis_wgpu::AdapterMetadata {
                name: "METADATA FIXTURE: no physical execution",
                backend: zetesis_wgpu::AdapterBackend::Vulkan,
                category: zetesis_wgpu::AdapterCategory::DiscreteGpu,
                vendor_id: 0,
                device_id: 0,
                pci_bus_id: None,
                driver: None,
                driver_info: None,
            },
        );
        assert_eq!(stats.requested_backend, crate::Backend::Nvidia);
        assert_eq!(stats.backend, "Vulkan");
        assert_eq!(stats.adapter, "METADATA FIXTURE: no physical execution");
        assert_eq!(stats.batches, 0);
        assert_eq!(stats.submitted_candidates, 0);
        assert_eq!(stats.dispatches, 0);
        assert_eq!(stats.uploaded_bytes, 0);
        assert_eq!(stats.downloaded_bytes, 0);
    }

    #[test]
    fn batch_results_preserve_candidate_occurrence_order() {
        let program = program();
        let seeds = [
            vec![],
            vec![atom("a")],
            vec![atom("b")],
            vec![atom("a"), atom("b")],
            vec![atom("a")],
        ]
        .map(|atoms| Seed::new(&program, atoms).unwrap());
        let batch = lazy::check_with(
            &program,
            &seeds,
            lazy::Limits::default(),
            &Control::default(),
            |chunk| Ok::<_, GpuError>(lazy::evaluate(chunk).unwrap()),
        );
        let actual = crate::lazy_execution::batch_results(batch).unwrap();
        assert_eq!(
            actual,
            vec![
                Ok(None),
                Ok(Some(Model::new([atom("a")]))),
                Ok(Some(Model::new([atom("b")]))),
                Ok(None),
                Ok(Some(Model::new([atom("a")])))
            ]
        );
    }

    #[test]
    fn source_failure_becomes_an_incomplete_session_result() {
        let program = program();
        let seeds = [Seed::new(&program, []).unwrap()];
        let batch = lazy::check_with(
            &program,
            &seeds,
            lazy::Limits {
                max_source_work: 0,
                ..Default::default()
            },
            &Control::default(),
            |chunk| Ok::<_, GpuError>(lazy::evaluate(chunk).unwrap()),
        );
        assert_eq!(
            crate::lazy_execution::batch_results(batch).unwrap(),
            vec![Err(Stop::WorkLimit)]
        );
    }

    #[test]
    fn protocol_failure_remains_an_external_session_error() {
        let program = program();
        let seeds = [Seed::new(&program, []).unwrap()];
        let batch = lazy::check_with(
            &program,
            &seeds,
            lazy::Limits::default(),
            &Control::default(),
            |_| Ok::<_, GpuError>(vec![]),
        );
        let error = crate::lazy_execution::batch_results(batch).unwrap_err();
        assert!(matches!(error, crate::RunError::LazyGpu(_)));
        assert!(error.to_string().contains("encoded contract"));
        let failure = error
            .source()
            .unwrap()
            .downcast_ref::<lazy::Failure<GpuError>>()
            .unwrap();
        assert_eq!(failure.progress.rounds, 0);
        assert!(failure.progress.source_work > 0);
        assert!(matches!(failure.cause, lazy::Cause::InvalidOutput));
    }
}
