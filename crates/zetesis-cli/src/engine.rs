//! Independent materialization selection and provisional hardware scheduling.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;
use std::sync::Arc;

use zetesis_core::{GroundProgram, Model, Program, Seed, StaticLimits};
use zetesis_cpu::{BatchOracle, Control, Limits, Stop};

use crate::phase_timing::{Recorder, SolvePhase};
use crate::{Backend, ExecutionResources, Grounder, Oracle, RunError, SolveConfig, SourceBatching};

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
    // Retain only resources needed by the deferred automatic device attempt.
    resources: ExecutionResources,
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
        observations: &mut impl ExecutionSink,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        Self::with_ground(
            options,
            program,
            None,
            &ExecutionResources::default(),
            observations,
            phases,
        )
    }

    pub(crate) fn with_ground(
        options: &SolveConfig,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        resources: &ExecutionResources,
        observations: &mut impl ExecutionSink,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        validate_combination(options)?;
        let executor = match options.backend {
            Backend::Auto | Backend::Cpu => {
                let cpu = Executor::cpu(options, program, cached, observations, phases)?;
                if options.backend == Backend::Auto {
                    if options.source_batching != SourceBatching::Independent {
                        observations.record(Event::SharedCpu)?;
                    } else if cfg!(feature = "gpu") {
                        observations.record(Event::DeferredDevice {
                            minimum_batch: AUTO_GPU_MIN_BATCH,
                            grounder: grounding_mode(options),
                        })?;
                    } else {
                        observations.record(Event::DeviceNotCompiled)?;
                    }
                }
                cpu
            }
            _ => Executor::gpu(options, program, cached, resources, observations, phases)?,
        };
        let automatic = options.backend == Backend::Auto
            && options.source_batching == SourceBatching::Independent
            && cfg!(feature = "gpu");
        Ok(Self {
            executor,
            automatic,
            attempted_gpu: false,
            resources: if automatic {
                resources.clone()
            } else {
                ExecutionResources::default()
            },
            retired_lazy_statistics: None,
        })
    }

    // Accept a prepared executor only after all its observations succeeded.
    // External observation failures are terminal, never device unavailability.
    fn finish_device_attempt(
        &mut self,
        attempt: Result<Executor, RunError>,
        options: &SolveConfig,
        observations: &mut impl ExecutionSink,
    ) -> Result<(), RunError> {
        match attempt {
            Ok(gpu) => self.executor = gpu,
            Err(error @ (RunError::Output(_) | RunError::ExecutionObservation(_))) => {
                return Err(error);
            }
            Err(error) => observations.record(Event::DeviceUnavailable {
                grounder: grounding_mode(options),
                cause: &error,
            })?,
        }
        Ok(())
    }

    pub(crate) fn check(
        &mut self,
        options: &SolveConfig,
        program: &Program,
        seeds: &[Seed],
        observations: &mut impl ExecutionSink,
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
            let resources = std::mem::take(&mut self.resources);
            let attempt = phases.measure(SolvePhase::ExecutionSetup, || {
                Executor::gpu(
                    options,
                    program,
                    self.executor.ground(),
                    &resources,
                    observations,
                    phases,
                )
            });
            self.finish_device_attempt(attempt, options, observations)?;
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
                observations.record(Event::DeviceRetry {
                    grounder: grounding_mode(options),
                    cause: &error,
                })?;
                self.executor = phases.measure(SolvePhase::ExecutionSetup, || {
                    Executor::cpu(
                        options,
                        program,
                        self.executor.ground(),
                        observations,
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

fn grounding_mode(options: &SolveConfig) -> Grounder {
    if options.grounder == Grounder::Eager {
        Grounder::Eager
    } else {
        Grounder::Lazy
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
        observations: &mut impl ExecutionSink,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        let oracle =
            BatchOracle::new(options.workers, options.batch_size).map_err(RunError::Batch)?;
        if options.grounder == Grounder::Eager {
            let ground = match cached {
                Some(ground) => ground,
                None => compile_static(options, program, options.max_atoms, phases)?,
            };
            observe_static(options, &ground, observations)?;
            observations.record(Event::CpuClosure {
                grounder: Grounder::Eager,
                batching: options.source_batching,
                workers: options.workers,
            })?;
            Ok(Self::StaticCpu { oracle, ground })
        } else {
            phases.lazy_grounding();
            observations.record(Event::LazyGrounding {
                requested: options.grounder,
            })?;
            if let Some(selection) = options.source_batching.selection() {
                observations.record(Event::CpuClosure {
                    grounder: Grounder::Lazy,
                    batching: options.source_batching,
                    workers: options.workers,
                })?;
                Ok(Self::SharedCpu {
                    oracle,
                    statistics: crate::SharedExecutionStatistics::new(options, selection),
                })
            } else {
                observations.record(Event::CpuClosure {
                    grounder: Grounder::Lazy,
                    batching: options.source_batching,
                    workers: options.workers,
                })?;
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
        _: &ExecutionResources,
        _: &mut impl ExecutionSink,
        _: &Recorder,
    ) -> Result<Self, RunError> {
        Err(RunError::BackendUnavailable)
    }

    #[cfg(feature = "gpu")]
    fn gpu(
        options: &SolveConfig,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        resources: &ExecutionResources,
        observations: &mut impl ExecutionSink,
        phases: &Recorder,
    ) -> Result<Self, RunError> {
        use zetesis_wgpu::{GpuOptions, GpuOracle};

        let context = resources.gpu_for(options.backend).map_err(RunError::Gpu)?;
        if options.grounder != Grounder::Eager {
            let oracle = match context {
                Some(context) => zetesis_wgpu::GpuLazyOracle::from_context(context),
                None => zetesis_wgpu::GpuLazyOracle::new_selected(
                    GpuOptions::default(),
                    selection(options.backend),
                ),
            }
            .map_err(RunError::Gpu)?;
            let statistics =
                crate::LazyExecutionStatistics::new(options.backend, oracle.info().metadata());
            phases.lazy_grounding();
            observations.record(Event::LazyDeviceGrounding {
                requested: options.grounder,
            })?;
            observations.record(Event::DeviceClosure {
                adapter: oracle.info().metadata(),
                static_counts: None,
            })?;
            return Ok(Self::LazyGpu(Box::new(LazyGpu { oracle, statistics })));
        }

        // Validate or discover hardware before new static materialization.
        // Eager CPU attempts already own a graph, shared without expansion.
        let oracle = match context {
            Some(context) => GpuOracle::from_context(context),
            None => GpuOracle::new_selected(GpuOptions::default(), selection(options.backend)),
        }
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
        observe_static(options, &ground, observations)?;
        observations.record(Event::DeviceClosure {
            adapter: oracle.info().metadata(),
            static_counts: Some((ground.atom_count(), ground.rules().len())),
        })?;
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

fn observe_static(
    options: &SolveConfig,
    ground: &GroundProgram,
    observations: &mut impl ExecutionSink,
) -> Result<(), RunError> {
    observations.record(Event::StaticGrounding {
        requested: options.grounder,
        atoms: ground.atom_count(),
        rules: ground.rules().len(),
        limits: StaticLimits {
            max_atoms: options.max_atoms,
            max_ground_rules: options.max_ground_rules,
            max_substitutions: options.max_substitutions,
        },
    })?;
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

#[cfg(all(test, feature = "gpu"))]
#[path = "../tests/support/engine_resources.rs"]
mod resource_tests;

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
