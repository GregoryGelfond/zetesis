//! Accounting and rendering fixtures are not physical device evidence.

use crate::{Backend, LazyExecutionStatistics};

fn fixture() -> LazyExecutionStatistics {
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
        transport_allocations: 2,
        transport_reuses: 3,
        peak_transport_bytes: 1024,
        transport_replacements: crate::LazyTransportReplacements {
            initial: 1,
            records_growth: 1,
            result_shape: 1,
            ..Default::default()
        },
        transport_usage: crate::LazyTransportUsage {
            uniform: crate::LazyBufferUsage {
                allocations: 1,
                reuses: 4,
            },
            offsets: crate::LazyBufferUsage {
                allocations: 1,
                reuses: 4,
            },
            records: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 3,
            },
            snapshots: crate::LazyBufferUsage {
                allocations: 1,
                reuses: 4,
            },
            seeds: crate::LazyBufferUsage {
                allocations: 1,
                reuses: 4,
            },
            output: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 3,
            },
            readback: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 3,
            },
            budget_releases: 0,
            accounting_overflow_releases: 0,
        },
        host_wait: std::time::Duration::from_nanos(123),
    }
}

#[test]
fn completed_batches_count_duplicate_candidate_occurrences() {
    let mut stats = fixture();
    stats
        .record(
            3,
            true,
            zetesis_cpu::lazy::Progress::default(),
            &zetesis_wgpu::LazyGpuStatistics::default(),
        )
        .unwrap();
    assert_eq!(stats.completed_candidates, 7);
    assert_eq!(stats.submitted_candidates, 10);
    assert_eq!(stats.stopped_candidates, 3);
}

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
            &zetesis_wgpu::LazyGpuStatistics {
                dispatches: 2,
                world_instances: 9,
                uploaded_bytes: 11,
                downloaded_bytes: 7,
                transport_allocations: 1,
                transport_reuses: 1,
                peak_transport_bytes: 2048,
                transport_replacements: zetesis_wgpu::LazyTransportReplacements {
                    initial: 1,
                    ..Default::default()
                },
                host_wait: std::time::Duration::from_nanos(9),
                ..Default::default()
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
    assert_eq!(
        (stats.transport_allocations, stats.transport_reuses),
        (3, 4)
    );
    assert_eq!(stats.peak_transport_bytes, 2048);
    assert_eq!(stats.host_wait.as_nanos(), 132);
}

