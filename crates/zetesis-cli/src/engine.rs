//! Independent materialization selection and provisional hardware scheduling.

use crate::presentation::{Diagnostics, Label};
use std::io::Write;
use std::sync::Arc;

use zetesis_core::{GroundProgram, Model, Program, Seed, StaticLimits};
use zetesis_cpu::{BatchOracle, Control, Limits, Stop};

use crate::phase_timing::{Recorder, SolvePhase};
use crate::{Backend, Grounder, Oracle, RunError, SolveConfig, SourceBatching};

/// An initial scheduling heuristic, not a measured performance crossover.
pub(crate) const AUTO_GPU_MIN_BATCH: usize = 32;

pub(crate) fn validate_combination(options: &SolveConfig) -> Result<(), RunError> {
    if options.source_batching != SourceBatching::Independent
        && (!matches!(options.backend, Backend::Auto | Backend::Cpu)
            || options.grounder == Grounder::Eager)
    {
        return Err(RunError::UnsupportedSourceBatching);
    }
    if options.oracle == Oracle::Countermodel {
        validate_countermodel(options)?;
    }
    Ok(())
}

pub(crate) fn validate_countermodel(options: &SolveConfig) -> Result<(), RunError> {
    if options.source_batching != SourceBatching::Independent {
        return Err(RunError::UnsupportedSourceBatching);
    }
    if options.grounder == Grounder::Lazy {
        return Err(RunError::UnsupportedOracle {
            backend: options.backend,
            grounder: options.grounder,
        });
    }
    Ok(())
}

pub(crate) struct Engine {
    executor: Executor,
    automatic: bool,
    attempted_gpu: bool,
    // Retain attempted device work if automatic execution resumes on CPU.
    retired_lazy_statistics: Option<crate::LazyExecutionStatistics>,
}

impl Engine {
    pub(crate) fn shared_statistics(&self) -> Option<crate::SharedExecutionStatistics> {
        match &self.executor {
            Executor::SharedCpu { statistics, .. } => Some(statistics.clone()),
            _ => None,
        }
    }
    pub(crate) fn lazy_statistics(
        &self,
        queued_results: usize,
    ) -> Option<crate::LazyExecutionStatistics> {
        let (statistics, owns_queue) = match &self.executor {
            #[cfg(feature = "gpu")]
            Executor::LazyGpu(executor) => (Some(&executor.statistics), true),
            _ => (self.retired_lazy_statistics.as_ref(), false),
        };
        statistics.map(|statistics| {
            let mut statistics = statistics.clone();
            statistics.queued_results = if owns_queue { queued_results } else { 0 };
            statistics
        })
    }
    #[cfg(test)]
    pub(crate) fn new(
        options: &SolveConfig,
        program: &Program,
        diagnostics: &mut Diagnostics<impl Write>,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        Self::with_ground(options, program, None, diagnostics, phases)
    }

    pub(crate) fn with_ground(
        options: &SolveConfig,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        diagnostics: &mut Diagnostics<impl Write>,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        validate_combination(options)?;
        let executor = match options.backend {
            Backend::Auto | Backend::Cpu => {
                let cpu = Executor::cpu(options, program, cached, diagnostics, phases)?;
                if options.backend == Backend::Auto {
                    if options.source_batching != SourceBatching::Independent {
                        diagnostics.metadata(Label::Auto, format_args!("explicit shared source batching selects CPU without device discovery."))?;
                    } else if cfg!(feature = "gpu") {
                        diagnostics.metadata(
                            Label::Auto,
                            format_args!("GPU discovery deferred; the first seed stays CPU. Later batches of at least {AUTO_GPU_MIN_BATCH} candidates may use a physical GPU with {} grounding (provisional heuristic).", grounding_mode(options)),
                        )?;
                    } else {
                        diagnostics.metadata(
                            Label::Auto,
                            format_args!(
                                "GPU support was not compiled; using CPU without device discovery."
                            ),
                        )?;
                    }
                }
                cpu
            }
            _ => Executor::gpu(options, program, cached, diagnostics, phases)?,
        };
        Ok(Self {
            executor,
            automatic: options.backend == Backend::Auto
                && options.source_batching == SourceBatching::Independent
                && cfg!(feature = "gpu"),
            attempted_gpu: false,
            retired_lazy_statistics: None,
        })
    }

