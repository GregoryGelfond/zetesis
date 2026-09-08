//! Useful per-world device inference over bounded lazy source chunks.
//!
//! The CPU source coordinator offers instances from the union snapshot. The
//! kernel independently checks each world's positives and frozen gates and
//! derives its head delta. No completed CPU closure or complete ground graph is
//! uploaded. One batch reuses bounded transport capacity across chunks/rounds;
//! every active input and output is refreshed before each dispatch. Automatic
//! performance selection remains a separate policy.

use std::borrow::Cow;
use std::time::Duration;

use zetesis_core::{Program, Seed};
use zetesis_cpu::{Control, lazy};

use crate::runtime::{self, DeviceProfile, ErrorScopes, Runtime};
use crate::{GpuError, GpuErrorKind, GpuInfo, GpuLimits, GpuOptions, GpuSelection};

mod plan;
mod transport;

use plan::{Capacity, Plan};
use transport::Transport;

const SHADER: &str = include_str!("lazy.wgsl");
const DIMENSION_WORDS: usize = 4;
const UNIFORM_BYTES: u64 = 32;
const RESULT_METADATA_WORDS: usize = 3;

/// Actual submitted device work for the latest attempted lazy batch, including
/// submissions before a later source/device failure. No timing is a GPU-only
/// kernel duration; waits include submission completion and mapped readback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LazyGpuStatistics {
    /// Submitted nonempty source chunks.
    pub dispatches: u64,
    /// Sum of submitted source instances times candidate occurrences.
    pub world_instances: u64,
    /// Bytes uploaded in uniforms, offsets, records, snapshots and seeds.
    pub uploaded_bytes: u64,
    /// Successfully decoded readback bytes.
    pub downloaded_bytes: u64,
    /// Submitted chunks that requested a new complete set of transport buffers.
    /// This counts allocation requests, including a later device failure.
    pub transport_allocations: u64,
    /// Submitted chunks using capacity from an earlier chunk of this batch.
    pub transport_reuses: u64,
    /// Largest requested GPU buffer payload for a submitted chunk, including
    /// retained inactive capacity. Excludes host payload and driver allocations.
    pub peak_transport_bytes: u64,
    /// Completed host time waiting for submissions and decoding readback.
    pub host_wait: Duration,
}

/// Physical-device executor for the admitted relational lazy source profile.
/// It owns the existing shared wgpu lifecycle and retains no cross-batch atoms,
/// snapshots, completed closures, instance chunks or transport buffers.
pub struct GpuLazyOracle {
    runtime: Runtime,
    statistics: LazyGpuStatistics,
    epoch: u32,
}

