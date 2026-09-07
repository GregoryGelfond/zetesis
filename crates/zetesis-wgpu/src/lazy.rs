//! Useful per-world device inference over bounded lazy source chunks.
//!
//! The CPU source coordinator offers instances from the union snapshot. The
//! kernel independently checks each world's positives and frozen gates and
//! derives its head delta. No completed CPU closure or complete ground graph is
//! uploaded. Transport is deliberately transient per chunk; residency and an
//! automatic performance policy require later measurement.

use std::borrow::Cow;
use std::time::Duration;

use zetesis_core::{Program, Seed};
use zetesis_cpu::{Control, lazy};

use crate::runtime::{self, DeviceProfile, ErrorScopes, Runtime};
use crate::{GpuError, GpuErrorKind, GpuInfo, GpuLimits, GpuOptions, GpuSelection};

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
    /// Completed host time waiting for submissions and decoding readback.
    pub host_wait: Duration,
}

/// Physical-device executor for the admitted relational lazy source profile.
/// It owns the existing shared wgpu lifecycle and retains no cross-batch atoms,
/// snapshots, completed closures or instance chunks.
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
    /// all current GPU buffers plus host packing and decoded words; coordinator
    /// storage is separately bounded by `source_limits.max_host_bytes`.
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
        self.statistics = LazyGpuStatistics::default();
        let result = lazy::check_with(program, seeds, source_limits, control, |chunk| {
            self.execute(chunk, limits, control)
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
    ) -> Result<Vec<u32>, GpuError> {
        self.runtime.check_health()?;
        let mut plan = Plan::new(chunk, limits, &self.runtime.limits)?;
        let epoch = self.epoch.checked_add(1).ok_or_else(|| {
            GpuError::new(GpuErrorKind::Capacity, "lazy submission identity exhausted")
        })?;
        plan.epoch = epoch;
        self.epoch = epoch;
        let scopes = ErrorScopes::new(&self.runtime.device);
        let outcome = self.dispatch(chunk, limits, &plan, control);
        self.runtime.complete(scopes, outcome)
    }

    fn dispatch(
        &mut self,
        chunk: &lazy::Chunk<'_>,
        limits: GpuLimits,
        plan: &Plan,
        control: &Control,
    ) -> Result<Vec<u32>, GpuError> {
        let device = &self.runtime.device;
        let transport = Transport::new(&self.runtime, chunk, plan);
        let counters = self.statistics.submitted(plan)?;
        let submission = runtime::submit(
            device,
            &self.runtime.queue,
            &runtime::Dispatch {
                command_label: "lazy source consequence command",
                pass_label: "lazy world consequences",
                pipeline: &self.runtime.pipeline,
                group: &transport.group,
                worlds: plan.dimensions[2],
                result: &transport.output,
                readback: &transport.readback,
                result_bytes: plan.result_bytes,
            },
        );
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

struct Plan {
    dimensions: [u32; DIMENSION_WORDS],
    result_words: usize,
    result_bytes: u64,
    uploaded_bytes: u64,
    epoch: u32,
}

struct Transport {
    group: wgpu::BindGroup,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
}

impl Transport {
    fn new(runtime: &Runtime, chunk: &lazy::Chunk<'_>, plan: &Plan) -> Self {
        let device = &runtime.device;
        let uniform = runtime::initialized(
            device,
            "lazy dimensions",
            &[
                plan.dimensions[0],
                plan.dimensions[1],
                plan.dimensions[2],
                plan.dimensions[3],
                plan.epoch,
                0,
                0,
                0,
            ],
            wgpu::BufferUsages::UNIFORM,
        );
        let offsets = runtime::initialized(
            device,
            "lazy source offsets",
            chunk.offsets(),
            wgpu::BufferUsages::STORAGE,
        );
        let records = runtime::initialized(
            device,
            "lazy source instances",
            chunk.records(),
            wgpu::BufferUsages::STORAGE,
        );
        let snapshots = runtime::initialized(
            device,
            "lazy immutable snapshots",
            chunk.snapshots(),
            wgpu::BufferUsages::STORAGE,
        );
        let seeds = runtime::initialized(
            device,
            "lazy frozen seeds",
            chunk.seeds(),
            wgpu::BufferUsages::STORAGE,
        );
        // WebGPU guarantees fresh buffer contents are zero initialized. The bind
        // group retains its input buffers after these local handles are dropped.
        let output = runtime::buffer(
            device,
            "lazy head delta",
            plan.result_bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let readback = runtime::buffer(
            device,
            "lazy delta readback",
            plan.result_bytes,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lazy round chunk"),
            layout: &runtime.pipeline.get_bind_group_layout(0),
            entries: &[
                runtime::entry(0, &uniform),
                runtime::entry(1, &offsets),
                runtime::entry(2, &records),
                runtime::entry(3, &snapshots),
                runtime::entry(4, &seeds),
                runtime::entry(5, &output),
            ],
        });
        Self {
            group,
            output,
            readback,
        }
    }
}

impl LazyGpuStatistics {
    fn submitted(self, plan: &Plan) -> Result<Self, GpuError> {
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
            ..self
        })
    }
}

