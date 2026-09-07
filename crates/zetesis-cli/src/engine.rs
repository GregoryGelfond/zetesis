//! Independent materialization selection and provisional hardware scheduling.

use std::io::Write;
use std::sync::Arc;

use zetesis_core::{GroundProgram, Model, Program, Seed, StaticLimits};
use zetesis_cpu::{BatchOracle, Control, Limits, Stop};

use crate::phase_timing::{Recorder, SolvePhase};
use crate::{Backend, Grounder, Options, Oracle, RunError};

/// An initial scheduling heuristic, not a measured performance crossover.
pub(crate) const AUTO_GPU_MIN_BATCH: usize = 32;

pub(crate) fn validate_combination(options: &Options) -> Result<(), RunError> {
    if options.oracle == Oracle::Countermodel {
        validate_countermodel(options)?;
    }
    if options.grounder == Grounder::Lazy
        && !matches!(options.backend, Backend::Auto | Backend::Cpu)
    {
        return Err(RunError::UnsupportedCombination {
            backend: options.backend,
            grounder: options.grounder,
        });
    }
    Ok(())
}

pub(crate) fn validate_countermodel(options: &Options) -> Result<(), RunError> {
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
}

impl Engine {
    pub(crate) fn new(
        options: &Options,
        program: &Program,
        diagnostics: &mut impl Write,
    ) -> Result<Self, RunError> {
        validate_combination(options)?;
        let executor = match options.backend {
            Backend::Auto | Backend::Cpu => {
                let cpu = Executor::cpu(options, program, None, diagnostics)?;
                if options.backend == Backend::Auto {
                    if options.grounder == Grounder::Lazy {
                        writeln!(
                            diagnostics,
                            "Auto: --grounder lazy requires source joins; using CPU without device discovery."
                        )?;
                    } else if cfg!(feature = "gpu") {
                        writeln!(
                            diagnostics,
                            "Auto: GPU discovery deferred; the first seed stays CPU. Later batches of at least {AUTO_GPU_MIN_BATCH} candidates may use a physical GPU with static lowering (provisional heuristic)."
                        )?;
                    } else {
                        writeln!(
                            diagnostics,
                            "Auto: GPU support was not compiled; using CPU without device discovery."
                        )?;
                    }
                }
                cpu
            }
            _ => Executor::gpu(options, program, None, diagnostics)?,
        };
        Ok(Self {
            executor,
            automatic: options.backend == Backend::Auto
                && options.grounder != Grounder::Lazy
                && cfg!(feature = "gpu"),
            attempted_gpu: false,
        })
    }