    pub(crate) fn check(
        &mut self,
        options: &SolveConfig,
        program: &Program,
        seeds: &[Seed],
        diagnostics: &mut Diagnostics<impl Write>,
        control: &Control,
        phases: &Recorder,
    ) -> Result<Vec<Result<Option<Model>, Stop>>, RunError> {
        if seeds.is_empty() {
            return Ok(Vec::new());
        }
        if let Err(error) = control.poll() {
            return Ok(vec![Err(error)]);
        }
        if should_probe_gpu(self.automatic, self.attempted_gpu, seeds.len()) {
            self.attempted_gpu = true;
            match phases.measure(SolvePhase::ExecutionSetup, || {
                Executor::gpu(
                    options,
                    program,
                    self.executor.ground(),
                    diagnostics,
                    phases,
                )
            }) {
                Ok(gpu) => self.executor = gpu,
                Err(error @ RunError::Output(_)) => return Err(error),
                Err(error) => {
                    diagnostics.metadata(
                        Label::Auto,
                        format_args!(
                            "retaining {} CPU; GPU unavailable: {error}",
                            grounding_mode(options)
                        ),
                    )?;
                }
            }
        }
        let phase = if self.executor.is_gpu() {
            SolvePhase::GpuHostOracle
        } else {
            SolvePhase::ClosureMembership
        };
        match phases.measure(phase, || {
            self.executor.check(options, program, seeds, control)
        }) {
            Err(error @ RunError::LazyStatisticsOverflow) => Err(error),
            Err(error) if self.automatic && self.executor.is_gpu() => {
                // No failed-batch result has been published. Eager retains the
                // same graph; lazy execution retries these same source seeds.
                self.retired_lazy_statistics = self.lazy_statistics(0);
                diagnostics.metadata(
                    Label::Auto,
                    format_args!(
                        "GPU batch failed; retrying on {} CPU: {error}",
                        grounding_mode(options)
                    ),
                )?;
                self.executor = phases.measure(SolvePhase::ExecutionSetup, || {
                    Executor::cpu(
                        options,
                        program,
                        self.executor.ground(),
                        diagnostics,
                        phases,
                    )
                })?;
                phases.measure(SolvePhase::ClosureMembership, || {
                    self.executor.check(options, program, seeds, control)
                })
            }
            result => result,
        }
    }
}

fn should_probe_gpu(automatic: bool, attempted: bool, candidates: usize) -> bool {
    automatic && !attempted && candidates >= AUTO_GPU_MIN_BATCH
}

fn grounding_mode(options: &SolveConfig) -> &'static str {
    if options.grounder == Grounder::Eager {
        "eager"
    } else {
        "lazy"
    }
}

enum Executor {
    Cpu(BatchOracle),
    SharedCpu {
        oracle: BatchOracle,
        statistics: crate::SharedExecutionStatistics,
    },
    StaticCpu {
        oracle: BatchOracle,
        ground: Arc<GroundProgram>,
    },
    #[cfg(feature = "gpu")]
    Gpu {
        oracle: Box<zetesis_wgpu::GpuOracle>,
        ground: Arc<GroundProgram>,
    },
    #[cfg(feature = "gpu")]
    LazyGpu(Box<LazyGpu>),
}

/// Keep device execution and its cumulative observations under one owner.
/// The enum uses one allocation for this state, leaving CPU variants compact.
#[cfg(feature = "gpu")]
struct LazyGpu {
    oracle: zetesis_wgpu::GpuLazyOracle,
    statistics: crate::LazyExecutionStatistics,
}

