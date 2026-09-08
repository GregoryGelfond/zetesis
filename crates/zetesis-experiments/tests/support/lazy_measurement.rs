use std::num::NonZeroUsize;

use super::*;
use crate::lazy_measurement::{Case, Family};

#[test]
fn six_iterations_balance_metal_route_positions() {
    let (_, mut configuration) = fixture();
    configuration.repetitions = NonZeroUsize::new(6).unwrap();
    let slots = schedule(&configuration, &METAL_ROUTES)
        .filter(|slot| slot.phase == Phase::Timed)
        .collect::<Vec<_>>();
    assert_eq!(slots.len(), 36);
    for route in METAL_ROUTES {
        let mut positions = slots
            .iter()
            .filter(|slot| slot.route == route)
            .map(|slot| slot.position)
            .collect::<Vec<_>>();
        positions.sort_unstable();
        assert_eq!(positions, [0, 1, 2, 3, 4, 5]);
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

#[test]
fn reordered_checks_fail_parity() {
    let (fixture, configuration) = fixture();
    let expected = scalar(&fixture, &configuration).unwrap();
    let mut batch = lazy::check_with(
        &fixture.program,
        &fixture.seeds,
        configuration.source_limits,
        &Control::default(),
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
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    batch.checks.pop();
    assert!(matches!(compare(&expected, &batch), Err(Error::Parity)));
}

#[test]
fn forged_device_counters_cannot_qualify_work() {
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
        ..Default::default()
    };
    verify_device_work(actual, progress, 2).unwrap();
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
    ] {
        assert!(matches!(
            verify_device_work(forged, progress, 2),
            Err(Error::DeviceWork)
        ));
    }
    assert!(matches!(
        verify_device_work(
            actual,
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
            zetesis_wgpu::LazyGpuStatistics {
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

#[test]
fn device_observations_retain_host_wait_units() {
    let work = view::DeviceWork::from(zetesis_wgpu::LazyGpuStatistics {
        dispatches: 2,
        world_instances: 3,
        uploaded_bytes: 4,
        downloaded_bytes: 5,
        host_wait: std::time::Duration::from_nanos(6),
    });
    assert_eq!(
        serde_json::to_value(work).unwrap(),
        serde_json::json!({
            "dispatches": 2, "world_instances": 3, "uploaded_bytes": 4, "downloaded_bytes": 5, "host_wait_ns": 6
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
