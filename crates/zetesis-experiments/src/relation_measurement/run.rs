use std::{hint::black_box, io, mem::size_of, time::Instant};

use rayon::prelude::*;
use zetesis_core::relation::{Limits, Mask, Query, Relation, Selection};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_wgpu::{
    GpuOptions, GpuRelationExecutor, PreparedGpuRelation, RelationGpuLimits, RelationGpuMasks,
};

use super::{
    Configuration, Error, Event, Observation, Phase, Preparation, Route, Subject,
    view::{self, DeviceWork},
};
use crate::{
    Backend,
    relation_fixtures::{Fixture, Limits as FixtureLimits},
};

/// Measure one retained relation with a common packed-output contract.
///
/// Each route receives every original row and query occurrence. Scalar and
/// Rayon invoke `Relation::select_mask`, copy its words, and drop each temporary
/// mask. Rayon owns disjoint query outputs and has at most its worker count of
/// temporary masks live. GPU masks are copied to that same layout.
/// Every batch is checked against the independent typed-row reference before
/// publication, then the same core mask decoder reconstructs its typed rows.
///
/// Construction, lookup, input selection, pool, pipeline and upload intervals
/// are reported separately. Each route has one initial sample plus the requested
/// warmups and repetitions. Validation, subject hashing and event publication
/// are outside the operation intervals. Authored simultaneous bounds are
/// conservative; they exclude driver storage, allocator metadata and Rayon
/// worker stacks. Routes run scalar, Rayon, then GPU; validation warms the rows
/// before timed reconstruction. CPU routes retain uploaded columns when a
/// physical backend is selected. This measures no full pattern matching,
/// grounding or solving.
///
/// # Errors
/// Refuses configuration/capacity, allocation, control, device, parity and sink
/// failures. A published prefix has no completion claim without Complete.
pub fn measure(
    configuration: Configuration,
    cancellation: &Cancellation,
    mut emit: impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    let configuration = configuration.validate()?;
    poll(cancellation)?;
    emit(&Event::Start {
        schema: 2,
        configuration,
    })
    .map_err(Error::Output)?;
    let started = Instant::now();
    let fixture = Fixture::new(
        configuration.family,
        configuration.payload,
        configuration.rows,
        configuration.queries,
        FixtureLimits {
            max_bytes: configuration.max_bytes,
        },
    )
    .map_err(Error::Fixture)?;
    let mut preparation = Preparation {
        fixture_ns: started.elapsed().as_nanos(),
        ..Preparation::default()
    };
    let fixture_bytes = fixture.storage().retained_bytes;
    let started = Instant::now();
    let relation = Relation::from_atoms(
        fixture.predicate(),
        fixture.atoms(),
        limits(configuration.max_bytes, fixture_bytes)?,
    )
    .map_err(Error::Relation)?;
    preparation.relation_ns = started.elapsed().as_nanos();
    preparation.construction_peak_bytes = fixture
        .storage()
        .peak_construction_bytes
        .max(fixture_bytes + relation.storage().peak_construction_bytes);
    let started = Instant::now();
    let (queries, query_bytes, query_peak) = queries(&fixture, &relation, configuration.max_bytes)?;
    preparation.query_ns = started.elapsed().as_nanos();
    preparation.construction_peak_bytes = preparation.construction_peak_bytes.max(query_peak);
    let external = fixture_bytes + query_bytes;
    let started = Instant::now();
    let input = relation
        .all(limits(configuration.max_bytes, external)?)
        .map_err(Error::Relation)?;
    preparation.input_ns = started.elapsed().as_nanos();
    preparation.shared_bytes =
        external + relation.storage().retained_bytes + input.retained_bytes();
    preparation.construction_peak_bytes = preparation
        .construction_peak_bytes
        .max(preparation.shared_bytes);
    let started = Instant::now();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(configuration.workers)
        .build()
        .map_err(Error::Pool)?;
    preparation.pool_ns = started.elapsed().as_nanos();
    let mut executor = device(configuration, &mut preparation)?;
    let mut prepared = if let Some(executor) = executor.as_mut() {
        let started = Instant::now();
        let prepared = executor
            .prepare(
                &relation,
                gpu_limits(configuration.max_bytes, preparation.shared_bytes)?,
                cancellation,
            )
            .map_err(Error::Gpu)?;
        preparation.upload_ns = Some(started.elapsed().as_nanos());
        preparation.column_bytes = prepared.column_bytes();
        Some(prepared)
    } else {
        None
    };
    let frame = Frame {
        fixture: &fixture,
        relation: &relation,
        queries: &queries,
        input: &input,
        configuration,
        shared_bytes: preparation.shared_bytes,
        column_bytes: usize::try_from(preparation.column_bytes)
            .map_err(|_| Error::Configuration("column bytes exceed host"))?,
    };
    frame.capacity(configuration.workers.min(configuration.queries))?;
    let identity = view::fingerprint(&fixture)?;
    emit(&Event::Subject {
        sha256: &identity,
        subject: Subject(&fixture),
        preparation,
        adapter: prepared.as_ref().map(|prepared| prepared.info().name()),
    })
    .map_err(Error::Output)?;
    schedule(
        &frame,
        &pool,
        prepared.as_mut(),
        &identity,
        cancellation,
        &mut emit,
    )
}