    pub(crate) fn check(
        &mut self,
        options: &Options,
        program: &Program,
        seeds: &[Seed],
        diagnostics: &mut impl Write,
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
                Executor::gpu(options, program, self.executor.ground(), diagnostics)
            }) {
                Ok(gpu) => self.executor = gpu,
                Err(error @ RunError::Output(_)) => return Err(error),
                Err(error) => {
                    writeln!(
                        diagnostics,
                        "Auto: retaining {} CPU; GPU unavailable: {error}",
                        cpu_mode(options)
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
            Err(error) if self.automatic && self.executor.is_gpu() => {
                // No failed-batch result has been published. Eager retains the
                // same graph; Auto returns to source joins for these same seeds.
                writeln!(
                    diagnostics,
                    "Auto: GPU batch failed; retrying on {} CPU: {error}",
                    cpu_mode(options)
                )?;
                self.executor = phases.measure(SolvePhase::ExecutionSetup, || {
                    Executor::cpu(options, program, self.executor.ground(), diagnostics)
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

fn cpu_mode(options: &Options) -> &'static str {
    if options.grounder == Grounder::Eager {
        "eager"
    } else {
        "lazy"
    }
}

enum Executor {
    Cpu(BatchOracle),
    StaticCpu {
        oracle: BatchOracle,
        ground: Arc<GroundProgram>,
    },
    #[cfg(feature = "gpu")]
    Gpu {
        oracle: Box<zetesis_wgpu::GpuOracle>,
        ground: Arc<GroundProgram>,
    },
}

impl Executor {
    fn cpu(
        options: &Options,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        diagnostics: &mut impl Write,
    ) -> Result<Self, RunError> {
        let oracle =
            BatchOracle::new(options.workers, options.batch_size).map_err(RunError::Batch)?;
        if options.grounder == Grounder::Eager {
            let ground = match cached {
                Some(ground) => ground,
                None => compile_static(options, program, options.max_atoms)?,
            };
            static_diagnostics(options, &ground, diagnostics)?;
            writeln!(
                diagnostics,
                "Backend: cpu (eager static closure scans, {} workers)",
                options.workers
            )?;
            Ok(Self::StaticCpu { oracle, ground })
        } else {
            writeln!(
                diagnostics,
                "Grounding: requested={}, effective=lazy (source joins; no complete ground-rule store)",
                options.grounder.label()
            )?;
            writeln!(
                diagnostics,
                "Backend: cpu (lazy source joins, {} workers)",
                options.workers
            )?;
            Ok(Self::Cpu(oracle))
        }
    }

    fn ground(&self) -> Option<Arc<GroundProgram>> {
        match self {
            Self::Cpu(_) => None,
            Self::StaticCpu { ground, .. } => Some(Arc::clone(ground)),
            #[cfg(feature = "gpu")]
            Self::Gpu { ground, .. } => Some(Arc::clone(ground)),
        }
    }

    fn is_gpu(&self) -> bool {
        match self {
            Self::Cpu(_) | Self::StaticCpu { .. } => false,
            #[cfg(feature = "gpu")]
            Self::Gpu { .. } => true,
        }
    }

    #[cfg(not(feature = "gpu"))]
    fn gpu(
        _: &Options,
        _: &Program,
        _: Option<Arc<GroundProgram>>,
        _: &mut impl Write,
    ) -> Result<Self, RunError> {
        Err(RunError::BackendUnavailable)
    }

    #[cfg(feature = "gpu")]
    fn gpu(
        options: &Options,
        program: &Program,
        cached: Option<Arc<GroundProgram>>,
        diagnostics: &mut impl Write,
    ) -> Result<Self, RunError> {
        use zetesis_wgpu::{GpuOptions, GpuOracle};

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
            None => compile_static(options, program, atom_limit)?,
        };
        static_diagnostics(options, &ground, diagnostics)?;
        writeln!(
            diagnostics,
            "Backend: gpu ({}, {}; vendor=0x{:04x}; static atoms={}, rules={})",
            oracle.info().name(),
            oracle.info().backend(),
            oracle.info().vendor_id(),
            ground.atom_count(),
            ground.rules().len()
        )?;
        Ok(Self::Gpu {
            oracle: Box::new(oracle),
            ground,
        })
    }

    fn check(
        &mut self,
        options: &Options,
        program: &Program,
        seeds: &[Seed],
        control: &Control,
    ) -> Result<Vec<Result<Option<Model>, Stop>>, RunError> {
        let limits = Limits {
            max_work: options.max_work,
            max_derived_atoms: options.max_atoms,
        };
        match self {
            Self::Cpu(oracle) => Ok(oracle
                .check_batch(program, seeds, limits, control)
                .map_err(RunError::Batch)?
                .into_iter()
                .map(|result| result.map(|check| check.accepted().then(|| check.closure().clone())))
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
    options: &Options,
    program: &Program,
    max_atoms: usize,
) -> Result<Arc<GroundProgram>, RunError> {
    GroundProgram::compile(
        program,
        StaticLimits {
            max_atoms,
            max_ground_rules: options.max_ground_rules,
            max_substitutions: options.max_substitutions,
        },
    )
    .map(Arc::new)
    .map_err(RunError::Static)
}

fn static_diagnostics(
    options: &Options,
    ground: &GroundProgram,
    diagnostics: &mut impl Write,
) -> Result<(), RunError> {
    writeln!(
        diagnostics,
        "Grounding: requested={}, effective=eager (static atoms={}, rules={}; lowering caps atoms={}, rules={}, substitutions={})",
        options.grounder.label(),
        ground.atom_count(),
        ground.rules().len(),
        options.max_atoms,
        options.max_ground_rules,
        options.max_substitutions
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
