use crate::{Backend, Family, fixtures};
use clap::Parser;
use std::fmt;
use std::hint::black_box;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use zetesis_core::{GroundProgram, Seed};
use zetesis_cpu::{BatchOracle, Cancellation, Limits, StaticCheck, check_static};
use zetesis_wgpu::{GpuCheck, GpuLimits, GpuOptions, GpuOracle, MAX_ATOMS};

/// Explicitly bounded experiment dimensions. All timings use a monotonic clock.
#[derive(Clone, Debug, Parser)]
#[command(
    name = "zetesis-bench",
    about = "Exact static-oracle parity and CPU/GPU measurements"
)]
pub struct Options {
    /// Backend qualification to run.
    #[arg(long, value_enum, default_value_t)]
    pub backend: Backend,
    /// Positive consequence counts; eight frozen gate atoms are added.
    #[arg(long, value_delimiter = ',', default_value = "64,256")]
    pub atoms: Vec<NonZeroUsize>,
    /// Worlds per batch; values above 256 deliberately repeat seed patterns.
    #[arg(long, value_delimiter = ',', default_value = "1,64,256")]
    pub batches: Vec<NonZeroUsize>,
    /// Deterministic rule-shape families.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "forward-chain,reverse-chain,wide"
    )]
    pub families: Vec<Family>,
    /// Warm samples per backend and fixture.
    #[arg(long, default_value = "5")]
    pub repetitions: NonZeroUsize,
    /// CPU baseline pool threads.
    #[arg(long, default_value = "4")]
    pub workers: NonZeroUsize,
    /// Logical operations allowed per dense CPU candidate.
    #[arg(long, default_value_t = 100_000_000)]
    pub max_work: u64,
    /// Accounted resident and transport bytes admitted by the GPU backend.
    #[arg(long, default_value_t = 134_217_728)]
    pub max_batch_bytes: u64,
}

/// A failed experiment. No timing is accepted as a successful hardware result
/// after parity, resource, device, or output failure.
#[derive(Debug)]
pub enum BenchmarkError {
    /// Dimensions fall outside the experiment's explicit safety/capacity bounds.
    Dimensions,
    /// Fixture admission failed.
    Admission(zetesis_core::AdmissionError),
    /// Explicit static compilation failed.
    Static(zetesis_core::StaticError),
    /// CPU worker-pool or batch admission failed.
    Batch(zetesis_cpu::BatchError),
    /// CPU logical work was incomplete.
    Stop(zetesis_cpu::Stop),
    /// Selected physical adapter, command, capacity or transport failure.
    Gpu(zetesis_wgpu::GpuError),
    /// A backend returned a different result for an identical frozen candidate.
    Parity,
    /// A warm dispatch unexpectedly replaced the resident graph or transport.
    Residency,
    /// Benchmark output failed.
    Output(io::Error),
}
impl fmt::Display for BenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dimensions => f.write_str("require 1..4088 consequence atoms, 1..4096 worlds, at most 100 repetitions and 64 workers"),
            Self::Admission(error) => error.fmt(f), Self::Static(error) => error.fmt(f),
            Self::Batch(error) => error.fmt(f), Self::Stop(error) => error.fmt(f),
            Self::Gpu(error) => error.fmt(f), Self::Parity => f.write_str("backend result mismatch"),
            Self::Residency => f.write_str("warm dispatch did not reuse resident buffers"),
            Self::Output(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for BenchmarkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Static(e) => Some(e),
            Self::Batch(e) => Some(e),
            Self::Stop(e) => Some(e),
            Self::Gpu(e) => Some(e),
            Self::Output(e) => Some(e),
            Self::Dimensions | Self::Parity | Self::Residency => None,
        }
    }
}
impl From<io::Error> for BenchmarkError {
    fn from(error: io::Error) -> Self {
        Self::Output(error)
    }
}

fn validate(options: &Options) -> Result<(), BenchmarkError> {
    if options.atoms.is_empty()
        || options.batches.is_empty()
        || options.families.is_empty()
        || options.atoms.iter().any(|n| n.get() > MAX_ATOMS - 8)
        || options.batches.iter().any(|n| n.get() > 4096)
        || options.repetitions.get() > 100
        || options.workers.get() > 64
    {
        return Err(BenchmarkError::Dimensions);
    }
    Ok(())
}

