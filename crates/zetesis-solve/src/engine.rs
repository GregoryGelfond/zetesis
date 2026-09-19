//! Independent materialization selection and explicit hardware execution.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;
use std::sync::Arc;

use rayon::prelude::*;
use zetesis_core::{
    GroundProgram, Model, ModelError, Program, SeedSelection, StaticLimits, WordError,
};
use zetesis_cpu::{BatchOracle, Control, Limits, PreparationLimits, Stop};

use crate::phase_timing::{Recorder, SolvePhase};
use crate::{
    Backend, ExecutionResources, Grounder, Oracle, SolveConfig, SolveError, SourceBatching,
};

pub(crate) fn validate_combination(options: &SolveConfig) -> Result<(), SolveError> {
    if options.source_batching != SourceBatching::Independent
        && (!matches!(options.backend, Backend::Auto | Backend::Cpu)
            || options.grounder == Grounder::Eager)
    {
        return Err(SolveError::UnsupportedSourceBatching);
    }
    if options.oracle == Oracle::Countermodel {
        validate_countermodel(options)?;
    }
    // Each assigned worker is admitted at the full per-closure allowance, so
    // the collective ceiling must hold the product before a session starts.
    let workers = options.workers.get();
    if workers
        .checked_mul(options.max_closure_bytes)
        .is_none_or(|product| product > options.max_closure_batch_bytes)
    {
        return Err(SolveError::ClosureReservation {
            workers,
            max_closure_bytes: options.max_closure_bytes,
            max_closure_batch_bytes: options.max_closure_batch_bytes,
        });
    }
    Ok(())
}

pub(crate) fn validate_countermodel(options: &SolveConfig) -> Result<(), SolveError> {
    if options.source_batching != SourceBatching::Independent {
        return Err(SolveError::UnsupportedSourceBatching);
    }
    if options.grounder == Grounder::Lazy {
        return Err(SolveError::UnsupportedOracle {
            backend: options.backend,
            grounder: options.grounder,
        });
    }
    Ok(())
}

pub(crate) struct Engine {
    executor: Executor,
}

impl Engine {
    pub(crate) fn query_observation(&self) -> Option<&crate::QueryExecutionObservation> {
        match &self.executor {
            Executor::Cpu(executor) => Some(&executor.observation),
            _ => None,
        }
    }
    pub(crate) fn closure_statistics(&self) -> Option<crate::ClosureExecutionStatistics> {
        match &self.executor {
            Executor::Cpu(executor) => Some(executor.statistics.clone()),
            Executor::StaticCpu { statistics, .. } => Some(statistics.clone()),
            Executor::SharedCpu { .. } => None,
            #[cfg(feature = "gpu")]
            Executor::Gpu { .. } | Executor::LazyGpu(_) => None,
        }
    }
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
        let statistics: Option<&crate::LazyExecutionStatistics> = match &self.executor {
            #[cfg(feature = "gpu")]
            Executor::LazyGpu(executor) => Some(&executor.statistics),
            _ => None,
        };
        statistics.map(|statistics| {
            let mut statistics = statistics.clone();
            statistics.queued_results = queued_results;
            statistics
        })
    }
    #[cfg(test)]
    pub(crate) fn new(
        options: &SolveConfig,
        program: &Program,
        observations: &mut impl ExecutionSink,
        phases: &Recorder,
    ) -> Result<Self, SolveError> {
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
    ) -> Result<Self, SolveError> {
        validate_combination(options)?;
        let executor = match options.backend {
            Backend::Auto | Backend::Cpu => {
                let cpu = Executor::cpu(options, program, cached, observations, phases)?;
                if options.backend == Backend::Auto {
                    if options.source_batching != SourceBatching::Independent {
                        observations.record(Event::SharedCpu)?;
                    } else if cfg!(feature = "gpu") {
                        observations.record(Event::AutomaticCpu)?;
                    } else {
                        observations.record(Event::DeviceNotCompiled)?;
                    }
                }
                cpu
            }
            _ => Executor::gpu(options, program, cached, resources, observations, phases)?,
        };
        Ok(Self { executor })
    }

    pub(crate) fn check(
        &mut self,
        options: &SolveConfig,
        program: &Program,
        seeds: &[SeedSelection],
        control: &Control,
        phases: &Recorder,
    ) -> Result<Vec<Result<Option<Model>, Stop>>, SolveError> {
        if seeds.is_empty() {
            return Ok(Vec::new());
        }
        if let Err(error) = control.poll() {
            return Ok(vec![Err(error)]);
        }
        let phase = if self.executor.is_gpu() {
            SolvePhase::GpuHostOracle
        } else {
            SolvePhase::ClosureMembership
        };
        phases.measure(phase, || {
            self.executor.check(options, program, seeds, control)
        })
    }
}

