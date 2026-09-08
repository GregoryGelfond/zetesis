use std::{io, time::Instant};

use zetesis_cpu::Control;
use zetesis_ferraris::TightVerdict;
use zetesis_wgpu::{GpuOptions, GpuTightOracle};

use super::{
    Activity, Configuration, DeviceWork, Error, Event, Observation, Phase, Route, Sample,
    checking::{self, Prepared},
    reserve,
    view::{Formula, Producer, Residency},
};

const CPU_ROUTES: &[Route] = &[Route::Scalar, Route::Rayon];
const METAL_ROUTES: &[Route] = &[
    Route::Scalar,
    Route::Rayon,
    Route::MetalFresh,
    Route::MetalResident,
];

struct Resources {
    pool: rayon::ThreadPool,
    fresh: Option<GpuTightOracle>,
    resident: Option<GpuTightOracle>,
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
                resource: Route::Rayon,
                elapsed_ns: start.elapsed().as_nanos(),
                adapter: None,
            },
        )?;
        let mut resources = Self {
            pool,
            fresh: None,
            resident: None,
        };
        if configuration.backend == crate::Backend::Metal {
            let fresh = device(Route::MetalFresh, control, emit)?;
            let resident = device(Route::MetalResident, control, emit)?;
            if fresh.info().metadata() != resident.info().metadata() {
                return Err(Error::DeviceWork);
            }
            resources.fresh = Some(fresh);
            resources.resident = Some(resident);
        }
        Ok(resources)
    }
}

fn device(
    route: Route,
    control: &Control,
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<GpuTightOracle, Error> {
    control.poll().map_err(Error::Cpu)?;
    let start = Instant::now();
    let oracle = GpuTightOracle::new_metal(GpuOptions::default()).map_err(Error::Device)?;
    publish(
        emit,
        &Event::Setup {
            resource: route,
            elapsed_ns: start.elapsed().as_nanos(),
            adapter: Some(oracle.info().metadata().into()),
        },
    )?;
    Ok(oracle)
}

fn publish(
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
    event: &Event<'_>,
) -> Result<(), Error> {
    emit(event).map_err(Error::Output)
}

/// Measure a finite rotated schedule with every occurrence independently checked.
///
/// Each case emits its full original DAG, ranks, producers, candidates and exact
/// reference before sample observations. Tiny cases enumerate proper subsets;
/// wide cases use general native reduct search without a class certificate.
/// GPU instances are independently owned: fresh residency is cleared before its
/// call, and resident reuse is required after that case's initial observation.
/// Residuals complete serially on CPU on every route. Initial and warmup calls
/// remain separate from repeated samples. The caller's event sink is synchronous
/// and outside clocks; no whole-campaign deadline or process RSS is claimed.
///
/// # Errors
/// Refuses dimensions, incomplete references/checks, mismatched witnesses,
/// missing device work or writer failure. Setup/reference failures return after
/// their preceding prefix; measured failures additionally emit `Event::Failed`.
/// No failed position is replaced and no completion follows any error.
pub fn measure(
    configuration: &Configuration,
    emit: impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    measure_with_control(configuration, &Control::default(), emit)
}

/// Run the same finite experiment under caller-owned cooperative control.
/// Device waits and every native check retain their own published polling scope.
///
/// # Errors
/// Returns the same typed failures as [`measure`], including cancellation.
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
            scope: "complete immutable TightPlan and identical unfiltered ordered occurrences; host classification and serial general-reduct residual completion are directly measured inside whole-call elapsed, not additive; fixture/reference/witness validation, pool/two-device pipeline setup, fresh cache clearing, destruction and publication excluded; no grounding, outer search, shader timestamps, process RSS or ordinary solver claim",
        },
    )?;
    control.poll().map_err(Error::Cpu)?;
    let mut resources = Resources::new(configuration, control, &mut emit)?;
    let routes = if configuration.backend == crate::Backend::Cpu {
        CPU_ROUTES
    } else {
        METAL_ROUTES
    };
    let mut samples = 0;
    for (case_index, &case) in configuration.cases.iter().enumerate() {
        let preparation_start = Instant::now();
        let prepared = checking::prepare(case, configuration, control)?;
        let preparation_ns = preparation_start.elapsed().as_nanos();
        prepared_event(case_index, case, &prepared, preparation_ns, &mut emit)?;
        for (phase, iterations) in [
            (Phase::Initial, 1),
            (Phase::Warmup, configuration.warmups),
            (Phase::Timed, configuration.repetitions.get()),
        ] {
            for iteration in 0..iterations {
                for position in 0..routes.len() {
                    let route = routes[(position + iteration) % routes.len()];
                    let observation = Observation {
                        case_index,
                        phase,
                        iteration,
                        position,
                        route,
                        elapsed_ns: 0,
                        classification_ns: 0,
                        completion_ns: None,
                        activity: Activity::default(),
                    };
                    let sample = observe(
                        &prepared,
                        configuration,
                        &mut resources,
                        control,
                        observation,
                        &mut emit,
                    )?;
                    publish(&mut emit, &Event::Sample { sample: &sample })?;
                    samples += 1;
                }
            }
        }
    }
    // A Sample callback may cancel after that observation was committed. Keep
    // its prefix, but do not assert campaign completion under stopped control.
    control.poll().map_err(Error::Cpu)?;
    publish(&mut emit, &Event::Complete { samples })
}