fn scalar(
    graph: &GroundProgram,
    seeds: &[Seed],
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<Vec<StaticCheck>, BenchmarkError> {
    seeds
        .iter()
        .map(|seed| check_static(graph, seed, limits, cancellation).map_err(BenchmarkError::Stop))
        .collect()
}
fn parallel(
    pool: &BatchOracle,
    graph: &GroundProgram,
    seeds: &[Seed],
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<Vec<StaticCheck>, BenchmarkError> {
    pool.check_static_batch(graph, seeds, limits, cancellation)
        .map_err(BenchmarkError::Batch)?
        .into_iter()
        .map(|result| result.map_err(BenchmarkError::Stop))
        .collect()
}
fn cpu_parity(expected: &[StaticCheck], actual: &[StaticCheck]) -> Result<(), BenchmarkError> {
    if expected.len() != actual.len()
        || expected.iter().zip(actual).any(|(a, b)| {
            a.closure_words() != b.closure_words()
                || a.constraint_violated() != b.constraint_violated()
                || a.seed_mismatch() != b.seed_mismatch()
        })
    {
        return Err(BenchmarkError::Parity);
    }
    Ok(())
}
fn gpu_parity(expected: &[StaticCheck], actual: &[GpuCheck]) -> Result<(), BenchmarkError> {
    if expected.len() != actual.len()
        || expected.iter().zip(actual).any(|(a, b)| {
            a.closure_words() != b.closure_words()
                || a.constraint_violated() != b.constraint_violated()
                || a.seed_mismatch() != b.seed_mismatch()
        })
    {
        return Err(BenchmarkError::Parity);
    }
    Ok(())
}

struct Row<'a> {
    family: Family,
    atoms: usize,
    rules: usize,
    batch: usize,
    backend: &'a str,
}
impl Row<'_> {
    fn emit(
        &self,
        output: &mut impl Write,
        phase: &str,
        iteration: usize,
        duration: Duration,
    ) -> io::Result<()> {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.family.label(),
            self.atoms,
            self.rules,
            self.batch,
            self.backend,
            phase,
            iteration,
            duration.as_nanos()
        )
    }
}

/// Require the selected physical API, compare all results, and write TSV timings.
/// Fixture/static compilation, pool/device initialization and initial-case dispatch are
/// reported separately. Warm dispatch timings include host packing and readback;
/// correctness comparison is outside each timed region. This is an oracle
/// microbenchmark, not a full solve or energy benchmark.
///
/// # Errors
/// Returns typed refusal on invalid dimensions, unavailable hardware, incomplete
/// CPU work, backend mismatch, or output failure. No fallback is performed.
pub fn run(options: &Options, output: &mut impl Write) -> Result<(), BenchmarkError> {
    validate(options)?;
    writeln!(
        output,
        "# zetesis_version={} target_os={} target_arch={} debug_assertions={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        cfg!(debug_assertions)
    )?;
    writeln!(
        output,
        "# timing_order=cpu-scalar,cpu-rayon{} scope=static-oracle-host-through-readback",
        match options.backend {
            Backend::Cpu => "",
            Backend::Metal => ",metal",
            Backend::Vulkan => ",vulkan",
        }
    )?;
    let started = Instant::now();
    let mut gpu = options
        .backend
        .selection()
        .map(|selection| GpuOracle::new_selected(GpuOptions::default(), selection))
        .transpose()
        .map_err(BenchmarkError::Gpu)?;
    if let Some(oracle) = &gpu {
        writeln!(
            output,
            "# adapter={} backend={} type={} device_init_ns={}",
            oracle.info().name(),
            oracle.info().backend(),
            oracle.info().device_type(),
            started.elapsed().as_nanos()
        )?;
    } else {
        writeln!(output, "# backend=cpu GPU_execution=not_requested")?;
    }
    let maximum = options
        .batches
        .iter()
        .copied()
        .max()
        .ok_or(BenchmarkError::Dimensions)?;
    let started = Instant::now();
    let pool = BatchOracle::new(options.workers, maximum).map_err(BenchmarkError::Batch)?;
    writeln!(
        output,
        "# workers={} pool_init_ns={}",
        options.workers,
        started.elapsed().as_nanos()
    )?;
    writeln!(
        output,
        "family\tatoms\trules\tworlds\tbackend\tphase\titeration\telapsed_ns"
    )?;
    for family in &options.families {
        for size in &options.atoms {
            let started = Instant::now();
            let graph = fixtures::fixture(*family, size.get())?;
            writeln!(
                output,
                "# fixture={} consequences={} build_and_static_compile_ns={}",
                family.label(),
                size,
                started.elapsed().as_nanos()
            )?;
            let limits = Limits {
                max_work: options.max_work,
                max_derived_atoms: graph.atom_count(),
                ..Limits::default()
            };
            let cancellation = Cancellation::default();
            for batch in &options.batches {
                Case {
                    options,
                    pool: &pool,
                    graph: &graph,
                    family: *family,
                    batch: batch.get(),
                    limits,
                    cancellation: &cancellation,
                }
                .measure(output, &mut gpu)?;
            }
        }
    }
    writeln!(
        output,
        "# status=PASS every_measured_result_matched_dense_CPU=true"
    )?;
    Ok(())
}

