//! Useful per-world device inference over bounded lazy source chunks.
//!
//! The CPU source coordinator offers instances from the union snapshot. The
//! kernel independently checks each world's positives and frozen gates and
//! derives its head delta. No completed CPU closure or complete ground graph is
//! uploaded. One batch reuses bounded transport capacity across chunks/rounds;
//! immutable input prefixes are reused only within their batch/round and layout.
//! Every active output is cleared before dispatch. Automatic
//! performance selection remains a separate policy.

use std::borrow::Cow;
use std::time::Duration;

use zetesis_core::{Program, Seed, SeedView};
use zetesis_cpu::{Control, lazy};

use crate::runtime::{self, DeviceProfile, ErrorScopes, Runtime};
use crate::{GpuError, GpuErrorKind, GpuInfo, GpuLimits, GpuOptions, GpuSelection};

mod plan;
mod transport;
mod statistics;
mod upload;

pub use statistics::{LazyBufferUsage, LazyTransportReplacements, LazyTransportUsage};

use plan::{Capacity, Plan, Selection};
use transport::Transport;

const SHADER: &str = include_str!("lazy.wgsl");
const DIMENSION_WORDS: usize = 3;
const UNIFORM_BYTES: u64 = 16;
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
    /// Submitted chunks requesting at least one new transport buffer.
    /// This counts allocation requests, including a later device failure.
    pub transport_allocations: u64,
    /// Submitted chunks reusing every transport buffer from an earlier chunk.
    pub transport_reuses: u64,
    /// Largest requested GPU buffer payload for a submitted chunk, including
    /// retained inactive capacity. Excludes host payload and driver allocations.
    pub peak_transport_bytes: u64,
    /// Overlapping reasons why submitted chunks required fresh transport.
    pub transport_replacements: LazyTransportReplacements,
    /// Per-buffer allocation/reuse requests and prospective slack releases.
    pub transport_usage: LazyTransportUsage,
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
        let runtime = pollster::block_on(Runtime::new(options, selection, Self::profile()))?;
        Ok(Self::with_runtime(runtime))
    }

    /// Compile this primitive on an existing device context.
    ///
    /// Prepared subjects belong to this primitive; device health and execution
    /// serialization are shared with all primitives using the context.
    ///
    /// # Errors
    /// Refuses an active or invalidated context, unsupported granted limits,
    /// allocation and shader validation failure. No adapter selection occurs.
    pub fn from_context(context: &crate::GpuContext) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::from_context(context, Self::profile()))?;
        Ok(Self::with_runtime(runtime))
    }

    /// Exact device context retained by this primitive.
    #[must_use]
    pub fn context(&self) -> &crate::GpuContext {
        &self.runtime.context
    }

    fn with_runtime(runtime: Runtime) -> Self {
        Self {
            runtime,
            statistics: LazyGpuStatistics::default(),
            epoch: 0,
        }
    }

    fn profile() -> DeviceProfile {
        DeviceProfile {
            device_label: "zetesis lazy reduct device",
            shader_label: "zetesis lazy consequence shader",
            pipeline_label: "zetesis lazy consequence pipeline",
            shader: Cow::Borrowed(SHADER),
            entry_point: "consequence",
            validate_limits: check_limits,
        }
    }

    /// Selected physical adapter metadata; constant-time borrow.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.runtime.context.info()
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
    /// `source_limits.max_host_bytes`. Rejected bindings and buffers are dropped
    /// before allocating replacements; fitting inputs, the uniform and exactly
    /// matching result buffers can survive. Input slack is released when needed
    /// to preserve admission of the exact shape. All buffers are dropped on each
    /// batch exit; driver-private deferred retirement is not counted.
    /// Frozen seeds are uploaded once per retained layout; snapshots once per
    /// retained layout and immutable source round. Replaced input buffers need
    /// fresh writes. Chunk records, offsets, uniform and cleared output remain
    /// per dispatch. Round indices have meaning only in this batch-owned scope.
    ///
    /// # Errors
    /// Returns no completed check on interrupted source coverage, capacity,
    /// device, timeout, readback or malformed-output failure. Charged source
    /// progress is retained in the error, device work in [`Self::statistics`].
    /// The whole batch leases the shared context, including source validation.
    /// Busy contention returns before source work; execution-stage failures
    /// invalidate the context for every primitive sharing it.
    /// After leasing, cancellation is checked before context health. A known
    /// invalidated context refuses before source admission, including empty calls.
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
        self.check_batch_with_source_views(
            program,
            seeds.iter().map(Seed::view),
            source_limits,
            limits,
            selection,
            control,
        )
    }

    /// Check borrowed candidate occurrences through the same lazy source rounds.
    /// No owned seed or intermediate view vector is materialized. The iterator
    /// preserves exact input order and must be replayable for source admission
    /// and final seed/closure agreement. Device work starts only for emitted
    /// source chunks; constructing a view does not compile a static graph.
    ///
    /// # Errors
    /// Preserves [`Self::check_batch`]'s failure and incomplete-coverage contract.
    pub fn check_batch_views<'seed>(
        &mut self,
        program: &Program,
        seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
        source_limits: lazy::Limits,
        limits: GpuLimits,
        control: &Control,
    ) -> Result<lazy::Batch, lazy::Failure<GpuError>> {
        self.check_batch_with_source_views(
            program,
            seeds,
            source_limits,
            limits,
            lazy::SourceSelection::Union,
            control,
        )
    }

    /// Check borrowed candidate occurrences using explicit host source selection.
    /// The implementation and device protocol are shared with
    /// [`Self::check_batch_with_source`]; views change input ownership only.
    ///
    /// # Errors
    /// Preserves [`Self::check_batch_with_source`]'s failure and coverage contract.
    pub fn check_batch_with_source_views<'seed>(
        &mut self,
        program: &Program,
        seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
        source_limits: lazy::Limits,
        limits: GpuLimits,
        selection: lazy::SourceSelection,
        control: &Control,
    ) -> Result<lazy::Batch, lazy::Failure<GpuError>> {
        self.statistics = LazyGpuStatistics::default();
        let context = self.runtime.context.clone();
        let _lease = context.lease().map_err(|error| lazy::Failure {
            cause: lazy::Cause::Execution(error),
            progress: lazy::Progress::default(),
        })?;
        control.poll().map_err(|stop| lazy::Failure {
            cause: lazy::Cause::Source(stop),
            progress: lazy::Progress::default(),
        })?;
        context.check_health().map_err(|error| lazy::Failure {
            cause: lazy::Cause::Execution(error),
            progress: lazy::Progress::default(),
        })?;
        let mut transport = None;
        let result = lazy::check_with_source_views(
            program,
            seeds,
            source_limits,
            selection,
            control,
            |chunk| self.execute(chunk, limits, control, &mut transport),
        );
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

    // The complete public batch retains the context lease, including source
    // validation after this chunk returns. Direct transport tests own an isolated
    // executor and exercise this internal step without another context client.
    fn execute(
        &mut self,
        chunk: &lazy::Chunk<'_>,
        limits: GpuLimits,
        control: &Control,
        transport: &mut Option<Transport>,
    ) -> Result<Vec<u32>, GpuError> {
        self.runtime.check_health()?;
        let epoch = crate::packing::next_epoch(self.epoch)?;
        let plan = Plan::new(epoch, chunk, limits, self.runtime.limits())?;
        self.epoch = epoch.get();
        let scopes = ErrorScopes::new(self.runtime.device());
        let outcome = self.dispatch(chunk, limits, &plan, control, transport);
        let result = self.runtime.complete(scopes, outcome);
        if result.is_err() {
            // Discard this transport after any failed read. A pending wait also
            // invalidates the shared context; a settled caller stop need not.
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
    ) -> Result<runtime::Completion<Vec<u32>>, GpuError> {
        let device = self.runtime.device();
        let mut selection = Selection::new(
            cached.as_ref().map(|transport| transport.capacity),
            plan,
            limits.max_batch_bytes,
        );
        selection.uploads = upload::Uploads::needed(
            cached.as_ref().and_then(|transport| transport.upload),
            chunk.into(),
            selection.retention,
        );
        let counters = self.statistics.submitted(plan, selection)?;
        if !selection.transition.is_reuse() {
            *cached = Some(cached.take().map_or_else(
                || Transport::new(&self.runtime, selection.capacity),
                |previous| previous.replace(&self.runtime, selection),
            ));
        }
        let transport = cached.as_mut().ok_or_else(|| {
            GpuError::new(GpuErrorKind::Device, "lazy transport is not initialized")
        })?;
        let submission = transport.submit(&self.runtime, chunk, plan, selection.uploads);
        self.statistics = counters;
        let start = std::time::Instant::now();
        let result = runtime::read_polled(
            device,
            transport.readback(),
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
        Ok(result)
    }
}

impl LazyGpuStatistics {
    fn submitted(self, plan: &Plan, selection: Selection) -> Result<Self, GpuError> {
        let capacity = || GpuError::new(GpuErrorKind::Capacity, "lazy submission counter overflow");
        Ok(Self {
            dispatches: self.dispatches.checked_add(1).ok_or_else(capacity)?,
            world_instances: self
                .world_instances
                .checked_add(u64::from(plan.dimensions[1]) * u64::from(plan.dimensions[2]))
                .ok_or_else(capacity)?,
            uploaded_bytes: self
                .uploaded_bytes
                .checked_add(selection.uploads.bytes(plan)?)
                .ok_or_else(capacity)?,
            transport_allocations: self
                .transport_allocations
                .checked_add(u64::from(!selection.transition.is_reuse()))
                .ok_or_else(capacity)?,
            transport_reuses: self
                .transport_reuses
                .checked_add(u64::from(selection.transition.is_reuse()))
                .ok_or_else(capacity)?,
            peak_transport_bytes: self
                .peak_transport_bytes
                .max(selection.capacity.bytes().ok_or_else(capacity)?),
            transport_replacements: self.transport_replacements.record(selection.transition)?,
            transport_usage: self.transport_usage.record(selection)?,
            ..self
        })
    }
}

fn check_limits(limits: &wgpu::Limits) -> Result<(), GpuError> {
    // The immutable-round shader has no workgroup variables. Its five storage
    // buffers and uniform are independent of the static closure kernel's scratch.
    for (name, required, available) in [
        (
            "workgroup invocations",
            64,
            limits.max_compute_invocations_per_workgroup,
        ),
        ("workgroup width", 64, limits.max_compute_workgroup_size_x),
        (
            "storage bindings",
            5,
            limits.max_storage_buffers_per_shader_stage,
        ),
        (
            "uniform bindings",
            1,
            limits.max_uniform_buffers_per_shader_stage,
        ),
        ("bindings per group", 6, limits.max_bindings_per_bind_group),
    ] {
        if available < required {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                format!("lazy {name}: need {required}, device provides {available}"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/lazy/capabilities.rs"]
mod capability_tests;

#[cfg(test)]
mod tests {
    use crate::{GpuErrorKind, GpuLimits};
    use std::num::NonZeroU32;
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
            let plan =
                super::Plan::new(NonZeroU32::MIN, chunk, GpuLimits::default(), &device).unwrap();
            assert_eq!(plan.dimensions, [1, 1, 1]);
            assert_eq!(plan.result_words, 4);
            // Four live uniform words, one offset, one four-word source record,
            // one snapshot word and one seed word: 44 uploaded bytes. Retained
            // buffers plus host packing and decoded output total 136 bytes.
            assert_eq!(plan.uploaded_bytes, 44);
            assert_eq!(plan.capacity.accounted(&plan), Some(136));
            let accounted = plan.uploaded_bytes * 2 + plan.result_bytes * 3;
            assert!(
                super::Plan::new(
                    NonZeroU32::MIN,
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
                NonZeroU32::MIN,
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
            let error = super::Plan::new(NonZeroU32::MIN, chunk, GpuLimits::default(), &device)
                .err()
                .unwrap();
            assert_eq!(error.kind(), GpuErrorKind::Capacity);
        });
    }

    #[test]
    fn lazy_transport_refuses_excess_candidate_occurrences() {
        inspect(|chunk| {
            let error = super::Plan::new(
                NonZeroU32::MIN,
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
            let plan = super::Plan::new(
                NonZeroU32::MIN,
                chunk,
                GpuLimits::default(),
                &wgpu::Limits::default(),
            )
            .unwrap();
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
                        .submitted(&plan, super::Selection::new(None, &plan, u64::MAX))
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
            let plan = super::Plan::new(
                NonZeroU32::MIN,
                chunk,
                GpuLimits::default(),
                &wgpu::Limits::default(),
            )
            .unwrap();
            let mut output = vec![0; plan.result_words];
            let width = plan.dimensions[0] as usize;
            output[width + 2] = plan.epoch.get();
            assert_eq!(plan.decode(&output).unwrap(), vec![0; width + 1]);
            output[width + 2] = plan.epoch.get() + 1;
            assert_eq!(
                plan.decode(&output).unwrap_err().kind(),
                GpuErrorKind::Readback
            );
        });
    }

    #[test]
    fn lazy_readback_requires_world_identity() {
        inspect(|chunk| {
            let plan = super::Plan::new(
                NonZeroU32::MIN,
                chunk,
                GpuLimits::default(),
                &wgpu::Limits::default(),
            )
            .unwrap();
            let mut output = vec![0; plan.result_words];
            let width = plan.dimensions[0] as usize;
            output[width + 1] = 1;
            output[width + 2] = plan.epoch.get();
            assert_eq!(
                plan.decode(&output).unwrap_err().kind(),
                GpuErrorKind::Readback
            );
        });
    }

    #[test]
    fn lazy_readback_requires_the_complete_output_shape() {
        inspect(|chunk| {
            let plan = super::Plan::new(
                NonZeroU32::MIN,
                chunk,
                GpuLimits::default(),
                &wgpu::Limits::default(),
            )
            .unwrap();
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
            assert!(super::Plan::new(NonZeroU32::MIN, chunk, GpuLimits::default(), &exact).is_ok());
            let below = wgpu::Limits {
                max_uniform_buffer_binding_size: super::UNIFORM_BYTES - 1,
                ..exact
            };
            assert_eq!(
                super::Plan::new(NonZeroU32::MIN, chunk, GpuLimits::default(), &below)
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
                    NonZeroU32::MIN,
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

#[cfg(test)]
#[path = "../tests/lazy/replacement.rs"]
mod replacement_tests;

#[cfg(test)]
#[path = "../tests/lazy/retention.rs"]
mod retention_tests;