fn device(
    configuration: Configuration,
    preparation: &mut Preparation,
) -> Result<Option<GpuRelationExecutor>, Error> {
    let executor = if let Some(selection) = configuration.backend.selection() {
        let started = Instant::now();
        let executor = GpuRelationExecutor::new_selected(GpuOptions::default(), selection)
            .map_err(Error::Gpu)?;
        if !executor.info().is_hardware_gpu()
            || !executor
                .info()
                .backend()
                .eq_ignore_ascii_case(configuration.backend.label())
        {
            return Err(Error::DeviceWork);
        }
        preparation.device_ns = Some(started.elapsed().as_nanos());
        Some(executor)
    } else {
        None
    };
    Ok(executor)
}

fn schedule<'owner, 'source>(
    frame: &Frame<'owner, 'source>,
    pool: &rayon::ThreadPool,
    mut prepared: Option<&mut PreparedGpuRelation<'_, 'owner, 'source>>,
    identity: &str,
    cancellation: &Cancellation,
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    let mut observations = 0;
    for (phase, repetitions) in [
        (Phase::Initial, 1),
        (Phase::Warmup, frame.configuration.warmups),
        (Phase::Timed, frame.configuration.repetitions),
    ] {
        for repetition in 0..repetitions {
            for route in [Route::Scalar, Route::Rayon] {
                let started = Instant::now();
                let batch = cpu_batch(frame, route, pool, cancellation)?;
                let elapsed = started.elapsed().as_nanos();
                publish(
                    frame,
                    &batch,
                    Sample {
                        route,
                        phase,
                        repetition,
                        elapsed,
                    },
                    identity,
                    cancellation,
                    emit,
                )?;
                observations += 1;
            }
            if let Some(prepared) = prepared.as_mut() {
                let started = Instant::now();
                let batch = gpu_batch(frame, prepared, cancellation)?;
                let elapsed = started.elapsed().as_nanos();
                let route = match frame.configuration.backend {
                    Backend::Metal => Route::Metal,
                    Backend::Vulkan => Route::Vulkan,
                    Backend::Cpu => return Err(Error::DeviceWork),
                };
                publish(
                    frame,
                    &batch,
                    Sample {
                        route,
                        phase,
                        repetition,
                        elapsed,
                    },
                    identity,
                    cancellation,
                    emit,
                )?;
                observations += 1;
            }
        }
    }
    poll(cancellation)?;
    emit(&Event::Complete { observations }).map_err(Error::Output)
}