fn prepared_event(
    case_index: usize,
    case: super::Case,
    prepared: &Prepared,
    preparation_ns: u128,
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    let mut nodes = reserve(prepared.fixture.theory.nodes().len())?;
    nodes.extend(
        prepared
            .fixture
            .theory
            .nodes()
            .iter()
            .copied()
            .map(Formula::from),
    );
    let mut producers = reserve(prepared.plan.producers().len())?;
    producers.extend(prepared.plan.producers().iter().map(|producer| Producer {
        head: producer.head(),
        body: producer.body(),
        root: producer.root(),
        kind: producer.kind().into(),
    }));
    let mut candidates = reserve(prepared.fixture.candidates.len())?;
    for candidate in &prepared.fixture.candidates {
        let mut atoms = reserve(case.atoms.get())?;
        atoms.extend(candidate.atoms());
        candidates.push(atoms);
    }
    let reference = checking::outcomes(&prepared.certificates, &prepared.reference)?;
    publish(
        emit,
        &Event::Prepared {
            case_index,
            case,
            preparation_ns,
            plan_work: prepared.plan.statistics().work,
            plan_dependencies: prepared.plan.statistics().dependencies,
            plan_construction_bytes: prepared.plan.statistics().construction_bytes,
            plan_resident_bytes: prepared.plan.statistics().resident_bytes,
            nodes: &nodes,
            roots: prepared.fixture.theory.roots(),
            producers: &producers,
            ranks: prepared.plan.ranks(),
            candidates: &candidates,
            reference: &reference,
        },
    )
}

