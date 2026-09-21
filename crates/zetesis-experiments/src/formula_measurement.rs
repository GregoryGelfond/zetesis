//! Exact membership qualification; outer search and source grounding are excluded.

use std::fmt;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use clap::Args;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_wgpu::{FormulaLimits, GpuFormulaOracle, GpuOptions};

use crate::formula_completion::{Membership, complete_residuals, native, verify, verify_residency};
use crate::formula_fixtures::reserve;
use crate::formula_parallel::FormulaPool;
use crate::{Backend, FormulaFamily, FormulaFixture};

mod projection;
pub use projection::run_formula_projection;

/// Resident general-reduct propagation followed by exact CPU residual checking.
/// This experiment does not enumerate source answer sets or measure a full solve.
#[derive(Clone, Debug, Args)]
pub struct FormulaOptions {
    /// Physical Metal/Vulkan or CPU baselines; projection requires a physical GPU.
    #[arg(long, value_enum, default_value_t)]
    pub backend: Backend,
    /// Semantic atom counts for the synthetic, unrewritten original theories.
    #[arg(long, value_delimiter = ',', default_value = "64,256")]
    pub atoms: Vec<NonZeroUsize>,
    /// Frozen candidates per batch; patterns deliberately repeat above 256.
    #[arg(long, value_delimiter = ',', default_value = "1,64,256")]
    pub batches: Vec<NonZeroUsize>,
    /// Independent formula families, including nonminimal and nonmodel probes.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "choices,cycle,conjunction,disjunction,masked-implication"
    )]
    pub families: Vec<FormulaFamily>,
    /// Warm membership samples; initial dispatch and setup are reported separately.
    #[arg(long, default_value = "5")]
    pub repetitions: NonZeroUsize,
    /// Dedicated Rayon workers, 1..=64; scalar and hybrid residual checks use one.
    #[arg(long, default_value = "4")]
    pub cpu_workers: NonZeroUsize,
    /// Exact CPU search/verification work ceiling per candidate.
    #[arg(long, default_value_t = 100_000_000)]
    pub max_work: u64,
    /// Accounted device, transport and authored packing bytes per GPU batch.
    #[arg(long, default_value_t = 134_217_728)]
    pub max_batch_bytes: u64,
    /// Complete GPU propagation sweeps before returning an explicit residual.
    #[arg(long, default_value_t = 64)]
    pub gpu_max_rounds: u32,
    /// Charged GPU setup and sweep work per frozen candidate.
    #[arg(long, default_value_t = 100_000_000)]
    pub gpu_max_work: u32,
}

/// An experiment failure, distinct from an unresolved GPU query.
#[derive(Debug)]
pub enum FormulaBenchmarkError {
    /// A paired projection experiment requires explicit physical GPU execution.
    ProjectionRequiresGpu,
    /// The two selected adapters reported different identifying metadata.
    AdapterMismatch,
    /// The requested fixture or measurement dimensions were refused.
    Dimensions,
    /// Fallible host storage reservation failed.
    Allocation,
    /// The explicitly sized Rayon worker pool could not be created.
    Pool(rayon::ThreadPoolBuildError),
    /// A formula or interpretation could not be admitted.
    Admission(zetesis_ferraris::AdmissionError),
    /// Exact CPU membership could not finish within its declared resources.
    Incomplete(zetesis_sat::Incomplete),
    /// The required GPU was unavailable or device execution failed.
    Gpu(zetesis_wgpu::GpuError),
    /// A parallel CPU or completed hybrid result disagreed with scalar membership.
    Parity,
    /// A warm batch unexpectedly uploaded the theory or allocated transport.
    Residency,
    /// A sample or diagnostic could not be written completely.
    Output(io::Error),
}

impl fmt::Display for FormulaBenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectionRequiresGpu => f.write_str("formula-projection requires --backend metal or vulkan"),
            Self::AdapterMismatch => f.write_str("projection adapters report different metadata"),
            Self::Dimensions => f.write_str(
                "require nonempty families, 1..4096 atoms/worlds, 1..100 repetitions, and 1..64 CPU workers",
            ),
            Self::Allocation => f.write_str("formula experiment storage reservation failed"),
            Self::Pool(error) => error.fmt(f),
            Self::Admission(error) => error.fmt(f),
            Self::Incomplete(error) => error.fmt(f),
            Self::Gpu(error) => error.fmt(f),
            Self::Parity => f.write_str("formula membership result mismatch"),
            Self::Residency => {
                f.write_str("warm formula batch did not reuse resident theory and transport")
            }
            Self::Output(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for FormulaBenchmarkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pool(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Incomplete(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Dimensions
            | Self::Allocation
            | Self::Parity
            | Self::Residency
            | Self::ProjectionRequiresGpu
            | Self::AdapterMismatch => None,
        }
    }
}
impl From<io::Error> for FormulaBenchmarkError {
    fn from(error: io::Error) -> Self {
        Self::Output(error)
    }
}