fn queries<'owner, 'source>(
    fixture: &Fixture,
    relation: &'owner Relation<'source>,
    maximum: usize,
) -> Result<(Vec<Query<'owner, 'source>>, usize, usize), Error> {
    let mut bytes =
        size_of::<Vec<Query<'_, '_>>>() + fixture.queries().len() * size_of::<Query<'_, '_>>();
    let base = fixture.storage().retained_bytes + relation.storage().retained_bytes;
    limits(maximum, base + bytes)?;
    let mut peak = base + bytes;
    let mut queries = vector(fixture.queries().len())?;
    for keys in fixture.queries() {
        let temporary = size_of::<Vec<(usize, &zetesis_core::Value)>>()
            + keys.len() * size_of::<(usize, &zetesis_core::Value)>();
        limits(maximum, base + bytes + temporary)?;
        let mut references = vector(keys.len())?;
        references.extend(keys.iter().map(|(column, value)| (*column, value)));
        let query = relation
            .query(
                &references,
                limits(
                    maximum,
                    fixture.storage().retained_bytes + bytes + temporary,
                )?,
            )
            .map_err(Error::Relation)?;
        peak = peak.max(base + bytes + temporary + query.retained_bytes());
        bytes = bytes
            .checked_add(query.retained_bytes() - size_of::<Query<'_, '_>>())
            .ok_or(Error::Configuration("query byte sum overflow"))?;
        queries.push(query);
    }
    Ok((queries, bytes, peak))
}

struct Frame<'owner, 'source> {
    fixture: &'source Fixture,
    relation: &'owner Relation<'source>,
    queries: &'owner [Query<'owner, 'source>],
    input: &'owner Selection<'owner, 'source>,
    configuration: Configuration,
    shared_bytes: usize,
    column_bytes: usize,
}

impl Frame<'_, '_> {
    fn words(&self) -> usize {
        self.configuration.rows.div_ceil(32)
    }
    fn mask_bytes(&self) -> usize {
        size_of::<Vec<u32>>() + self.words() * self.configuration.queries * size_of::<u32>()
    }
    fn capacity(&self, workers: usize) -> Result<usize, Error> {
        let positions =
            size_of::<Selection<'_, '_>>() + self.configuration.rows * size_of::<usize>();
        let mask = size_of::<Mask<'_, '_>>() + self.words() * size_of::<u32>();
        // CPU workers retain direct masks. Reference validation retains at most
        // two position vectors later, after those temporary masks are dropped.
        let temporaries = if self.queries.is_empty() {
            0
        } else {
            (workers * mask).max(2 * positions)
        };
        let metrics = size_of::<Vec<u128>>() + self.configuration.queries * size_of::<u128>();
        let total =
            self.shared_bytes + self.column_bytes + self.mask_bytes() + temporaries + metrics;
        if total > self.configuration.max_bytes {
            return Err(Error::Configuration(
                "simultaneous relation route exceeds authored byte ceiling",
            ));
        }
        Ok(total)
    }

    fn decoding_limits(&self) -> Limits {
        Limits {
            max_bytes: self.relation.storage().retained_bytes
                + self.words() * size_of::<u32>()
                + size_of::<Selection<'_, '_>>()
                + self.configuration.rows * size_of::<usize>(),
            ..Limits::default()
        }
    }
}

struct Batch {
    masks: Vec<u32>,
    work: u128,
    peak: usize,
    device: Option<DeviceWork>,
}
#[derive(Clone, Copy)]
struct Sample {
    route: Route,
    phase: Phase,
    repetition: usize,
    elapsed: u128,
}

