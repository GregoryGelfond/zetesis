use std::num::NonZeroUsize;

use super::*;
use crate::lazy_measurement::{Case, Family};

#[test]
fn six_iterations_balance_physical_route_positions() {
    let (_, mut configuration) = fixture();
    configuration.repetitions = NonZeroUsize::new(6).unwrap();
    for routes in [METAL_ROUTES, VULKAN_ROUTES] {
        let slots = schedule(&configuration, &routes)
            .filter(|slot| slot.phase == Phase::Timed)
            .collect::<Vec<_>>();
        assert_eq!(slots.len(), 36);
        for route in routes {
            let mut positions = slots
                .iter()
                .filter(|slot| slot.route == route)
                .map(|slot| slot.position)
                .collect::<Vec<_>>();
            positions.sort_unstable();
            assert_eq!(positions, [0, 1, 2, 3, 4, 5]);
        }
    }
}

fn fixture() -> (fixture::Fixture, Configuration) {
    let configuration = Configuration {
        cases: vec![Case {
            family: Family::Sparse,
            width: NonZeroUsize::new(2).unwrap(),
            worlds: NonZeroUsize::new(2).unwrap(),
        }],
        backend: Backend::Cpu,
        warmups: 0,
        repetitions: NonZeroUsize::new(1).unwrap(),
        workers: NonZeroUsize::new(1).unwrap(),
        cpu_limits: zetesis_cpu::Limits::default(),
        source_limits: lazy::Limits::default(),
        gpu_limits: zetesis_wgpu::GpuLimits::default(),
    };
    (
        fixture::build(configuration.cases[0]).unwrap(),
        configuration,
    )
}

fn varied_checks() -> [Check; 4] {
    let (fixture, configuration) = fixture();
    let mut checks = scalar(&fixture, &configuration).unwrap();
    let dense = zetesis_core::SeedSelection::from_carrier_atoms(
        &fixture.program,
        fixture
            .program
            .gate_atoms()
            .map(|atom| atom.expect("finite fixture carrier"))
            .filter(|atom| fixture.seeds.iter().any(|seed| seed.contains(atom.atom()))),
    )
    .unwrap()
    .to_seed();
    checks.push(
        zetesis_cpu::check(
            &fixture.program,
            &dense,
            configuration.cpu_limits,
            &Cancellation::default(),
        )
        .unwrap(),
    );
    // Preserve a second occurrence of the accepted sparse candidate.
    checks.push(checks[1].clone());
    assert_eq!(checks.iter().filter(|check| check.accepted()).count(), 2);
    assert_eq!(
        checks
            .iter()
            .map(|check| check.closure().atoms().len())
            .collect::<Vec<_>>(),
        [5, 5, 16, 5]
    );
    checks.try_into().unwrap()
}

#[test]
fn independent_work_sums_completed_occurrences() {
    let checks = varied_checks();
    let [first, second, dense, repeated] = checks.each_ref().map(Check::statistics);
    let total = view::IndependentWork::from_checks(&checks);
    assert_eq!(total.derived_atoms, 31);
    assert_eq!(
        total.work,
        u128::from(first.work)
            + u128::from(second.work)
            + u128::from(dense.work)
            + u128::from(repeated.work)
    );
    assert_eq!(
        total.catalog_work,
        u128::from(first.catalog_work)
            + u128::from(second.catalog_work)
            + u128::from(dense.catalog_work)
            + u128::from(repeated.catalog_work)
    );
    assert_eq!(
        total.rounds,
        u128::from(first.rounds)
            + u128::from(second.rounds)
            + u128::from(dense.rounds)
            + u128::from(repeated.rounds)
    );
    assert_eq!(
        total.bindings,
        u128::from(first.bindings)
            + u128::from(second.bindings)
            + u128::from(dense.bindings)
            + u128::from(repeated.bindings)
    );
    assert!(
        checks
            .iter()
            .all(|check| check.statistics().tuple_probes > 0)
    );
    assert_eq!(
        total.tuple_probes,
        u128::from(first.tuple_probes)
            + u128::from(second.tuple_probes)
            + u128::from(dense.tuple_probes)
            + u128::from(repeated.tuple_probes)
    );
}

#[test]
fn independent_peak_is_largest_candidate_envelope() {
    let checks = varied_checks();
    let peaks = checks
        .iter()
        .map(|check| check.statistics().peak_closure_bytes)
        .collect::<Vec<_>>();
    let total = view::IndependentWork::from_checks(&checks);
    assert!(peaks.iter().all(|peak| *peak > 0));
    assert_eq!(total.peak_closure_bytes, *peaks.iter().max().unwrap());
    assert!(total.peak_closure_bytes < peaks.iter().sum::<usize>());
}

