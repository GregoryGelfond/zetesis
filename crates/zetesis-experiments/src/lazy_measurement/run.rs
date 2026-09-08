use std::{io, num::NonZeroUsize, time::Instant};

use zetesis_cpu::{BatchOracle, Check, Control, lazy};
use zetesis_wgpu::{GpuBackendPreference, GpuLazyOracle, GpuOptions, GpuSelection};

use super::{Configuration, Error, Event, Phase, Route, Sample, fixture, view};
use crate::Backend;

const CPU_ROUTES: [Route; 4] = [
    Route::Scalar,
    Route::Rayon,
    Route::PortableUnion,
    Route::PortableWorlds,
];
const METAL_ROUTES: [Route; 6] = [
    Route::Scalar,
    Route::Rayon,
    Route::PortableUnion,
    Route::PortableWorlds,
    Route::MetalUnion,
    Route::MetalWorlds,
];

/// Execute a fixed rotating schedule and synchronously publish typed events.
/// Every route is checked against the same precomputed scalar reference outside
/// its timer. Each phase starts with the declared route order; iteration i
/// rotates it left by i. Six Metal repetitions balance all six route positions.
/// Programs, seeds and reference checks remain live during the samples.
///
/// The pool/device are reused across cases; lazy catalog/source/transport state
/// is rebuilt per call. This is not a resident-transport experiment. No outer
/// candidate search, parsing, objectives, peak RSS or kernel timestamps are timed.
/// Caller-owned binaries/source identity and host-load control remain external.
///
/// # Errors
/// Stops at the first configuration, oracle, parity or observation failure.
/// Source failures retain charged progress in the typed error. The completion
/// event is emitted only after every requested sample completed successfully.
pub fn measure(
    configuration: &Configuration,
    mut observe: impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    configuration.validate()?;
    observe(&Event::Configuration { schema: 1, configuration,
        scope: "relational-source-scans-and-complete-reduct-checks; excludes outer search, fixture/setup/parity/publication",
        peak_rss: "unavailable: source mask payload and transfer bytes are not process RSS",
    }).map_err(Error::Output)?;
    let maximum = configuration
        .cases
        .iter()
        .map(|case| case.worlds)
        .max()
        .ok_or(Error::Configuration("no candidate batch"))?;
    let mut execution = Execution::new(configuration, maximum, &mut observe)?;
    let routes = match configuration.backend {
        Backend::Cpu => &CPU_ROUTES[..],
        Backend::Metal => &METAL_ROUTES[..],
    };
    let mut emitted = 0;
    for (case_index, case) in configuration.cases.iter().copied().enumerate() {
        let started = Instant::now();
        let fixture = fixture::build(case)?;
        let fixture_ns = started.elapsed().as_nanos();
        let started = Instant::now();
        let expected = scalar(&fixture, configuration)?;
        observe(&Event::Prepared {
            case_index,
            fixture_ns,
            reference_ns: started.elapsed().as_nanos(),
        })
        .map_err(Error::Output)?;
        for Slot {
            phase,
            iteration,
            position,
            route,
        } in schedule(configuration, routes)
        {
            let measured = match execution.check(configuration, &fixture, &expected, route) {
                Ok(measured) => measured,
                Err(error) => {
                    let source = match &error {
                        Error::Source(failure) => Some(failure.progress.into()),
                        Error::Metal(failure) => Some(failure.progress.into()),
                        _ => None,
                    };
                    let device = if matches!(route, Route::MetalUnion | Route::MetalWorlds) {
                        execution
                            .metal
                            .as_ref()
                            .map(|oracle| oracle.statistics().into())
                    } else {
                        None
                    };
                    observe(&Event::Failed {
                        case_index,
                        phase,
                        iteration,
                        position,
                        route,
                        message: error.to_string(),
                        source,
                        device,
                    })
                    .map_err(Error::Output)?;
                    return Err(error);
                }
            };
            observe(&Event::Sample(Sample {
                case_index,
                case,
                phase,
                iteration,
                position,
                route,
                elapsed_ns: measured.elapsed_ns,
                checked: expected.len(),
                accepted: expected.iter().filter(|check| check.accepted()).count(),
                source: measured.source,
                device: measured.device,
            }))
            .map_err(Error::Output)?;
            emitted += 1;
        }
    }
    observe(&Event::Complete { samples: emitted }).map_err(Error::Output)
}

struct Slot {
    phase: Phase,
    iteration: usize,
    position: usize,
    route: Route,
}

fn schedule<'a>(
    configuration: &Configuration,
    routes: &'a [Route],
) -> impl Iterator<Item = Slot> + 'a {
    [
        (Phase::Initial, 1),
        (Phase::Warmup, configuration.warmups),
        (Phase::Timed, configuration.repetitions.get()),
    ]
    .into_iter()
    .flat_map(move |(phase, iterations)| {
        (0..iterations).flat_map(move |iteration| {
            (0..routes.len()).map(move |position| Slot {
                phase,
                iteration,
                position,
                route: routes[(iteration + position) % routes.len()],
            })
        })
    })
}

struct Execution {
    pool: BatchOracle,
    metal: Option<GpuLazyOracle>,
}

struct Measured {
    elapsed_ns: u128,
    source: Option<view::SourceWork>,
    device: Option<view::DeviceWork>,
}