#[test]
fn transport_peak_is_the_largest_attempted_batch() {
    let mut stats = fixture();
    for (peak, expected) in [(512, 1024), (2048, 2048), (1024, 2048)] {
        stats
            .record(
                1,
                true,
                zetesis_cpu::lazy::Progress::default(),
                &zetesis_wgpu::LazyGpuStatistics {
                    peak_transport_bytes: peak,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(stats.peak_transport_bytes, expected);
    }
}

#[test]
fn replacement_overflow_preserves_the_previous_record() {
    type Field = fn(&mut zetesis_wgpu::LazyTransportReplacements) -> &mut u64;
    let fields: [Field; 8] = [
        |value| &mut value.initial,
        |value| &mut value.offsets_growth,
        |value| &mut value.records_growth,
        |value| &mut value.snapshots_growth,
        |value| &mut value.seeds_growth,
        |value| &mut value.result_shape,
        |value| &mut value.budget,
        |value| &mut value.accounting_overflow,
    ];
    for field in fields {
        let mut stats = fixture();
        stats.transport_replacements = crate::LazyTransportReplacements {
            initial: 1,
            offsets_growth: 1,
            records_growth: 1,
            snapshots_growth: 1,
            seeds_growth: 1,
            result_shape: 1,
            budget: 1,
            accounting_overflow: 1,
        };
        let before = stats.clone();
        let mut device = zetesis_wgpu::LazyGpuStatistics::default();
        *field(&mut device.transport_replacements) = u64::MAX;
        let error = stats
            .record(1, true, zetesis_cpu::lazy::Progress::default(), &device)
            .unwrap_err();
        assert!(matches!(error, crate::SolveError::LazyStatisticsOverflow));
        assert_eq!(stats, before);
    }
}

#[test]
fn failed_batches_retain_replacement_reasons() {
    let mut stats = fixture();
    stats
        .record(
            1,
            false,
            zetesis_cpu::lazy::Progress::default(),
            &zetesis_wgpu::LazyGpuStatistics {
                transport_replacements: zetesis_wgpu::LazyTransportReplacements {
                    initial: 1,
                    offsets_growth: 2,
                    records_growth: 3,
                    snapshots_growth: 4,
                    seeds_growth: 5,
                    result_shape: 6,
                    budget: 7,
                    accounting_overflow: 8,
                },
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        stats.transport_replacements,
        crate::LazyTransportReplacements {
            initial: 2,
            offsets_growth: 2,
            records_growth: 4,
            snapshots_growth: 4,
            seeds_growth: 5,
            result_shape: 7,
            budget: 7,
            accounting_overflow: 8,
        }
    );
}

#[test]
fn transport_overflow_preserves_the_previous_record() {
    for (allocations, reuses) in [(u64::MAX, 0), (0, u64::MAX)] {
        let mut stats = fixture();
        let before = stats.clone();
        let error = stats
            .record(
                1,
                true,
                zetesis_cpu::lazy::Progress::default(),
                &zetesis_wgpu::LazyGpuStatistics {
                    transport_allocations: allocations,
                    transport_reuses: reuses,
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(matches!(error, crate::SolveError::LazyStatisticsOverflow));
        assert_eq!(stats, before);
    }
}

#[test]
fn binding_overflow_preserves_the_previous_record() {
    type Field = fn(&mut zetesis_wgpu::LazyTransportUsage) -> &mut zetesis_wgpu::LazyBufferUsage;
    let fields: [Field; 7] = [
        |value| &mut value.uniform,
        |value| &mut value.offsets,
        |value| &mut value.records,
        |value| &mut value.snapshots,
        |value| &mut value.seeds,
        |value| &mut value.output,
        |value| &mut value.readback,
    ];
    for field in fields {
        for (allocations, reuses) in [(u64::MAX, 0), (0, u64::MAX)] {
            let mut stats = fixture();
            let before = stats.clone();
            let mut device = zetesis_wgpu::LazyGpuStatistics::default();
            *field(&mut device.transport_usage) = zetesis_wgpu::LazyBufferUsage {
                allocations,
                reuses,
            };
            let error = stats
                .record(1, true, zetesis_cpu::lazy::Progress::default(), &device)
                .unwrap_err();
            assert!(matches!(error, crate::SolveError::LazyStatisticsOverflow));
            assert_eq!(stats, before);
        }
    }
}

#[test]
fn release_overflow_preserves_the_previous_record() {
    for overflow in [false, true] {
        let mut stats = fixture();
        stats.transport_usage.budget_releases = 1;
        stats.transport_usage.accounting_overflow_releases = 1;
        let before = stats.clone();
        let usage = zetesis_wgpu::LazyTransportUsage {
            budget_releases: if overflow { 0 } else { u64::MAX },
            accounting_overflow_releases: if overflow { u64::MAX } else { 0 },
            ..Default::default()
        };
        let device = zetesis_wgpu::LazyGpuStatistics {
            transport_usage: usage,
            ..Default::default()
        };
        let error = stats
            .record(1, true, zetesis_cpu::lazy::Progress::default(), &device)
            .unwrap_err();
        assert!(matches!(error, crate::SolveError::LazyStatisticsOverflow));
        assert_eq!(stats, before);
    }
}

#[test]
fn failed_batches_retain_binding_observations() {
    let mut stats = fixture();
    let usage = zetesis_wgpu::LazyTransportUsage {
        uniform: zetesis_wgpu::LazyBufferUsage {
            allocations: 1,
            reuses: 2,
        },
        offsets: zetesis_wgpu::LazyBufferUsage {
            allocations: 2,
            reuses: 1,
        },
        records: zetesis_wgpu::LazyBufferUsage {
            allocations: 3,
            reuses: 0,
        },
        snapshots: zetesis_wgpu::LazyBufferUsage {
            allocations: 1,
            reuses: 2,
        },
        seeds: zetesis_wgpu::LazyBufferUsage {
            allocations: 1,
            reuses: 2,
        },
        output: zetesis_wgpu::LazyBufferUsage {
            allocations: 2,
            reuses: 1,
        },
        readback: zetesis_wgpu::LazyBufferUsage {
            allocations: 2,
            reuses: 1,
        },
        budget_releases: 1,
        accounting_overflow_releases: 2,
    };
    let device = zetesis_wgpu::LazyGpuStatistics {
        transport_usage: usage,
        ..Default::default()
    };
    stats
        .record(1, false, zetesis_cpu::lazy::Progress::default(), &device)
        .unwrap();
    assert_eq!(
        stats.transport_usage,
        crate::LazyTransportUsage {
            uniform: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 6
            },
            offsets: crate::LazyBufferUsage {
                allocations: 3,
                reuses: 5
            },
            records: crate::LazyBufferUsage {
                allocations: 5,
                reuses: 3
            },
            snapshots: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 6
            },
            seeds: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 6
            },
            output: crate::LazyBufferUsage {
                allocations: 4,
                reuses: 4
            },
            readback: crate::LazyBufferUsage {
                allocations: 4,
                reuses: 4
            },
            budget_releases: 1,
            accounting_overflow_releases: 2,
        }
    );
}

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
            &zetesis_wgpu::LazyGpuStatistics {
                downloaded_bytes: 1,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, crate::SolveError::LazyStatisticsOverflow));
    assert_eq!(stats, before);
}

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
            &zetesis_wgpu::LazyGpuStatistics {
                host_wait: std::time::Duration::from_nanos(1),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, crate::SolveError::LazyStatisticsOverflow));
    assert_eq!(stats, before);
}