impl Executor {
    fn cpu(
        options: &SolveConfig,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        diagnostics: &mut Diagnostics<impl Write>,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        let oracle =
            BatchOracle::new(options.workers, options.batch_size).map_err(RunError::Batch)?;
        if options.grounder == Grounder::Eager {
            let ground = match cached {
                Some(ground) => ground,
                None => compile_static(options, program, options.max_atoms, phases)?,
            };
            static_diagnostics(options, &ground, diagnostics)?;
            diagnostics.metadata(
                Label::Backend,
                format_args!(
                    "cpu (eager static closure scans, {} workers)",
                    options.workers
                ),
            )?;
            Ok(Self::StaticCpu { oracle, ground })
        } else {
            phases.lazy_grounding();
            diagnostics.metadata(
                Label::Grounding,
                format_args!(
                    "requested={}, effective=lazy (source joins; no complete ground-rule store)",
                    options.grounder.label()
                ),
            )?;
            if let Some(selection) = options.source_batching.selection() {
                diagnostics.metadata(Label::Backend, format_args!(
                    "cpu (shared {} source rounds, {} workers; collective source and per-world evaluation budgets)",
                    options.source_batching.label(), options.workers))?;
                Ok(Self::SharedCpu {
                    oracle,
                    statistics: crate::SharedExecutionStatistics::new(options, selection),
                })
            } else {
                diagnostics.metadata(
                    Label::Backend,
                    format_args!("cpu (lazy source joins, {} workers)", options.workers),
                )?;
                Ok(Self::Cpu(oracle))
            }
        }
    }

    fn ground(&self) -> Option<Arc<GroundProgram>> {
        match self {
            Self::Cpu(_) | Self::SharedCpu { .. } => None,
            Self::StaticCpu { ground, .. } => Some(Arc::clone(ground)),
            #[cfg(feature = "gpu")]
            Self::Gpu { ground, .. } => Some(Arc::clone(ground)),
            #[cfg(feature = "gpu")]
            Self::LazyGpu(_) => None,
        }
    }

    fn is_gpu(&self) -> bool {
        match self {
            Self::Cpu(_) | Self::SharedCpu { .. } | Self::StaticCpu { .. } => false,
            #[cfg(feature = "gpu")]
            Self::Gpu { .. } | Self::LazyGpu(_) => true,
        }
    }

    #[cfg(not(feature = "gpu"))]
    fn gpu(
        _: &SolveConfig,
        _: &Program,
        _: Option<Arc<GroundProgram>>,
        _: &mut impl Write,
        _: &Recorder,
    ) -> Result<Self, RunError> {
        Err(RunError::BackendUnavailable)
    }

    #[cfg(feature = "gpu")]
    fn gpu(
        options: &SolveConfig,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        diagnostics: &mut Diagnostics<impl Write>,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        use zetesis_wgpu::{GpuOptions, GpuOracle};

        if options.grounder != Grounder::Eager {
            let oracle = zetesis_wgpu::GpuLazyOracle::new_selected(
                GpuOptions::default(),
                selection(options.backend),
            )
            .map_err(RunError::Gpu)?;
            let statistics =
                crate::LazyExecutionStatistics::new(options.backend, oracle.info().metadata());
            phases.lazy_grounding();
            diagnostics.metadata(Label::Grounding, format_args!("requested={}, effective=lazy (host source joins; per-world device consequences; no complete ground-rule store)", options.grounder.label()))?;
            diagnostics.metadata(
                Label::Backend,
                format_args!(
                    "gpu ({}, {}; vendor=0x{:04x}; lazy immutable reduct rounds)",
                    oracle.info().name(),
                    oracle.info().backend(),
                    oracle.info().vendor_id()
                ),
            )?;
            return Ok(Self::LazyGpu(Box::new(LazyGpu { oracle, statistics })));
        }

        // Discover hardware before any new static materialization. Eager CPU
        // attempts already own a graph, shared through Arc without expansion.
        let oracle = GpuOracle::new_selected(GpuOptions::default(), selection(options.backend))
            .map_err(RunError::Gpu)?;
        let atom_limit = options.max_atoms.min(zetesis_wgpu::MAX_ATOMS);
        let ground = match cached {
            Some(ground) => {
                if ground.atom_count() > atom_limit {
                    return Err(RunError::Static(zetesis_core::StaticError::LimitExceeded {
                        resource: "GPU atoms",
                        actual: ground.atom_count(),
                        limit: atom_limit,
                    }));
                }
                ground
            }
            None => compile_static(options, program, atom_limit, phases)?,
        };
        static_diagnostics(options, &ground, diagnostics)?;
        diagnostics.metadata(
            Label::Backend,
            format_args!(
                "gpu ({}, {}; vendor=0x{:04x}; static atoms={}, rules={})",
                oracle.info().name(),
                oracle.info().backend(),
                oracle.info().vendor_id(),
                ground.atom_count(),
                ground.rules().len()
            ),
        )?;
        Ok(Self::Gpu {
            oracle: Box::new(oracle),
            ground,
        })
    }