enum Executor {
    Cpu(IndependentCpu),
    SharedCpu {
        oracle: BatchOracle,
        statistics: crate::SharedExecutionStatistics,
    },
    StaticCpu {
        oracle: BatchOracle,
        ground: Arc<GroundProgram>,
        statistics: crate::ClosureExecutionStatistics,
    },
    #[cfg(feature = "gpu")]
    Gpu {
        oracle: Box<zetesis_wgpu::GpuOracle>,
        ground: Arc<GroundProgram>,
    },
    #[cfg(feature = "gpu")]
    LazyGpu(Box<LazyGpu>),
}

struct IndependentCpu {
    oracle: BatchOracle,
    observation: crate::QueryExecutionObservation,
    statistics: crate::ClosureExecutionStatistics,
}

impl IndependentCpu {
    fn check(
        &mut self,
        program: &Program,
        seeds: &[SeedSelection],
        limits: Limits,
        control: &Control,
    ) -> Result<Vec<Result<Option<Model>, Stop>>, SolveError> {
        let result = self.oracle.check_batch_views(
            program,
            seeds.par_iter().map(SeedSelection::view),
            limits,
            control,
        );
        self.observation.capture(self.oracle.query_statistics());
        // A snapshot fault is retained separately and delivered by the session
        // after these already-checked results. No membership is discarded here.
        result
            .map_err(SolveError::Batch)?
            .into_iter()
            .map(|result| match result {
                Ok(check) => {
                    self.statistics.completed_lazy(&check.statistics())?;
                    Ok(Ok(match check.into_stable_interpretation() {
                        Ok(accepted) => Some(accepted.into_interpretation()),
                        Err(_) => None,
                    }))
                }
                Err(stop) => {
                    self.statistics.stopped()?;
                    Ok(Err(stop))
                }
            })
            .collect()
    }
}

/// Keep device execution and its cumulative observations under one owner.
/// The enum uses one allocation for this state, leaving CPU variants compact.
#[cfg(feature = "gpu")]
struct LazyGpu {
    oracle: zetesis_wgpu::GpuLazyOracle,
    statistics: crate::LazyExecutionStatistics,
}

#[cfg(feature = "gpu")]
impl LazyGpu {
    /// Record the complete attempt before interpreting its bounded result.
    fn check(
        &mut self,
        options: &SolveConfig,
        program: &Program,
        seeds: &[SeedSelection],
        control: &Control,
    ) -> Result<Vec<Result<Option<Model>, Stop>>, SolveError> {
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
        let result = self.oracle.check_batch_views(
            program,
            seeds.iter().map(SeedSelection::view),
            source_limits,
            device_limits,
            control,
        );
        let progress = match &result {
            Ok(batch) => batch.progress,
            Err(failure) => failure.progress,
        };
        self.statistics.record(
            seeds.len(),
            result.is_ok(),
            progress,
            &self.oracle.statistics(),
        )?;
        crate::lazy_execution::batch_results(result)
    }
}