fn cpu_batch(
    frame: &Frame<'_, '_>,
    route: Route,
    pool: &rayon::ThreadPool,
    cancellation: &Cancellation,
) -> Result<Batch, Error> {
    poll(cancellation)?;
    let workers = if route == Route::Rayon {
        frame.configuration.workers.min(frame.queries.len())
    } else {
        1
    };
    let peak = frame.capacity(workers)?;
    let mut masks = vector(frame.words() * frame.queries.len())?;
    masks.resize(frame.words() * frame.queries.len(), 0u32);
    let mut work = vector(frame.queries.len())?;
    work.resize(frame.queries.len(), 0u128);
    let select =
        |((query, words), work): ((&Query<'_, '_>, &mut [u32]), &mut u128)| -> Result<(), Error> {
            poll(cancellation)?;
            let maximum = frame.relation.storage().retained_bytes
                + query.retained_bytes()
                + frame.input.retained_bytes()
                + size_of::<Mask<'_, '_>>()
                + frame.words() * size_of::<u32>();
            let selected = frame
                .relation
                .select_mask(
                    query,
                    frame.input,
                    Limits {
                        max_bytes: maximum,
                        ..Limits::default()
                    },
                )
                .map_err(Error::Relation)?;
            *work = selected.work();
            words.copy_from_slice(selected.words());
            Ok(())
        };
    if route == Route::Rayon {
        pool.install(|| {
            frame
                .queries
                .par_iter()
                .zip(masks.par_chunks_mut(frame.words()))
                .zip(work.par_iter_mut())
                .try_for_each(select)
        })?;
    } else {
        for inputs in frame
            .queries
            .iter()
            .zip(masks.chunks_mut(frame.words()))
            .zip(&mut work)
        {
            select(inputs)?;
        }
    }
    Ok(Batch {
        masks,
        work: work.into_iter().sum(),
        peak,
        device: None,
    })
}

fn gpu_batch<'owner, 'source>(
    frame: &Frame<'owner, 'source>,
    prepared: &mut PreparedGpuRelation<'_, 'owner, 'source>,
    cancellation: &Cancellation,
) -> Result<Batch, Error> {
    poll(cancellation)?;
    let reconstruction = frame.capacity(0)?;
    let mut masks = vector(frame.words() * frame.queries.len())?;
    let limits = gpu_limits(
        frame.configuration.max_bytes,
        frame.shared_bytes + frame.mask_bytes() + size_of::<RelationGpuMasks<'_, '_>>(),
    )?;
    let output = prepared
        .filter(frame.queries, limits, cancellation)
        .map_err(Error::Gpu)?;
    if output.query_count() != frame.queries.len() || !frame.relation.same_owner(output.relation())
    {
        return Err(Error::Parity);
    }
    for query in 0..frame.queries.len() {
        masks.extend_from_slice(output.words(query).ok_or(Error::Parity)?);
    }
    let activity = prepared.activity();
    let stats = prepared.last_stats().ok_or(Error::DeviceWork)?;
    let submitted = u64::from(!frame.queries.is_empty());
    if activity.submissions != submitted
        || activity.submitted_queries != frame.queries.len() as u64
        || activity.completed_queries != activity.submitted_queries
        || activity.completed_work != activity.scheduled_work
        || activity.submitted_workgroups
            != frame.configuration.rows.div_ceil(64) as u64 * frame.queries.len() as u64
        || (submitted != 0 && activity.downloaded_bytes == 0)
    {
        return Err(Error::DeviceWork);
    }
    let total = frame.shared_bytes
        + frame.mask_bytes()
        + size_of::<RelationGpuMasks<'_, '_>>()
        + usize::try_from(stats.accounted_bytes)
            .map_err(|_| Error::Configuration("device capacity exceeds host"))?;
    Ok(Batch {
        masks,
        work: 0,
        peak: reconstruction.max(total),
        device: Some(DeviceWork::new(activity, stats)),
    })
}