mod classification {
    use std::error::Error;
    use zetesis_core::{
        AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
    };
    use zetesis_cpu::{Cancellation, Stop, lazy};
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
            &Cancellation::default(),
            |chunk| Ok::<_, GpuError>(lazy::evaluate(chunk).unwrap()),
        );
        let actual = crate::lazy_execution::batch_results(batch).unwrap();
        assert_eq!(
            actual,
            vec![
                Ok(None),
                Ok(Some(Model::new([atom("a")]).unwrap())),
                Ok(Some(Model::new([atom("b")]).unwrap())),
                Ok(None),
                Ok(Some(Model::new([atom("a")]).unwrap()))
            ]
        );
    }

    fn payload(model: &Model) -> &str {
        let zetesis_core::ValueNodeRef::String(value) = model
            .atoms()
            .first()
            .unwrap()
            .values()
            .at(0)
            .unwrap()
            .descriptor()
        else {
            panic!("fixture contains one string argument");
        };
        value
    }

    #[test]
    fn batch_results_transfer_payload_storage() {
        let head = AtomPattern::new(
            Predicate::new("message", 1).unwrap(),
            vec![Term::Constant(Value::String("retained payload".into()))],
        )
        .unwrap();
        let program = Program::new(
            vec![Template::new(Some(head), vec![], vec![], vec![], vec![])],
            AdmissionLimits::default(),
        )
        .unwrap();
        // CPU-authored chunks exercise the session adapter, not device execution.
        let batch = lazy::check_with(
            &program,
            &[Seed::new(&program, []).unwrap()],
            lazy::Limits::default(),
            &Cancellation::default(),
            |chunk| Ok::<_, GpuError>(lazy::evaluate(chunk).unwrap()),
        )
        .unwrap();
        assert!(batch.checks[0].accepted());
        let original = payload(batch.checks[0].closure()).as_ptr();
        let mut results = crate::lazy_execution::batch_results(Ok(batch)).unwrap();
        assert_eq!(results.len(), 1);
        let model = results.pop().unwrap().unwrap().unwrap();
        assert_eq!(payload(&model), "retained payload");
        assert_eq!(payload(&model).as_ptr(), original);
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
            &Cancellation::default(),
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
            &Cancellation::default(),
            |_| Ok::<_, GpuError>(vec![]),
        );
        let error = crate::lazy_execution::batch_results(batch).unwrap_err();
        assert!(matches!(error, crate::SolveError::LazyGpu(_)));
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