impl GpuLazyOracle {
    /// Discover a device and validate the distinct immutable-round shader.
    ///
    /// # Errors
    /// Returns adapter, capability, shader, pipeline or device failure.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::new(
            options,
            selection,
            DeviceProfile {
                device_label: "zetesis lazy reduct device",
                shader_label: "zetesis lazy consequence shader",
                pipeline_label: "zetesis lazy consequence pipeline",
                shader: Cow::Borrowed(SHADER),
                entry_point: "consequence",
                validate_limits: crate::check_adapter_limits,
            },
        ))?;
        Ok(Self {
            runtime,
            statistics: LazyGpuStatistics::default(),
            epoch: 0,
        })
    }

    /// Selected physical adapter metadata; constant-time borrow.
    #[must_use]
    pub const fn info(&self) -> &GpuInfo {
        &self.runtime.info
    }

    /// Actual work from the latest attempt, including a failed attempt.
    #[must_use]
    pub const fn statistics(&self) -> LazyGpuStatistics {
        self.statistics
    }

    /// Check the exact seed occurrences using host source joins and useful
    /// per-world device consequences. Source limits are shared across the batch;
    /// transport limits apply to every nonempty chunk. `max_batch_bytes` counts
    /// all retained GPU buffer capacities plus active host packing and decoded
    /// words; coordinator storage is separately bounded by
    /// `source_limits.max_host_bytes`. Buffers are dropped before replacement and
    /// on every batch exit; driver-private deferred retirement is not counted.
    ///
    /// # Errors
    /// Returns no completed check on interrupted source coverage, capacity,
    /// device, timeout, readback or malformed-output failure. Charged source
    /// progress is retained in the error, device work in [`Self::statistics`].
    pub fn check_batch(
        &mut self,
        program: &Program,
        seeds: &[Seed],
        source_limits: lazy::Limits,
        limits: GpuLimits,
        control: &Control,
    ) -> Result<lazy::Batch, lazy::Failure<GpuError>> {
        self.check_batch_with_source(
            program,
            seeds,
            source_limits,
            limits,
            lazy::SourceSelection::Union,
            control,
        )
    }

    /// Check the same seed occurrences with explicit host source selection.
    /// [`lazy::SourceSelection::Worlds`] removes positive prefixes with no
    /// current-world witness before transport. The device still checks every
    /// positive antecedent and frozen gate; its ABI and execution are unchanged.
    /// Source mask storage/work share the supplied source budgets, and their
    /// progress is distinct from actual submitted device work. Host join storage
    /// is reused only within this batch; each round rebuilds membership truth.
    ///
    /// # Errors
    /// Preserves [`Self::check_batch`]'s failure and incomplete-coverage contract.
    /// This opt-in path requires its own physical-device qualification; portable
    /// semantic and transport tests do not establish hardware execution.
    pub fn check_batch_with_source(
        &mut self,
        program: &Program,
        seeds: &[Seed],
        source_limits: lazy::Limits,
        limits: GpuLimits,
        selection: lazy::SourceSelection,
        control: &Control,
    ) -> Result<lazy::Batch, lazy::Failure<GpuError>> {
        self.statistics = LazyGpuStatistics::default();
        let mut transport = None;
        let result =
            lazy::check_with_source(program, seeds, source_limits, selection, control, |chunk| {
                self.execute(chunk, limits, control, &mut transport)
            });
        if matches!(
            &result,
            Err(lazy::Failure {
                cause: lazy::Cause::InvalidOutput,
                ..
            })
        ) {
            self.runtime.invalidate();
        }
        result.map_err(|mut failure| {
            if let lazy::Cause::Execution(error) = &failure.cause
                && let Some(stop) = error.interruption
            {
                failure.cause = lazy::Cause::Source(stop);
            }
            failure
        })
    }

    fn execute(
        &mut self,
        chunk: &lazy::Chunk<'_>,
        limits: GpuLimits,
        control: &Control,
        transport: &mut Option<Transport>,
    ) -> Result<Vec<u32>, GpuError> {
        self.runtime.check_health()?;
        let mut plan = Plan::new(chunk, limits, &self.runtime.limits)?;
        let epoch = self.epoch.checked_add(1).ok_or_else(|| {
            GpuError::new(GpuErrorKind::Capacity, "lazy submission identity exhausted")
        })?;
        plan.epoch = epoch;
        self.epoch = epoch;
        let scopes = ErrorScopes::new(&self.runtime.device);
        let outcome = self.dispatch(chunk, limits, &plan, control, transport);
        let result = self.runtime.complete(scopes, outcome);
        if result.is_err() {
            // A failed or interrupted read can still have live device work.
            // The invalidated runtime must never reuse its mapped storage.
            *transport = None;
        }
        result
    }

    fn dispatch(
        &mut self,
        chunk: &lazy::Chunk<'_>,
        limits: GpuLimits,
        plan: &Plan,
        control: &Control,
        cached: &mut Option<Transport>,
    ) -> Result<Vec<u32>, GpuError> {
        let device = &self.runtime.device;
        let retained = cached
            .as_ref()
            .map(|transport| transport.capacity)
            .filter(|capacity| capacity.reusable(plan, limits.max_batch_bytes));
        let capacity = retained.unwrap_or(plan.capacity);
        let counters = self
            .statistics
            .submitted(plan, retained.is_some(), capacity)?;
        if retained.is_none() {
            // Drop bindings and all old buffers before allocating replacements;
            // active shapes alone never admit an oversized retained allocation.
            *cached = None;
        }
        let transport = cached.get_or_insert_with(|| Transport::new(&self.runtime, capacity));
        let submission = transport.submit(&self.runtime, chunk, plan);
        self.statistics = counters;
        let start = std::time::Instant::now();
        let result = runtime::read_polled(
            device,
            &transport.readback,
            submission,
            limits.timeout,
            || control.poll().map_err(GpuError::interrupted),
            |words| plan.decode(words),
        );
        self.statistics.host_wait = self
            .statistics
            .host_wait
            .checked_add(start.elapsed())
            .ok_or_else(|| {
                GpuError::new(GpuErrorKind::Capacity, "lazy host-wait duration overflow")
            })?;
        if result.is_ok() {
            self.statistics.downloaded_bytes = self
                .statistics
                .downloaded_bytes
                .checked_add(plan.result_bytes)
                .ok_or_else(|| {
                    GpuError::new(
                        GpuErrorKind::Capacity,
                        "lazy readback byte counter overflow",
                    )
                })?;
        }
        result
    }
}