    fn check(
        &mut self,
        options: &SolveConfig,
        program: &Program,
        seeds: &[Seed],
        control: &Control,
    ) -> Result<Vec<Result<Option<Model>, Stop>>, RunError> {
        let limits = Limits {
            max_work: options.max_work,
            max_derived_atoms: options.max_atoms,
        };
        match self {
            Self::SharedCpu { oracle, statistics } => {
                let source = zetesis_cpu::lazy::Limits {
                    max_candidates: options.batch_size.get(),
                    max_atoms: options.max_atoms,
                    max_source_work: options.max_source_work,
                    max_rounds: u64::try_from(options.max_atoms)
                        .unwrap_or(u64::MAX)
                        .saturating_add(1),
                    max_host_bytes: usize::try_from(options.max_batch_bytes).unwrap_or(usize::MAX),
                    ..Default::default()
                };
                let result = oracle.check_shared(
                    program,
                    seeds,
                    zetesis_cpu::lazy::shared::Limits {
                        source,
                        max_world_work: options.max_work,
                    },
                    statistics.selection,
                    control,
                );
                crate::shared_execution::batch_results(result, statistics)
            }
            #[cfg(feature = "gpu")]
            Self::LazyGpu(executor) => {
                let LazyGpu { oracle, statistics } = executor.as_mut();
                let bytes = usize::try_from(options.max_batch_bytes / 2).unwrap_or(usize::MAX);
                let source_limits = zetesis_cpu::lazy::Limits {
                    max_candidates: options.batch_size.get(),
                    max_atoms: options.max_atoms,
                    max_source_work: options.max_work,
                    max_rounds: u64::try_from(options.max_atoms)
                        .unwrap_or(u64::MAX)
                        .saturating_add(1),
                    max_host_bytes: bytes,
                    ..Default::default()
                };
                let device_limits = zetesis_wgpu::GpuLimits {
                    max_candidates: options.batch_size.get(),
                    max_batch_bytes: options.max_batch_bytes / 2,
                    ..Default::default()
                };
                let result =
                    oracle.check_batch(program, seeds, source_limits, device_limits, control);
                let progress = match &result {
                    Ok(batch) => batch.progress,
                    Err(failure) => failure.progress,
                };
                statistics.record(seeds.len(), result.is_ok(), progress, &oracle.statistics())?;
                crate::lazy_execution::batch_results(result)
            }
            Self::Cpu(oracle) => Ok(oracle
                .check_batch(program, seeds, limits, control)
                .map_err(RunError::Batch)?
                .into_iter()
                .map(|result| {
                    result.map(|check| match check.into_stable_interpretation() {
                        Ok(accepted) => Some(accepted.into_interpretation()),
                        Err(_) => None,
                    })
                })
                .collect()),
            Self::StaticCpu { oracle, ground } => oracle
                .check_static_batch(ground, seeds, limits, control)
                .map_err(RunError::Batch)?
                .into_iter()
                .map(|result| match result {
                    Ok(check) => decode(ground, check.accepted(), check.closure_words()).map(Ok),
                    Err(stop) => Ok(Err(stop)),
                })
                .collect(),
            #[cfg(feature = "gpu")]
            Self::Gpu { oracle, ground } => {
                let limits = zetesis_wgpu::GpuLimits {
                    max_candidates: options.batch_size.get(),
                    max_batch_bytes: options.max_batch_bytes,
                    ..Default::default()
                };
                let checks = oracle
                    .check_batch(ground, seeds, limits)
                    .map_err(RunError::Gpu)?;
                if let Err(error) = control.poll() {
                    return Ok(vec![Err(error)]);
                }
                checks
                    .into_iter()
                    .map(|check| decode(ground, check.accepted(), check.closure_words()).map(Ok))
                    .collect()
            }
        }
    }
}