fn validate(options: &FormulaOptions) -> Result<(), FormulaBenchmarkError> {
    if options.atoms.is_empty()
        || options.batches.is_empty()
        || options.families.is_empty()
        || options.atoms.iter().any(|n| n.get() > 4096)
        || options.batches.iter().any(|n| n.get() > 4096)
        || options.repetitions.get() > 100
        || options.cpu_workers.get() > 64
    {
        return Err(FormulaBenchmarkError::Dimensions);
    }
    Ok(())
}

/// Run explicit GPU-assisted membership and scalar/parallel CPU baselines.
/// Every device verdict is compared with independent native membership. GPU
/// residuals run that native checker inside the measured hybrid interval;
/// quiescence or a device round/work stop never becomes a stability verdict.
/// Timings exclude source admission and outer candidate generation. All sample
/// order, setup, residency and residual counts remain visible in the output.
/// The dedicated Rayon pool is reused across batches. Its checks use the same
/// per-candidate limits as the scalar path; hybrid residual checks stay serial.
///
/// # Errors
/// Refuses malformed dimensions, incomplete CPU checks, device/transport
/// failures, semantic mismatches or output errors. Explicit physical selection never falls
/// back after a device error, and a failed run has no final success marker.
pub fn run_formula(
    options: &FormulaOptions,
    output: &mut impl Write,
) -> Result<(), FormulaBenchmarkError> {
    validate(options)?;
    writeln!(
        output,
        "# profile=formula-membership source_grounding=excluded outer_search=excluded"
    )?;
    writeln!(
        output,
        "# timing_order=cpu-native,cpu-rayon{} candidate_stream=synthetic cpu_workers={} scalar_workers=1 residual_cpu_workers=1",
        match options.backend {
            Backend::Cpu => "",
            Backend::Metal => ",metal-with-cpu-residuals",
            Backend::Vulkan => ",vulkan-with-cpu-residuals",
        },
        options.cpu_workers
    )?;
    writeln!(
        output,
        "# version={} max_work_per_candidate={} max_batch_bytes={} gpu_max_rounds={} gpu_max_work_per_candidate={}",
        env!("CARGO_PKG_VERSION"),
        options.max_work,
        options.max_batch_bytes,
        options.gpu_max_rounds,
        options.gpu_max_work
    )?;
    let max_candidates = options
        .batches
        .iter()
        .map(|value| value.get())
        .max()
        .ok_or(FormulaBenchmarkError::Dimensions)?;
    let started = Instant::now();
    let pool = FormulaPool::new(options.cpu_workers, max_candidates)?;
    writeln!(
        output,
        "# cpu_pool=rayon requested_workers={} actual_workers={} pool_init_ns={} cpu_work_limit_scope=per_candidate",
        options.cpu_workers,
        pool.workers(),
        started.elapsed().as_nanos()
    )?;
    let started = Instant::now();
    let mut gpu = options
        .backend
        .selection()
        .map(|selection| GpuFormulaOracle::new_selected(GpuOptions::default(), selection))
        .transpose()
        .map_err(FormulaBenchmarkError::Gpu)?;
    if let Some(oracle) = &gpu {
        writeln!(
            output,
            "# adapter={} backend={} device_init_ns={}",
            oracle.info().name(),
            oracle.info().backend(),
            started.elapsed().as_nanos()
        )?;
    } else {
        writeln!(output, "# backend=cpu GPU_execution=not_requested")?;
    }
    writeln!(
        output,
        "family\tatoms\tnodes\tworlds\tbackend\tphase\titeration\telapsed_ns\tdispatch_host_ns\tresidual_cpu_ns\tresiduals"
    )?;
    for &family in &options.families {
        for atoms in &options.atoms {
            let started = Instant::now();
            let fixture = FormulaFixture::new(family, atoms.get())?;
            writeln!(
                output,
                "# family={} fixture_build_ns={}",
                family.label(),
                started.elapsed().as_nanos()
            )?;
            for batch in &options.batches {
                FormulaCase {
                    options,
                    family,
                    fixture: &fixture,
                    batch: batch.get(),
                }
                .measure(output, &mut gpu, &pool)?;
            }
        }
    }
    writeln!(
        output,
        "# status=PASS scope=synthetic-membership all_completed_results_match_native=true"
    )?;
    Ok(())
}