fn observe(
    prepared: &Prepared,
    configuration: &Configuration,
    resources: &mut Resources,
    control: &Control,
    mut observation: Observation,
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<Sample, Error> {
    // A dedicated fresh instance cannot invalidate the other route's residency.
    if observation.route == Route::MetalFresh {
        resources
            .fresh
            .as_mut()
            .ok_or(Error::DeviceWork)?
            .clear_residency();
    }
    let start = Instant::now();
    let classification_start = Instant::now();
    let classified = classify(
        prepared,
        configuration,
        resources,
        control,
        &mut observation,
    );
    observation.classification_ns = classification_start.elapsed().as_nanos();
    let completed = classified.and_then(|verdicts| {
        let completion_start = Instant::now();
        let checks = checking::complete(
            prepared,
            &verdicts,
            configuration,
            control,
            &mut observation.activity,
        );
        observation.completion_ns = Some(completion_start.elapsed().as_nanos());
        checks.map(|checks| (verdicts, checks))
    });
    observation.elapsed_ns = start.elapsed().as_nanos();
    let validated = completed.and_then(|(verdicts, checks)| {
        checking::validate(prepared, &verdicts, &checks, configuration, control)?;
        let per_candidate = prepared.fixture.theory.nodes().len()
            + prepared.fixture.theory.roots().len()
            + prepared.plan.producers().len()
            + 2 * prepared.fixture.theory.atom_count();
        validate_device(
            &observation,
            prepared.fixture.candidates.len(),
            per_candidate,
        )?;
        let outcomes = checking::outcomes(&verdicts, &checks)?;
        control.poll().map_err(Error::Cpu)?;
        Ok(outcomes)
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

fn classify(
    prepared: &Prepared,
    configuration: &Configuration,
    resources: &mut Resources,
    control: &Control,
    observation: &mut Observation,
) -> Result<Vec<TightVerdict>, Error> {
    match observation.route {
        Route::Scalar => checking::classify(
            prepared,
            configuration,
            None,
            control,
            &mut observation.activity,
        ),
        Route::Rayon => checking::classify(
            prepared,
            configuration,
            Some(&resources.pool),
            control,
            &mut observation.activity,
        ),
        Route::MetalFresh | Route::MetalResident => {
            let oracle = match observation.route {
                Route::MetalFresh => resources.fresh.as_mut(),
                _ => resources.resident.as_mut(),
            }
            .ok_or(Error::DeviceWork)?;
            let result = oracle.check_batch(
                &prepared.plan,
                &prepared.fixture.candidates,
                configuration.gpu_limits,
                control,
            );
            observation.activity.device = Some(device_work(oracle));
            let checks = result.map_err(Error::Metal)?;
            let mut verdicts = reserve(checks.len())?;
            verdicts.extend(checks.iter().map(zetesis_wgpu::TightGpuCheck::verdict));
            observation.activity.classified = verdicts.len();
            observation.activity.gpu_decisions = verdicts
                .iter()
                .filter(|verdict| !matches!(verdict, TightVerdict::Residual { .. }))
                .count();
            Ok(verdicts)
        }
    }
}

fn device_work(oracle: &GpuTightOracle) -> DeviceWork {
    let activity = oracle.activity();
    DeviceWork {
        submissions: activity.submissions,
        submitted_candidates: activity.submitted_candidates,
        scheduled_work: activity.scheduled_work,
        completed_candidates: activity.completed_candidates,
        completed_work: activity.completed_work,
        uploaded_bytes: activity.uploaded_bytes,
        downloaded_bytes: activity.downloaded_bytes,
        residency: oracle.last_batch_stats().map(|stats| Residency {
            theory_uploaded: stats.theory_uploaded,
            transport_allocated: stats.transport_allocated,
            theory_bytes: stats.resident_theory_bytes,
            transport_bytes: stats.resident_transport_bytes,
            accounted_bytes: stats.accounted_bytes,
            dispatches: stats.dispatches,
            candidates: stats.candidates,
            work: stats.work,
        }),
    }
}

fn validate_device(
    observation: &Observation,
    candidates: usize,
    per_candidate: usize,
) -> Result<(), Error> {
    let Some(device) = observation.activity.device else {
        return if matches!(observation.route, Route::Scalar | Route::Rayon) {
            Ok(())
        } else {
            Err(Error::DeviceWork)
        };
    };
    let expected = u64::try_from(candidates).map_err(|_| Error::DeviceWork)?;
    if matches!(observation.route, Route::Scalar | Route::Rayon) {
        return Err(Error::DeviceWork);
    }
    let work = u64::try_from(per_candidate)
        .map_err(|_| Error::DeviceWork)?
        .checked_mul(expected)
        .ok_or(Error::DeviceWork)?;
    let residency = device.residency.ok_or(Error::DeviceWork)?;
    if device.submissions != 1
        || device.submitted_candidates != expected
        || device.completed_candidates != expected
        || device.completed_work != work
        || residency.dispatches != device.submissions
        || residency.candidates != device.completed_candidates
        || residency.work != device.completed_work
        || device.scheduled_work != device.completed_work
        || device.uploaded_bytes == 0
        || device.downloaded_bytes == 0
        || (observation.route == Route::MetalFresh
            && (!residency.theory_uploaded || !residency.transport_allocated))
        || (observation.route == Route::MetalResident
            && observation.phase != Phase::Initial
            && (residency.theory_uploaded || residency.transport_allocated))
    {
        return Err(Error::DeviceWork);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/tight/device_work.rs"]
mod tests;