fn compile_static(
    options: &SolveConfig,
    program: &Program,
    max_atoms: usize,
    phases: &Recorder,
) -> Result<Arc<GroundProgram>, RunError> {
    let grounding = phases.stage(crate::SolveStage::Grounding);
    let result = GroundProgram::compile(
        program,
        StaticLimits {
            max_atoms,
            max_ground_rules: options.max_ground_rules,
            max_substitutions: options.max_substitutions,
        },
    );
    drop(grounding);
    result.map(Arc::new).map_err(RunError::Static)
}

fn static_diagnostics(
    options: &SolveConfig,
    ground: &GroundProgram,
    diagnostics: &mut Diagnostics<impl Write>,
) -> Result<(), RunError> {
    diagnostics.metadata(
        Label::Grounding,
        format_args!(
            "requested={}, effective=eager (static atoms={}, rules={}; lowering caps atoms={}, rules={}, substitutions={})",
            options.grounder.label(),
            ground.atom_count(),
            ground.rules().len(),
            options.max_atoms,
            options.max_ground_rules,
            options.max_substitutions
        ),
    )?;
    Ok(())
}

fn decode(
    ground: &GroundProgram,
    accepted: bool,
    words: &[u32],
) -> Result<Option<Model>, RunError> {
    if accepted {
        ground
            .model_from_words(words)
            .map(Some)
            .map_err(RunError::Words)
    } else {
        Ok(None)
    }
}

#[cfg(feature = "gpu")]
pub(crate) fn selection(backend: Backend) -> zetesis_wgpu::GpuSelection {
    use zetesis_wgpu::{GpuBackendPreference, GpuSelection, NVIDIA_VENDOR_ID};

    GpuSelection {
        backend: match backend {
            Backend::Metal => GpuBackendPreference::Metal,
            Backend::Vulkan => GpuBackendPreference::Vulkan,
            Backend::Dx12 => GpuBackendPreference::Dx12,
            Backend::Gl => GpuBackendPreference::Gl,
            Backend::Auto | Backend::Cpu | Backend::Gpu | Backend::Nvidia => {
                GpuBackendPreference::Auto
            }
        },
        vendor_id: (backend == Backend::Nvidia).then_some(NVIDIA_VENDOR_ID),
    }
}

#[cfg(test)]
#[path = "../tests/support/engine_control_contracts.rs"]
mod control_contract_tests;

#[cfg(test)]
mod tests {
    use super::{AUTO_GPU_MIN_BATCH, should_probe_gpu};

    #[test]
    fn auto_probes_only_once_and_after_the_lazy_first_seed() {
        assert!(!should_probe_gpu(true, false, 1));
        assert!(!should_probe_gpu(true, false, AUTO_GPU_MIN_BATCH - 1));
        assert!(should_probe_gpu(true, false, AUTO_GPU_MIN_BATCH));
        assert!(!should_probe_gpu(true, true, AUTO_GPU_MIN_BATCH));
        assert!(!should_probe_gpu(false, false, AUTO_GPU_MIN_BATCH));
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn explicit_api_and_vendor_requests_are_preserved() {
        use crate::Backend;
        use zetesis_wgpu::{GpuBackendPreference, NVIDIA_VENDOR_ID};

        for (request, expected) in [
            (Backend::Metal, GpuBackendPreference::Metal),
            (Backend::Vulkan, GpuBackendPreference::Vulkan),
            (Backend::Dx12, GpuBackendPreference::Dx12),
            (Backend::Gl, GpuBackendPreference::Gl),
        ] {
            let selected = super::selection(request);
            assert_eq!(selected.backend, expected);
            assert_eq!(selected.vendor_id, None);
        }
        let nvidia = super::selection(Backend::Nvidia);
        assert_eq!(nvidia.vendor_id, Some(NVIDIA_VENDOR_ID));
        assert_eq!(nvidia.backend, GpuBackendPreference::Auto);
    }
}