#[test]
fn rayon_snapshot_matches_owned_pool() {
    let (fixture, configuration) = fixture();
    let expected = scalar(&fixture, &configuration).unwrap();
    let mut execution = Execution::new(&configuration, configuration.cases[0].worlds, &mut |_| {
        Ok(())
    })
    .unwrap();
    for reused in [0, 1] {
        let measured = execution
            .check(&configuration, &fixture, &expected, Route::Rayon)
            .unwrap();
        let actual = execution.pool.query_statistics().unwrap();
        assert_eq!(measured.queries, Some(actual));
        assert_eq!(actual.preparation_builds, 1);
        assert_eq!(actual.active_workspaces, 1);
        assert_eq!(actual.reused_workspaces, reused);
        assert!(actual.preparation.is_some());
        assert!(actual.retained_bytes > 0);
        assert!(actual.reserved_bytes >= actual.retained_bytes);
    }
}

#[test]
fn reordered_checks_fail_parity() {
    let (fixture, configuration) = fixture();
    let expected = scalar(&fixture, &configuration).unwrap();
    let mut batch = lazy::check_with(
        &fixture.program,
        &fixture.seeds,
        configuration.source_limits,
        &Cancellation::default(),
        lazy::evaluate,
    )
    .unwrap();
    compare(&expected, &batch).unwrap();
    batch.checks.reverse();
    assert!(matches!(compare(&expected, &batch), Err(Error::Parity)));
}

#[test]
fn missing_checks_fail_parity() {
    let (fixture, configuration) = fixture();
    let expected = scalar(&fixture, &configuration).unwrap();
    let mut batch = lazy::check_with(
        &fixture.program,
        &fixture.seeds,
        configuration.source_limits,
        &Cancellation::default(),
        lazy::evaluate,
    )
    .unwrap();
    batch.checks.pop();
    assert!(matches!(compare(&expected, &batch), Err(Error::Parity)));
}

#[test]
fn inconsistent_device_counters_refuse_qualification() {
    let progress = lazy::Progress {
        chunks: 3,
        instances: 5,
        ..Default::default()
    };
    let actual = zetesis_wgpu::LazyGpuStatistics {
        dispatches: 3,
        world_instances: 10,
        uploaded_bytes: 64,
        downloaded_bytes: 48,
        transport_allocations: 1,
        transport_reuses: 2,
        peak_transport_bytes: 256,
        transport_usage: usage(1, 2),
        ..Default::default()
    };
    verify_device_work(&actual, progress, 2).unwrap();
    for forged in [
        zetesis_wgpu::LazyGpuStatistics {
            dispatches: 0,
            ..actual
        },
        zetesis_wgpu::LazyGpuStatistics {
            dispatches: 2,
            ..actual
        },
        zetesis_wgpu::LazyGpuStatistics {
            world_instances: 5,
            ..actual
        },
        zetesis_wgpu::LazyGpuStatistics {
            uploaded_bytes: 0,
            ..actual
        },
        zetesis_wgpu::LazyGpuStatistics {
            downloaded_bytes: 0,
            ..actual
        },
        zetesis_wgpu::LazyGpuStatistics {
            transport_allocations: 0,
            ..actual
        },
        zetesis_wgpu::LazyGpuStatistics {
            transport_reuses: 0,
            ..actual
        },
        zetesis_wgpu::LazyGpuStatistics {
            transport_reuses: u64::MAX,
            ..actual
        },
        zetesis_wgpu::LazyGpuStatistics {
            peak_transport_bytes: 0,
            ..actual
        },
    ] {
        assert!(matches!(
            verify_device_work(&forged, progress, 2),
            Err(Error::DeviceWork)
        ));
    }
    assert!(matches!(
        verify_device_work(
            &actual,
            lazy::Progress {
                instances: u64::MAX,
                ..progress
            },
            2
        ),
        Err(Error::DeviceWork)
    ));
    assert!(matches!(
        verify_device_work(
            &zetesis_wgpu::LazyGpuStatistics {
                world_instances: 0,
                ..actual
            },
            lazy::Progress {
                instances: 0,
                ..progress
            },
            2
        ),
        Err(Error::DeviceWork)
    ));
}