fn publish(
    frame: &Frame<'_, '_>,
    batch: &Batch,
    sample: Sample,
    identity: &str,
    cancellation: &Cancellation,
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<(), Error> {
    validate(frame, &batch.masks, cancellation)?;
    let started = Instant::now();
    let mut cells = 0;
    for words in batch.masks.chunks(frame.words()) {
        poll(cancellation)?;
        let selection = frame
            .relation
            .selection_from_mask(words, frame.decoding_limits())
            .map_err(Error::Relation)?;
        for index in 0..selection.positions().len() {
            let row = selection.row(index).ok_or(Error::Parity)?;
            for column in 0..frame.relation.predicate().arity() {
                black_box(row.value(column).ok_or(Error::Parity)?);
                cells += 1;
            }
        }
    }
    let reconstruction_ns = started.elapsed().as_nanos();
    let expected_cells = batch
        .masks
        .iter()
        .map(|word| word.count_ones() as usize)
        .sum::<usize>()
        * frame.relation.predicate().arity();
    if cells != expected_cells {
        return Err(Error::Parity);
    }
    emit(&Event::Observation(Observation {
        subject_sha256: identity,
        route: sample.route,
        phase: sample.phase,
        repetition: sample.repetition,
        selection_ns: sample.elapsed,
        reconstruction_ns,
        operation_ns: sample.elapsed + reconstruction_ns,
        cpu_selection_work: batch.work,
        reconstructed_cells: cells,
        peak_bytes: batch.peak,
        rows: frame.configuration.rows,
        queries: frame.queries.len(),
        words_per_query: frame.words(),
        masks: &batch.masks,
        device: batch.device,
    }))
    .map_err(Error::Output)
}

fn validate(
    frame: &Frame<'_, '_>,
    masks: &[u32],
    cancellation: &Cancellation,
) -> Result<(), Error> {
    if masks.len() != frame.words() * frame.queries.len() {
        return Err(Error::Parity);
    }
    for (query, words) in masks.chunks(frame.words()).enumerate() {
        poll(cancellation)?;
        let expected = frame
            .fixture
            .reference_positions(query, frame.configuration.rows)
            .map_err(Error::Fixture)?;
        if expected.capacity() > frame.configuration.rows {
            return Err(Error::Configuration(
                "reference allocation exceeds planned capacity",
            ));
        }
        let count: usize = words.iter().map(|word| word.count_ones() as usize).sum();
        if count != expected.len()
            || expected
                .iter()
                .any(|&row| words[row / 32] & (1 << (row % 32)) == 0)
        {
            return Err(Error::Parity);
        }
        let decoded = frame
            .relation
            .selection_from_mask(words, frame.decoding_limits())
            .map_err(Error::Relation)?;
        if decoded.positions() != expected {
            return Err(Error::Parity);
        }
        for (index, &position) in expected.iter().enumerate() {
            let row = decoded.row(index).ok_or(Error::Parity)?;
            if row.source_index() != position {
                return Err(Error::Parity);
            }
            for (column, value) in frame.fixture.atoms()[position].values().iter().enumerate() {
                if row.value(column) != Some(value) {
                    return Err(Error::Parity);
                }
            }
        }
    }
    Ok(())
}

fn limits(maximum: usize, external: usize) -> Result<Limits, Error> {
    Ok(Limits {
        max_bytes: maximum.checked_sub(external).ok_or(Error::Configuration(
            "enclosing relation storage exceeds byte ceiling",
        ))?,
        ..Limits::default()
    })
}
fn gpu_limits(maximum: usize, external: usize) -> Result<RelationGpuLimits, Error> {
    Ok(RelationGpuLimits {
        max_bytes: limits(maximum, external)?.max_bytes as u64,
        ..RelationGpuLimits::default()
    })
}
fn vector<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::Stopped(Stop::Allocation))?;
    if values.capacity() > count {
        return Err(Error::Configuration("allocation exceeds planned capacity"));
    }
    Ok(values)
}
fn poll(cancellation: &Cancellation) -> Result<(), Error> {
    cancellation.poll().map_err(Error::Stopped)
}

#[cfg(test)]
#[path = "../../tests/relation_measurement/validation.rs"]
mod tests;
