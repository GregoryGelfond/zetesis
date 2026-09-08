use super::{
    Activity, Case, Configuration, DeviceWork, Error, Event, Observation, Outcome, Phase, Route,
    Sample, checking, fixture, view::Preparation,
};
use std::{io, time::Instant};
use zetesis_cpu::Control;
use zetesis_ferraris::native_aggregate as native;
use zetesis_wgpu::{AggregateGpuPlan, AggregateGpuPlanLimits, GpuAggregateOracle, GpuOptions};

const CPU_ROUTES: &[Route] = &[Route::Scalar, Route::Rayon];
const DEVICE_ROUTES: &[Route] = &[
    Route::Scalar,
    Route::Rayon,
    Route::DeviceFresh,
    Route::DeviceResident,
];
// Independent checks of this experiment schema's fixed device wire contract.
const DEVICE_LANES: u64 = 64;
const DEVICE_UNIFORM_BYTES: u64 = 32;
const DEVICE_RESULT_BYTES: u64 = 40;
const WORD_BITS: usize = 32;
const WORD_BYTES: u64 = 4;

struct Resources {
    pool: rayon::ThreadPool,
    fresh: Option<GpuAggregateOracle>,
    resident: Option<GpuAggregateOracle>,
}
impl Resources {
    fn new(
        configuration: &Configuration,
        control: &Control,
        emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
    ) -> Result<Self, Error> {
        control.poll().map_err(Error::Cpu)?;
        let start = Instant::now();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(configuration.workers.get())
            .build()
            .map_err(Error::Pool)?;
        publish(
            emit,
            &Event::Setup {
                route: Route::Rayon,
                elapsed_ns: start.elapsed().as_nanos(),
                adapter: None,
            },
        )?;
        let mut resources = Self {
            pool,
            fresh: None,
            resident: None,
        };
        if let Some(selection) = configuration.backend.selection() {
            for (route, slot) in [
                (Route::DeviceFresh, &mut resources.fresh),
                (Route::DeviceResident, &mut resources.resident),
            ] {
                control.poll().map_err(Error::Cpu)?;
                let start = Instant::now();
                let oracle = GpuAggregateOracle::new_selected(GpuOptions::default(), selection)
                    .map_err(Error::Device)?;
                let expected = match configuration.backend {
                    crate::Backend::Metal => "Metal",
                    crate::Backend::Vulkan => "Vulkan",
                    crate::Backend::Cpu => return Err(Error::Accounting),
                };
                if oracle.info().backend() != expected || !oracle.info().is_hardware_gpu() {
                    return Err(Error::Accounting);
                }
                publish(
                    emit,
                    &Event::Setup {
                        route,
                        elapsed_ns: start.elapsed().as_nanos(),
                        adapter: Some(oracle.info().metadata().into()),
                    },
                )?;
                *slot = Some(oracle);
            }
            if resources
                .fresh
                .as_ref()
                .map(|oracle| oracle.info().metadata())
                != resources
                    .resident
                    .as_ref()
                    .map(|oracle| oracle.info().metadata())
            {
                return Err(Error::Accounting);
            }
        }
        Ok(resources)
    }
}

fn publish(
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
    event: &Event<'_>,
) -> Result<(), Error> {
    emit(event).map_err(Error::Output)
}

struct Prepared<'a> {
    numeric: AggregateGpuPlan<'a>,
    records: Vec<native::Eligibility<'a>>,
    reference: Vec<Outcome>,
    preparation: Preparation,
}
impl<'a> Prepared<'a> {
    fn new(
        fixture: &'a fixture::Fixture,
        fixture_ns: u128,
        configuration: &Configuration,
        control: &Control,
    ) -> Result<Self, Error> {
        let mut preparation = Preparation {
            fixture_ns,
            admission_work: fixture.group.statistics().work,
            ..Default::default()
        };
        let start = Instant::now();
        let records = fixture::acquire(fixture, configuration, control)?;
        preparation.acquisition_ns = start.elapsed().as_nanos();
        for record in &records {
            let statistics = record.statistics();
            preparation.acquisition_work = preparation
                .acquisition_work
                .checked_add(statistics.work)
                .ok_or(Error::Accounting)?;
            preparation.eligibility_bytes = preparation
                .eligibility_bytes
                .checked_add(statistics.resident_bytes)
                .ok_or(Error::Accounting)?;
            preparation.acquisition_peak_bytes = preparation
                .acquisition_peak_bytes
                .max(statistics.peak_bytes);
        }
        let start = Instant::now();
        let numeric =
            AggregateGpuPlan::new(&fixture.group, AggregateGpuPlanLimits::default(), control)
                .map_err(Error::Gpu)?;
        preparation.numeric_ns = start.elapsed().as_nanos();
        preparation.numeric_work = numeric.work();
        preparation.numeric_bytes = numeric.bytes();
        let mut activity = Activity::default();
        let start = Instant::now();
        let reference = checking::cpu(
            &records,
            configuration.max_reference_work,
            None,
            control,
            &mut activity,
        )?;
        preparation.reference_ns = start.elapsed().as_nanos();
        preparation.reference_work = activity.cpu_work;
        Ok(Self {
            numeric,
            records,
            reference,
            preparation,
        })
    }
}