struct FormulaCase<'a> {
    options: &'a FormulaOptions,
    family: FormulaFamily,
    fixture: &'a FormulaFixture,
    batch: usize,
}

impl FormulaCase<'_> {
    fn measure(
        &self,
        output: &mut impl Write,
        gpu: &mut Option<GpuFormulaOracle>,
        pool: &FormulaPool,
    ) -> Result<(), FormulaBenchmarkError> {
        for iteration in 0..=self.options.repetitions.get() {
            let candidates = self.fixture.candidates(self.batch, iteration)?;
            let phase = if iteration == 0 {
                "initial-case"
            } else {
                "warm"
            };
            let expected = self.reference(output, pool, &candidates, (phase, iteration))?;
            if let Some(oracle) = gpu {
                self.hybrid(output, oracle, &candidates, &expected, (phase, iteration))?;
            }
        }
        Ok(())
    }

    fn reference(
        &self,
        output: &mut impl Write,
        pool: &FormulaPool,
        candidates: &[Interpretation],
        sample: (&str, usize),
    ) -> Result<Vec<Membership>, FormulaBenchmarkError> {
        let started = Instant::now();
        let mut expected = reserve(self.batch)?;
        for candidate in candidates {
            expected.push(native(
                self.fixture.theory(),
                candidate,
                self.options.max_work,
            )?);
        }
        self.emit(
            output,
            "cpu-native",
            sample.0,
            sample.1,
            [started.elapsed(), Duration::ZERO, Duration::ZERO],
            0,
        )?;
        let started = Instant::now();
        let parallel = pool.check_batch(
            self.fixture.theory(),
            candidates,
            self.options.max_work,
            &Cancellation::default(),
        )?;
        let elapsed = started.elapsed();
        verify(&parallel, &expected)?;
        self.emit(
            output,
            "cpu-rayon",
            sample.0,
            sample.1,
            [elapsed, Duration::ZERO, Duration::ZERO],
            0,
        )?;
        Ok(expected)
    }

    fn hybrid(
        &self,
        output: &mut impl Write,
        oracle: &mut GpuFormulaOracle,
        candidates: &[Interpretation],
        expected: &[Membership],
        sample: (&str, usize),
    ) -> Result<(), FormulaBenchmarkError> {
        let started = Instant::now();
        let actual = oracle
            .propagate_batch(
                self.fixture.theory(),
                candidates,
                FormulaLimits {
                    max_candidates: self.batch,
                    max_batch_bytes: self.options.max_batch_bytes,
                    max_rounds: self.options.gpu_max_rounds,
                    max_work_per_candidate: self.options.gpu_max_work,
                    ..Default::default()
                },
            )
            .map_err(FormulaBenchmarkError::Gpu)?;
        let device = started.elapsed();
        verify_residency(oracle.last_batch_stats(), sample.1)?;
        let residual_started = Instant::now();
        let (completed, residuals) = complete_residuals(
            self.fixture.theory(),
            candidates,
            actual.iter().map(zetesis_wgpu::FormulaCheck::verdict),
            expected,
            self.options.max_work,
        )?;
        let residual = residual_started.elapsed();
        let elapsed = started.elapsed();
        verify(&completed, expected)?;
        self.emit(
            output,
            &hybrid_label(self.options.backend, oracle.projection()),
            sample.0,
            sample.1,
            [elapsed, device, residual],
            residuals,
        )?;
        writeln!(output, "# residency {:?}", oracle.last_batch_stats())?;
        Ok(())
    }

    fn emit(
        &self,
        output: &mut impl Write,
        backend: &str,
        phase: &str,
        iteration: usize,
        elapsed: [Duration; 3],
        residuals: usize,
    ) -> io::Result<()> {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.family.label(),
            self.fixture.theory().atom_count(),
            self.fixture.theory().nodes().len(),
            self.batch,
            backend,
            phase,
            iteration,
            elapsed[0].as_nanos(),
            elapsed[1].as_nanos(),
            elapsed[2].as_nanos(),
            residuals
        )
    }
}

fn hybrid_label(backend: Backend, projection: zetesis_wgpu::GateProjection) -> String {
    let projection = match projection {
        zetesis_wgpu::GateProjection::Enumerated => "",
        zetesis_wgpu::GateProjection::Bitwise => "-bitwise",
    };
    format!("{}{projection}-with-cpu-residuals", backend.label())
}