impl Execution {
    fn new(
        configuration: &Configuration,
        maximum: NonZeroUsize,
        observe: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
    ) -> Result<Self, Error> {
        let started = Instant::now();
        let pool = BatchOracle::new(configuration.workers, maximum).map_err(Error::Pool)?;
        let pool_init_ns = started.elapsed().as_nanos();
        let started = Instant::now();
        let metal = if configuration.backend == Backend::Metal {
            Some(
                GpuLazyOracle::new_selected(
                    GpuOptions::default(),
                    GpuSelection {
                        backend: GpuBackendPreference::Metal,
                        vendor_id: None,
                    },
                )
                .map_err(Error::Device)?,
            )
        } else {
            None
        };
        let device_init_ns = metal.as_ref().map(|_| started.elapsed().as_nanos());
        observe(&Event::Setup {
            workers: configuration.workers.get(),
            pool_init_ns,
            adapter: metal.as_ref().map(|oracle| oracle.info().name().to_owned()),
            backend: metal
                .as_ref()
                .map(|oracle| oracle.info().backend().to_owned()),
            device_init_ns,
        })
        .map_err(Error::Output)?;
        Ok(Self { pool, metal })
    }

    fn check(
        &mut self,
        configuration: &Configuration,
        fixture: &fixture::Fixture,
        expected: &[Check],
        route: Route,
    ) -> Result<Measured, Error> {
        let control = Control::default();
        let started = Instant::now();
        match route {
            Route::Scalar | Route::Rayon => {
                let checks = if route == Route::Scalar {
                    scalar(fixture, configuration)?
                } else {
                    self.pool
                        .check_batch(
                            &fixture.program,
                            &fixture.seeds,
                            configuration.cpu_limits,
                            &control,
                        )
                        .map_err(Error::Pool)?
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(Error::Cpu)?
                };
                let elapsed_ns = started.elapsed().as_nanos();
                if expected.len() != checks.len()
                    || expected.iter().zip(&checks).any(|(left, right)| {
                        left.closure() != right.closure()
                            || left.constraint_violated() != right.constraint_violated()
                            || left.seed_mismatch() != right.seed_mismatch()
                    })
                {
                    return Err(Error::Parity);
                }
                Ok(Measured {
                    elapsed_ns,
                    source: None,
                    device: None,
                })
            }
            Route::PortableUnion
            | Route::PortableWorlds
            | Route::MetalUnion
            | Route::MetalWorlds => {
                let selection = match route {
                    Route::PortableWorlds | Route::MetalWorlds => lazy::SourceSelection::Worlds,
                    _ => lazy::SourceSelection::Union,
                };
                let (batch, device) = if matches!(route, Route::MetalUnion | Route::MetalWorlds) {
                    let oracle = self
                        .metal
                        .as_mut()
                        .ok_or(Error::Configuration("Metal route has no device"))?;
                    let batch = oracle
                        .check_batch_with_source(
                            &fixture.program,
                            &fixture.seeds,
                            configuration.source_limits,
                            configuration.gpu_limits,
                            selection,
                            &control,
                        )
                        .map_err(Error::Metal)?;
                    (batch, Some(oracle.statistics()))
                } else {
                    (
                        lazy::check_with_source(
                            &fixture.program,
                            &fixture.seeds,
                            configuration.source_limits,
                            selection,
                            &control,
                            lazy::evaluate,
                        )
                        .map_err(Error::Source)?,
                        None,
                    )
                };
                let elapsed_ns = started.elapsed().as_nanos();
                compare(expected, &batch)?;
                if let Some(statistics) = device {
                    verify_device_work(statistics, batch.progress, fixture.seeds.len())?;
                }
                Ok(Measured {
                    elapsed_ns,
                    source: Some(batch.progress.into()),
                    device: device.map(Into::into),
                })
            }
        }
    }
}

fn scalar(fixture: &fixture::Fixture, configuration: &Configuration) -> Result<Vec<Check>, Error> {
    let control = Control::default();
    fixture
        .seeds
        .iter()
        .map(|seed| {
            zetesis_cpu::check(&fixture.program, seed, configuration.cpu_limits, &control)
                .map_err(Error::Cpu)
        })
        .collect()
}

fn compare(expected: &[Check], batch: &lazy::Batch) -> Result<(), Error> {
    if expected.len() != batch.checks.len()
        || expected.iter().zip(&batch.checks).any(|(left, right)| {
            left.closure() != right.closure()
                || left.constraint_violated() != right.constraint_violated()
                || left.seed_mismatch() != right.seed_mismatch()
        })
    {
        return Err(Error::Parity);
    }
    Ok(())
}

fn verify_device_work(
    statistics: zetesis_wgpu::LazyGpuStatistics,
    progress: lazy::Progress,
    worlds: usize,
) -> Result<(), Error> {
    let world_instances = u64::try_from(worlds)
        .ok()
        .and_then(|worlds| progress.instances.checked_mul(worlds));
    if statistics.dispatches == 0
        || statistics.world_instances == 0
        || statistics.dispatches != progress.chunks
        || Some(statistics.world_instances) != world_instances
        || statistics.uploaded_bytes == 0
        || statistics.downloaded_bytes == 0
    {
        return Err(Error::DeviceWork);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/support/lazy_measurement.rs"]
mod tests;