/// Measure identical acquired occurrences across scalar, Rayon and requested GPU.
///
/// Each case reports fixture, actual formula acquisition, numeric preparation and
/// exact native references outside sample clocks. Fresh GPU residency is cleared
/// before timing; its measured call includes new group/transport allocations and
/// upload. A separate resident oracle is primed by the initial observation and
/// must reuse both group and exact transport thereafter. Neither route reuses
/// earlier reductions. Every original/frozen value and guard is compared outside
/// timers; duplicates and original-only records retain their positions.
///
/// # Errors
/// Refuses invalid schedules, incomplete native/device operations, mismatched
/// results/accounting or writer errors. Measured failures emit their attempted
/// observation; preparation/setup failures retain only their preceding prefix.
/// Failed positions are never replaced and no completion follows any error.
pub fn measure(
    configuration: &Configuration,
    emit: impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    measure_with_control(configuration, &Control::default(), emit)
}

/// Run the same experiment under shared cooperative cancellation/deadline control.
///
/// # Errors
/// Returns the same failures as [`measure`], including native/device control.
pub fn measure_with_control(
    configuration: &Configuration,
    control: &Control,
    mut emit: impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    configuration.validate()?;
    publish(
        &mut emit,
        &Event::Configuration {
            schema: 1,
            configuration,
            scope: "one complete native Group; identical acquired original/frozen Eligibility occurrences; fixture, actual mask acquisition, numeric preparation and native reference have separate setup intervals; reduction clocks include result allocation/conversion and actual GPU upload/dispatch/readback; pool/two-device pipeline setup, fresh cache clearing, parity checks, destruction and publication excluded; CPU and GPU retain distinct work units; no grounding, outer search, stable-membership, shader timestamps, process RSS or ordinary solver speedup claim",
        },
    )?;
    let mut resources = Resources::new(configuration, control, &mut emit)?;
    let mut samples = 0;
    for (index, &case) in configuration.cases.iter().enumerate() {
        let start = Instant::now();
        let fixture = fixture::build(case, control)?;
        let prepared = Prepared::new(&fixture, start.elapsed().as_nanos(), configuration, control)?;
        publish(
            &mut emit,
            &Event::Prepared {
                case_index: index,
                case,
                fixture_version: 1,
                preparation: prepared.preparation,
                interpretations: &fixture.interpretations,
                reference: &prepared.reference,
            },
        )?;
        samples += run_case(
            index,
            case,
            &prepared,
            configuration,
            control,
            &mut resources,
            &mut emit,
        )?;
    }
    control.poll().map_err(Error::Cpu)?;
    publish(&mut emit, &Event::Complete { samples })
}