impl Plan {
    fn new(
        chunk: &lazy::Chunk<'_>,
        limits: GpuLimits,
        device: &wgpu::Limits,
    ) -> Result<Self, GpuError> {
        let capacity = || {
            GpuError::new(
                GpuErrorKind::Capacity,
                "lazy chunk exceeds checked host/device dimensions",
            )
        };
        let dimensions = [
            chunk.words(),
            chunk.offsets().len(),
            chunk.worlds(),
            chunk.catalog_atoms(),
        ]
        .map(u32::try_from);
        let [words, rules, candidates, atoms] = dimensions;
        let dimensions = [
            words.map_err(|_| capacity())?,
            rules.map_err(|_| capacity())?,
            candidates.map_err(|_| capacity())?,
            atoms.map_err(|_| capacity())?,
        ];
        u32::try_from(chunk.records().len()).map_err(|_| capacity())?;
        dimensions[1]
            .checked_add(crate::WORKGROUP_SIZE)
            .ok_or_else(capacity)?;
        if chunk.worlds() > limits.max_candidates
            || dimensions[2] > device.max_compute_workgroups_per_dimension
        {
            return Err(capacity());
        }
        let result_words = chunk
            .words()
            .checked_add(RESULT_METADATA_WORDS)
            .and_then(|n| n.checked_mul(chunk.worlds()))
            .ok_or_else(capacity)?;
        u32::try_from(result_words).map_err(|_| capacity())?;
        let bytes = |count: usize| {
            u64::try_from(count)
                .ok()
                .and_then(|n| n.checked_mul(4))
                .ok_or_else(capacity)
        };
        let result_bytes = bytes(result_words)?;
        let sizes = [
            bytes(chunk.offsets().len())?,
            bytes(chunk.records().len())?,
            bytes(chunk.snapshots().len())?,
            bytes(chunk.seeds().len())?,
            result_bytes,
        ];
        for size in sizes {
            if size > device.max_storage_buffer_binding_size || size > device.max_buffer_size {
                return Err(capacity());
            }
        }
        if UNIFORM_BYTES > device.max_uniform_buffer_binding_size
            || UNIFORM_BYTES > device.max_buffer_size
        {
            return Err(capacity());
        }
        if limits.timeout.is_zero() {
            return Err(capacity());
        }
        let uploaded_bytes = sizes[..4]
            .iter()
            .try_fold(UNIFORM_BYTES, |sum, size| sum.checked_add(*size))
            .ok_or_else(capacity)?;
        // GPU input/output/readback, host input packing and decoded output.
        let accounted = uploaded_bytes
            .checked_mul(2)
            .and_then(|n| result_bytes.checked_mul(3).and_then(|r| n.checked_add(r)))
            .ok_or_else(capacity)?;
        if accounted > limits.max_batch_bytes {
            return Err(capacity());
        }
        Ok(Self {
            dimensions,
            result_words,
            result_bytes,
            uploaded_bytes,
            epoch: 1,
        })
    }

    fn decode(&self, words: &[u32]) -> Result<Vec<u32>, GpuError> {
        let malformed = || {
            GpuError::new(
                GpuErrorKind::Readback,
                "lazy delta shape or submission/world identity mismatch",
            )
        };
        if words.len() != self.result_words {
            return Err(malformed());
        }
        let width = self.dimensions[0] as usize;
        let mut decoded = Vec::new();
        let decoded_words = (width + 1) * self.dimensions[2] as usize;
        decoded
            .try_reserve_exact(decoded_words)
            .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
        for (world, record) in words
            .chunks_exact(width + RESULT_METADATA_WORDS)
            .enumerate()
        {
            if record[width + 1] as usize != world || record[width + 2] != self.epoch {
                return Err(malformed());
            }
            decoded.extend_from_slice(&record[..=width]);
        }
        Ok(decoded)
    }
}

#[cfg(test)]
mod tests {
    use crate::{GpuErrorKind, GpuLimits};
    use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Seed, Template};
    use zetesis_cpu::{Control, lazy};

    fn inspect(mut assertion: impl FnMut(&lazy::Chunk<'_>)) {
        let a = AtomPattern::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
        let program = Program::new(
            vec![Template::new(Some(a), vec![], vec![], vec![], vec![])],
            AdmissionLimits::default(),
        )
        .unwrap();
        let seeds = [Seed::new(&program, []).unwrap()];
        lazy::check_with(
            &program,
            &seeds,
            lazy::Limits {
                max_atoms: 33,
                ..lazy::Limits::default()
            },
            &Control::default(),
            |chunk| {
                assertion(chunk);
                lazy::evaluate(chunk)
            },
        )
        .unwrap();
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
                    statistics.submitted(&plan).unwrap_err().kind(),
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