impl LazyGpuStatistics {
    fn submitted(self, plan: &Plan, reused: bool, storage: Capacity) -> Result<Self, GpuError> {
        let capacity = || GpuError::new(GpuErrorKind::Capacity, "lazy submission counter overflow");
        Ok(Self {
            dispatches: self.dispatches.checked_add(1).ok_or_else(capacity)?,
            world_instances: self
                .world_instances
                .checked_add(u64::from(plan.dimensions[1]) * u64::from(plan.dimensions[2]))
                .ok_or_else(capacity)?,
            uploaded_bytes: self
                .uploaded_bytes
                .checked_add(plan.uploaded_bytes)
                .ok_or_else(capacity)?,
            transport_allocations: self
                .transport_allocations
                .checked_add(u64::from(!reused))
                .ok_or_else(capacity)?,
            transport_reuses: self
                .transport_reuses
                .checked_add(u64::from(reused))
                .ok_or_else(capacity)?,
            peak_transport_bytes: self
                .peak_transport_bytes
                .max(storage.bytes().ok_or_else(capacity)?),
            ..self
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{GpuErrorKind, GpuLimits};
    use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Seed, Template};
    use zetesis_cpu::{Control, lazy};

    pub(super) fn inspect(mut assertion: impl FnMut(&lazy::Chunk<'_>)) {
        let a = AtomPattern::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
        let program = Program::new(
            vec![Template::new(Some(a), vec![], vec![], vec![], vec![])],
            AdmissionLimits::default(),
        )
        .unwrap();
        let seeds = [Seed::new(&program, []).unwrap()];
        for selection in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
            lazy::check_with_source(
                &program,
                &seeds,
                lazy::Limits {
                    max_atoms: 33,
                    ..lazy::Limits::default()
                },
                selection,
                &Control::default(),
                |chunk| {
                    assertion(chunk);
                    lazy::evaluate(chunk)
                },
            )
            .unwrap();
        }
    }

    #[test]
    fn lazy_transport_bytes_have_an_exact_ceiling() {
        inspect(|chunk| {
            let device = wgpu::Limits::default();
            let plan = super::Plan::new(chunk, GpuLimits::default(), &device).unwrap();
            assert_eq!(plan.dimensions, [1, 1, 1, 1]);
            assert_eq!(plan.result_words, 4);
            let accounted = plan.uploaded_bytes * 2 + plan.result_bytes * 3;
            assert!(
                super::Plan::new(
                    chunk,
                    GpuLimits {
                        max_batch_bytes: accounted,
                        ..GpuLimits::default()
                    },
                    &device
                )
                .is_ok()
            );
            let error = super::Plan::new(
                chunk,
                GpuLimits {
                    max_batch_bytes: accounted - 1,
                    ..GpuLimits::default()
                },
                &device,
            )
            .err()
            .unwrap();
            assert_eq!(error.kind(), GpuErrorKind::Capacity);
        });
    }

    #[test]
    fn lazy_transport_checks_each_storage_binding() {
        inspect(|chunk| {
            let device = wgpu::Limits {
                max_storage_buffer_binding_size: 8,
                ..wgpu::Limits::default()
            };
            let error = super::Plan::new(chunk, GpuLimits::default(), &device)
                .err()
                .unwrap();
            assert_eq!(error.kind(), GpuErrorKind::Capacity);
        });
    }

    #[test]
    fn lazy_transport_refuses_excess_candidate_occurrences() {
        inspect(|chunk| {
            let error = super::Plan::new(
                chunk,
                GpuLimits {
                    max_candidates: 0,
                    ..GpuLimits::default()
                },
                &wgpu::Limits::default(),
            )
            .err()
            .unwrap();
            assert_eq!(error.kind(), GpuErrorKind::Capacity);
        });
    }

    #[test]
    fn lazy_submission_counters_refuse_overflow() {
        inspect(|chunk| {
            let plan =
                super::Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
            for statistics in [
                super::LazyGpuStatistics {
                    dispatches: u64::MAX,
                    ..Default::default()
                },
                super::LazyGpuStatistics {
                    world_instances: u64::MAX,
                    ..Default::default()
                },
                super::LazyGpuStatistics {
                    uploaded_bytes: u64::MAX,
                    ..Default::default()
                },
            ] {
                assert_eq!(
                    statistics
                        .submitted(&plan, false, plan.capacity)
                        .unwrap_err()
                        .kind(),
                    GpuErrorKind::Capacity
                );
            }
        });
    }

    #[test]
    fn lazy_readback_requires_submission_identity() {
        inspect(|chunk| {
            let plan =
                super::Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
            let mut output = vec![0; plan.result_words];
            let width = plan.dimensions[0] as usize;
            output[width + 2] = plan.epoch;
            assert_eq!(plan.decode(&output).unwrap(), vec![0; width + 1]);
            output[width + 2] = plan.epoch + 1;
            assert_eq!(
                plan.decode(&output).unwrap_err().kind(),
                GpuErrorKind::Readback
            );
        });
    }

    #[test]
    fn lazy_readback_requires_world_identity() {
        inspect(|chunk| {
            let plan =
                super::Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
            let mut output = vec![0; plan.result_words];
            let width = plan.dimensions[0] as usize;
            output[width + 1] = 1;
            output[width + 2] = plan.epoch;
            assert_eq!(
                plan.decode(&output).unwrap_err().kind(),
                GpuErrorKind::Readback
            );
        });
    }

    #[test]
    fn lazy_readback_requires_the_complete_output_shape() {
        inspect(|chunk| {
            let plan =
                super::Plan::new(chunk, GpuLimits::default(), &wgpu::Limits::default()).unwrap();
            assert_eq!(plan.decode(&[]).unwrap_err().kind(), GpuErrorKind::Readback);
        });
    }

    #[test]
    fn lazy_uniform_checks_its_complete_identity_header() {
        inspect(|chunk| {
            let exact = wgpu::Limits {
                max_uniform_buffer_binding_size: super::UNIFORM_BYTES,
                ..wgpu::Limits::default()
            };
            assert!(super::Plan::new(chunk, GpuLimits::default(), &exact).is_ok());
            let below = wgpu::Limits {
                max_uniform_buffer_binding_size: super::UNIFORM_BYTES - 1,
                ..exact
            };
            assert_eq!(
                super::Plan::new(chunk, GpuLimits::default(), &below)
                    .err()
                    .unwrap()
                    .kind(),
                GpuErrorKind::Capacity
            );
        });
    }

    #[test]
    fn lazy_transport_refuses_a_zero_wait_limit() {
        inspect(|chunk| {
            assert_eq!(
                super::Plan::new(
                    chunk,
                    GpuLimits {
                        timeout: std::time::Duration::ZERO,
                        ..GpuLimits::default()
                    },
                    &wgpu::Limits::default()
                )
                .err()
                .unwrap()
                .kind(),
                GpuErrorKind::Capacity
            );
        });
    }

    #[test]
    fn lazy_shader_validates_without_optional_capabilities() {
        let module = naga::front::wgsl::parse_str(super::SHADER).expect("lazy WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("lazy WGSL validates");
        assert_eq!(module.entry_points.len(), 1);
        assert_eq!(module.entry_points[0].workgroup_size, [64, 1, 1]);
    }
}

#[cfg(test)]
#[path = "../tests/lazy/transport.rs"]
mod transport_tests;