fn run_case(
    index: usize,
    case: Case,
    prepared: &Prepared<'_>,
    configuration: &Configuration,
    control: &Control,
    resources: &mut Resources,
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<usize, Error> {
    let routes = if configuration.backend == crate::Backend::Cpu {
        CPU_ROUTES
    } else {
        DEVICE_ROUTES
    };
    let mut samples = 0;
    for (phase, iterations) in [
        (Phase::Initial, 1),
        (Phase::Warmup, configuration.warmups),
        (Phase::Timed, configuration.repetitions.get()),
    ] {
        for iteration in 0..iterations {
            for position in 0..routes.len() {
                let route = routes[(position + iteration) % routes.len()];
                let observation = Observation {
                    case_index: index,
                    phase,
                    iteration,
                    position,
                    route,
                    elapsed_ns: 0,
                    activity: Activity::default(),
                };
                let sample = observe(
                    case,
                    prepared,
                    configuration,
                    control,
                    resources,
                    observation,
                    emit,
                )?;
                publish(emit, &Event::Sample { sample: &sample })?;
                samples += 1;
            }
        }
    }
    Ok(samples)
}

fn observe(
    case: Case,
    prepared: &Prepared<'_>,
    configuration: &Configuration,
    control: &Control,
    resources: &mut Resources,
    mut observation: Observation,
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<Sample, Error> {
    if observation.route == Route::DeviceFresh {
        resources
            .fresh
            .as_mut()
            .ok_or(Error::Accounting)?
            .clear_residency();
    }
    let start = Instant::now();
    let result = reduce(
        prepared,
        configuration,
        control,
        resources,
        &mut observation,
    );
    observation.elapsed_ns = start.elapsed().as_nanos();
    let validated = result.and_then(|values| {
        if values != prepared.reference {
            return Err(Error::Parity);
        }
        validate_activity(case, prepared, &observation)?;
        control.poll().map_err(Error::Cpu)?;
        Ok(values)
    });
    match validated {
        Ok(outcomes) => Ok(Sample {
            observation,
            outcomes,
        }),
        Err(error) => {
            publish(
                emit,
                &Event::Failed {
                    observation,
                    reason: &error.to_string(),
                },
            )?;
            Err(error)
        }
    }
}

fn reduce(
    prepared: &Prepared<'_>,
    configuration: &Configuration,
    control: &Control,
    resources: &mut Resources,
    observation: &mut Observation,
) -> Result<Vec<Outcome>, Error> {
    match observation.route {
        Route::Scalar | Route::Rayon => checking::cpu(
            &prepared.records,
            configuration.max_reduction_work,
            (observation.route == Route::Rayon).then_some(&resources.pool),
            control,
            &mut observation.activity,
        ),
        Route::DeviceFresh | Route::DeviceResident => {
            let oracle = if observation.route == Route::DeviceFresh {
                resources.fresh.as_mut()
            } else {
                resources.resident.as_mut()
            }
            .ok_or(Error::Accounting)?;
            let result = oracle.check_batch(
                &prepared.numeric,
                &prepared.records,
                configuration.gpu_limits(),
                control,
            );
            observation.activity.device = Some(DeviceWork {
                activity: oracle.activity(),
                batch: oracle.last_batch_stats().copied(),
            });
            checking::device(&result.map_err(Error::Gpu)?)
        }
    }
}

fn validate_activity(
    case: Case,
    prepared: &Prepared<'_>,
    observation: &Observation,
) -> Result<(), Error> {
    let activity = observation.activity;
    if matches!(observation.route, Route::Scalar | Route::Rayon) {
        return if activity.device.is_none()
            && activity.cpu_attempts == case.occurrences.get()
            && activity.cpu_completed == case.occurrences.get()
            && activity.cpu_work == prepared.preparation.reference_work
        {
            Ok(())
        } else {
            Err(Error::Accounting)
        };
    }
    let device = activity.device.ok_or(Error::Accounting)?;
    let batch = device.batch.ok_or(Error::Accounting)?;
    let expected = case.occurrences.get() as u64;
    let guards = prepared.numeric.group().guards().len() as u64;
    // Configuration bounds make each product fit comfortably in u64.
    let work = 2 * (case.tuples as u64 + guards + DEVICE_LANES - 1) * expected;
    let fresh = observation.route == Route::DeviceFresh || observation.phase == Phase::Initial;
    let masks = expected * (1 + 2 * case.tuples.div_ceil(WORD_BITS)) as u64 * WORD_BYTES;
    let results = DEVICE_RESULT_BYTES * expected;
    let transport = DEVICE_UNIFORM_BYTES + masks + 2 * results;
    let host_work = masks / WORD_BYTES + expected * (case.tuples as u64 + 2 + 2 * guards);
    let accounted = 2 * prepared.numeric.bytes()
        + transport
        + masks
        + results
        + expected * size_of::<zetesis_wgpu::AggregateGpuReduction>() as u64;
    if activity.cpu_attempts != 0
        || activity.cpu_completed != 0
        || activity.cpu_work != 0
        || device.activity.submissions != 1
        || device.activity.submitted_occurrences != expected
        || device.activity.completed_occurrences != expected
        || device.activity.scheduled_work != work
        || device.activity.completed_work != work
        || device.activity.downloaded_bytes != results
        || device.activity.uploaded_bytes
            != u64::from(fresh) * prepared.numeric.bytes() + DEVICE_UNIFORM_BYTES + masks
        || batch.occurrences != expected
        || batch.device_work != work
        || batch.group_uploaded != fresh
        || batch.transport_allocated != fresh
        || batch.resident_group_bytes != prepared.numeric.bytes()
        || batch.resident_transport_bytes != transport
        || batch.accounted_bytes != accounted
        || batch.host_work != host_work
        || (observation.route == Route::DeviceFresh && batch.incoming_resident_bytes != 0)
        || (!fresh && batch.incoming_resident_bytes != 2 * prepared.numeric.bytes() + transport)
    {
        return Err(Error::Accounting);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/aggregate/device_work.rs"]
mod tests;