fn usage(allocations: u64, reuses: u64) -> zetesis_wgpu::LazyTransportUsage {
    let binding = zetesis_wgpu::LazyBufferUsage {
        allocations,
        reuses,
    };
    zetesis_wgpu::LazyTransportUsage {
        uniform: binding,
        offsets: binding,
        records: binding,
        snapshots: binding,
        seeds: binding,
        output: binding,
        readback: binding,
        budget_releases: 0,
        accounting_overflow_releases: 0,
    }
}

#[test]
fn inconsistent_binding_counts_refuse_qualification() {
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
    let progress = lazy::Progress {
        chunks: 3,
        instances: 5,
        ..Default::default()
    };
    let actual = zetesis_wgpu::LazyGpuStatistics {
        dispatches: 3,
        world_instances: 10,
        uploaded_bytes: 64,
        downloaded_bytes: 48,
        transport_allocations: 1,
        transport_reuses: 2,
        peak_transport_bytes: 256,
        transport_usage: usage(1, 2),
        ..Default::default()
    };
    verify_device_work(&actual, progress, 2).unwrap();
    for field in fields {
        for (allocations, reuses) in [(0, 3), (1, 1), (2, 1), (u64::MAX, 2)] {
            let mut forged = actual;
            *field(&mut forged.transport_usage) = zetesis_wgpu::LazyBufferUsage {
                allocations,
                reuses,
            };
            assert!(verify_device_work(&forged, progress, 2).is_err());
        }
    }
    for overflow in [false, true] {
        let mut forged = actual;
        if overflow {
            forged.transport_usage.accounting_overflow_releases = 2;
        } else {
            forged.transport_usage.budget_releases = 2;
        }
        assert!(verify_device_work(&forged, progress, 2).is_err());
    }
}

#[test]
fn device_observations_preserve_recorded_units() {
    let work = view::DeviceWork::from(zetesis_wgpu::LazyGpuStatistics {
        dispatches: 2,
        world_instances: 3,
        uploaded_bytes: 4,
        downloaded_bytes: 5,
        transport_allocations: 1,
        transport_reuses: 1,
        peak_transport_bytes: 256,
        host_wait: std::time::Duration::from_nanos(6),
        transport_replacements: zetesis_wgpu::LazyTransportReplacements {
            initial: 1,
            ..Default::default()
        },
        transport_usage: usage(1, 1),
    });
    assert_eq!(
        serde_json::to_value(work).unwrap(),
        serde_json::json!({
            "dispatches": 2,
            "world_instances": 3,
            "uploaded_bytes": 4,
            "downloaded_bytes": 5,
            "transport_allocations": 1,
            "transport_reuses": 1,
            "peak_transport_bytes": 256,
            "host_wait_ns": 6,
            "transport_replacements": {
                "initial": 1,
                "offsets_growth": 0,
                "records_growth": 0,
                "snapshots_growth": 0,
                "seeds_growth": 0,
                "result_shape": 0,
                "budget": 0,
                "accounting_overflow": 0
            },
            "transport_usage": {
                "uniform": {"allocations": 1, "reuses": 1},
                "offsets": {"allocations": 1, "reuses": 1},
                "records": {"allocations": 1, "reuses": 1},
                "snapshots": {"allocations": 1, "reuses": 1},
                "seeds": {"allocations": 1, "reuses": 1},
                "output": {"allocations": 1, "reuses": 1},
                "readback": {"allocations": 1, "reuses": 1},
                "budget_releases": 0,
                "accounting_overflow_releases": 0
            }
        })
    );
}

#[test]
fn scalar_disagreement_cannot_be_timed_as_success() {
    let (fixture, configuration) = fixture();
    let mut expected = scalar(&fixture, &configuration).unwrap();
    expected.reverse();
    let mut execution = Execution::new(&configuration, configuration.cases[0].worlds, &mut |_| {
        Ok(())
    })
    .unwrap();
    for route in CPU_ROUTES {
        assert!(matches!(
            execution.check(&configuration, &fixture, &expected, route),
            Err(Error::Parity)
        ));
    }
}

#[test]
fn an_absent_device_cannot_run_a_metal_route() {
    let (fixture, configuration) = fixture();
    let expected = scalar(&fixture, &configuration).unwrap();
    let mut execution = Execution::new(&configuration, configuration.cases[0].worlds, &mut |_| {
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        execution.check(&configuration, &fixture, &expected, Route::MetalUnion),
        Err(Error::Configuration(_))
    ));
}