struct Case<'a> {
    options: &'a Options,
    pool: &'a BatchOracle,
    graph: &'a GroundProgram,
    family: Family,
    batch: usize,
    limits: Limits,
    cancellation: &'a Cancellation,
}

impl Case<'_> {
    fn measure(
        &self,
        output: &mut impl Write,
        gpu: &mut Option<GpuOracle>,
    ) -> Result<(), BenchmarkError> {
        let Self {
            options,
            pool,
            graph,
            family,
            batch,
            limits,
            cancellation,
        } = *self;
        let seeds = fixtures::seeds(graph, batch, 0);
        let expected = scalar(graph, &seeds, limits, cancellation)?;
        let mut row = Row {
            family,
            atoms: graph.atom_count(),
            rules: graph.rules().len(),
            batch,
            backend: options.backend.label(),
        };
        let gpu_limits = GpuLimits {
            max_candidates: batch,
            max_batch_bytes: options.max_batch_bytes,
            ..Default::default()
        };
        if let Some(oracle) = gpu {
            let started = Instant::now();
            let actual = oracle
                .check_batch(graph, &seeds, gpu_limits)
                .map_err(BenchmarkError::Gpu)?;
            let elapsed = started.elapsed();
            gpu_parity(&expected, &actual)?;
            row.emit(output, "initial-case", 0, elapsed)?;
            if let Some(stats) = oracle.last_batch_stats() {
                writeln!(output, "# residency {stats:?}")?;
            }
        }
        // Warm all CPU paths before timing; the first frozen batch already warmed the selected device.
        cpu_parity(
            &expected,
            &parallel(pool, graph, &seeds, limits, cancellation)?,
        )?;
        for repetition in 0..options.repetitions.get() {
            let seeds = fixtures::seeds(graph, batch, repetition + 1);
            let started = Instant::now();
            let expected = scalar(black_box(graph), black_box(&seeds), limits, cancellation)?;
            let elapsed = started.elapsed();
            row.backend = "cpu-scalar";
            row.emit(output, "warm", repetition, elapsed)?;
            let started = Instant::now();
            let actual = parallel(
                pool,
                black_box(graph),
                black_box(&seeds),
                limits,
                cancellation,
            )?;
            let elapsed = started.elapsed();
            cpu_parity(&expected, &actual)?;
            row.backend = "cpu-rayon";
            row.emit(output, "warm", repetition, elapsed)?;
            if let Some(oracle) = gpu {
                let started = Instant::now();
                let actual = oracle
                    .check_batch(black_box(graph), black_box(&seeds), gpu_limits)
                    .map_err(BenchmarkError::Gpu)?;
                let elapsed = started.elapsed();
                gpu_parity(&expected, &actual)?;
                if !oracle
                    .last_batch_stats()
                    .is_some_and(|stats| !stats.graph_uploaded && !stats.transport_allocated)
                {
                    return Err(BenchmarkError::Residency);
                }
                row.backend = options.backend.label();
                row.emit(output, "warm", repetition, elapsed)?;
                if let Some(stats) = oracle.last_batch_stats() {
                    writeln!(output, "# residency {stats:?}")?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/support/parity_contracts.rs"]
mod contract_tests;