impl Executor {
    fn cpu(
        options: &SolveConfig,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        observations: &mut impl ExecutionSink,
        phases: &Recorder,
    ) -> Result<Self, SolveError> {
        let oracle = BatchOracle::new(options.workers, options.batch_size)
            .map_err(SolveError::Batch)?
            .with_closure_storage_limit(options.max_closure_batch_bytes)
            .with_preparation_limits(PreparationLimits {
                max_work: options.max_source_work,
                max_bytes: options.max_closure_batch_bytes,
                ..PreparationLimits::default()
            });
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
            Ok(Self::StaticCpu {
                oracle,
                ground,
                statistics: crate::ClosureExecutionStatistics::new(Grounder::Eager),
            })
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
                Ok(Self::Cpu(IndependentCpu {
                    oracle,
                    observation: crate::QueryExecutionObservation::default(),
                    statistics: crate::ClosureExecutionStatistics::new(Grounder::Lazy),
                }))
            }
        }
    }

    #[cfg(test)]
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
    ) -> Result<Self, SolveError> {
        Err(SolveError::BackendUnavailable)
    }

    #[cfg(feature = "gpu")]
    fn gpu(
        options: &SolveConfig,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        resources: &ExecutionResources,
        observations: &mut impl ExecutionSink,
        phases: &Recorder,
    ) -> Result<Self, SolveError> {
        use zetesis_wgpu::{GpuOptions, GpuOracle};

        let context = resources
            .gpu_for(options.backend)
            .map_err(SolveError::Gpu)?;
        if options.grounder != Grounder::Eager {
            let oracle = match context {
                Some(context) => zetesis_wgpu::GpuLazyOracle::from_context(context),
                None => zetesis_wgpu::GpuLazyOracle::new_selected(
                    GpuOptions::default(),
                    selection(options.backend),
                ),
            }
            .map_err(SolveError::Gpu)?;
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
        // Caller-supplied compiled graphs are reused without expansion.
        let oracle = match context {
            Some(context) => GpuOracle::from_context(context),
            None => GpuOracle::new_selected(GpuOptions::default(), selection(options.backend)),
        }
        .map_err(SolveError::Gpu)?;
        let atom_limit = options.max_atoms.min(zetesis_wgpu::MAX_ATOMS);
        let ground = match cached {
            Some(ground) => {
                if ground.atom_count() > atom_limit {
                    return Err(SolveError::Static(
                        zetesis_core::StaticError::LimitExceeded {
                            resource: "GPU atoms",
                            actual: ground.atom_count(),
                            limit: atom_limit,
                        },
                    ));
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
        seeds: &[SeedSelection],
        control: &Control,
    ) -> Result<Vec<Result<Option<Model>, Stop>>, SolveError> {
        let limits = Limits {
            max_work: options.max_work,
            max_derived_atoms: options.max_atoms,
            max_closure_bytes: options.max_closure_bytes,
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
                let result = oracle.check_shared_views(
                    program,
                    seeds.iter().map(SeedSelection::view),
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
            Self::LazyGpu(executor) => executor.check(options, program, seeds, control),
            Self::Cpu(executor) => executor.check(program, seeds, limits, control),
            Self::StaticCpu {
                oracle,
                ground,
                statistics,
            } => oracle
                .check_static_batch_views(
                    ground,
                    seeds.par_iter().map(SeedSelection::view),
                    limits,
                    control,
                )
                .map_err(SolveError::Batch)?
                .into_iter()
                .map(|result| match result {
                    Ok(check) => {
                        statistics.completed_eager(&check.statistics())?;
                        decode(ground, check.accepted(), check.closure_words())
                    }
                    Err(stop) => {
                        statistics.stopped()?;
                        Ok(Err(stop))
                    }
                })
                .collect(),
            #[cfg(feature = "gpu")]
            Self::Gpu { oracle, ground } => {
                let limits = zetesis_wgpu::GpuLimits {
                    max_candidates: options.batch_size.get(),
                    max_batch_bytes: options.max_batch_bytes,
                    ..Default::default()
                };
                let checks = match oracle.check_batch_views_with_control(
                    ground,
                    seeds.iter().map(SeedSelection::view),
                    limits,
                    control,
                ) {
                    Ok(checks) => checks,
                    Err(error) => {
                        return match error.interruption() {
                            Some(stop) => Ok(vec![Err(stop)]),
                            None => Err(SolveError::Gpu(error)),
                        };
                    }
                };
                if let Err(error) = control.poll() {
                    return Ok(vec![Err(error)]);
                }
                checks
                    .into_iter()
                    .map(|check| decode(ground, check.accepted(), check.closure_words()))
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
) -> Result<Arc<GroundProgram>, SolveError> {
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
    result.map(Arc::new).map_err(SolveError::Static)
}

fn observe_static(
    options: &SolveConfig,
    ground: &GroundProgram,
    observations: &mut impl ExecutionSink,
) -> Result<(), SolveError> {
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
) -> Result<Result<Option<Model>, Stop>, SolveError> {
    if accepted {
        materialized(ground.model_from_words(words))
    } else {
        Ok(Ok(None))
    }
}

// Accepted closure words still need an owned model before this candidate can
// produce an answer. Allocation stops that candidate; malformed words remain
// an execution failure. Both static backends use this publication boundary.
fn materialized(
    result: Result<Model, WordError>,
) -> Result<Result<Option<Model>, Stop>, SolveError> {
    match result {
        Ok(model) => Ok(Ok(Some(model))),
        Err(WordError::Model(ModelError::Allocation)) => Ok(Err(Stop::Allocation)),
        Err(error) => Err(SolveError::Words(error)),
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
    #[test]
    fn static_model_allocation_stops_the_candidate() {
        assert!(matches!(
            super::materialized(Err(zetesis_core::WordError::Model(
                zetesis_core::ModelError::Allocation
            ))),
            Ok(Err(zetesis_cpu::Stop::Allocation))
        ));
    }

    #[test]
    fn malformed_static_words_remain_execution_failures() {
        assert!(matches!(
            super::materialized(Err(zetesis_core::WordError::TailBits)),
            Err(crate::SolveError::Words(zetesis_core::WordError::TailBits))
        ));
    }

    #[test]
    fn materialized_static_models_keep_their_owner() {
        let model = zetesis_core::Model::new([]);
        let owner = model.catalog().clone();
        let retained = super::materialized(Ok(model)).unwrap().unwrap().unwrap();
        assert!(retained.catalog().same_owner(&owner));
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
