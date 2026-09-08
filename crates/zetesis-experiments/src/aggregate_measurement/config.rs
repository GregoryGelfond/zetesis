use super::Error;
use crate::Backend;
use clap::{Args, ValueEnum};
use serde::{Serialize, Serializer};
use std::{num::NonZeroUsize, time::Duration};

const MAX_CASES: usize = 64;
const MAX_TUPLES: usize = 65_536;
const MAX_OCCURRENCES: usize = 256;
const MAX_WARMUPS: usize = 6;
const MAX_REPETITIONS: usize = 60;
const MAX_WORKERS: usize = 64;

/// Native retained operation, with no Boolean-lowering reconstruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Function {
    /// Count every selected whole key.
    Count,
    /// Sum selected numeric contributions.
    Sum,
    /// Sum positive numeric contributions; preserve neutral keys.
    SumPlus,
    /// Numeric minimum with genuine empty supremum.
    Min,
    /// Numeric maximum with genuine empty infimum.
    Max,
}
impl From<Function> for zetesis_ferraris::native_aggregate::Function {
    fn from(value: Function) -> Self {
        match value {
            Function::Count => Self::Count,
            Function::Sum => Self::Sum,
            Function::SumPlus => Self::SumPlus,
            Function::Min => Self::Min,
            Function::Max => Self::Max,
        }
    }
}

/// One complete deterministic group and its ordered acquired occurrences.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Case {
    /// Native operation to compare.
    pub function: Function,
    /// Complete distinct tuple keys, zero through 65,536.
    pub tuples: usize,
    /// Repeated original/frozen observations, one through 256.
    pub occurrences: NonZeroUsize,
}

/// Bounded schedule and explicit independent operation ceilings.
#[derive(Clone, Debug, Serialize)]
pub struct Configuration {
    /// One through 64 cases, preserving repeated cases and caller order.
    pub cases: Vec<Case>,
    /// Hard physical API, or an explicit CPU-only experiment.
    #[serde(serialize_with = "backend")]
    pub backend: Backend,
    /// Preparation observations per route, zero through six.
    pub warmups: usize,
    /// Timed observations per route, one through sixty.
    pub repetitions: NonZeroUsize,
    /// Independently owned Rayon pool, one through 64 workers.
    pub workers: NonZeroUsize,
    /// Per-occurrence formula acquisition work, excluded from reduction clocks.
    pub max_acquisition_work: u64,
    /// Per-occurrence simultaneous acquisition scratch/mask payload.
    pub max_acquisition_bytes: u64,
    /// Per-occurrence exact native reference work, outside sample clocks.
    pub max_reference_work: u64,
    /// Per-occurrence scalar/Rayon reduction work, including failed attempts.
    pub max_reduction_work: u64,
    /// Per-batch prospective authored GPU/host payload after eviction; not RSS.
    pub max_gpu_bytes: u64,
    /// Per-batch GPU host packing/readback work allowance.
    pub max_gpu_host_work: u64,
    /// Per-batch complete device reduction/guard work allowance.
    pub max_gpu_device_work: u64,
    /// Device submission/readback wait; pipeline/setup excluded.
    #[serde(rename = "gpu_timeout_ns", serialize_with = "duration")]
    pub gpu_timeout: Duration,
}
impl Configuration {
    pub(super) fn validate(&self) -> Result<(), Error> {
        if !(1..=MAX_CASES).contains(&self.cases.len()) {
            return Err(Error::Configuration("require one through 64 cases"));
        }
        if self
            .cases
            .iter()
            .any(|case| case.tuples > MAX_TUPLES || case.occurrences.get() > MAX_OCCURRENCES)
        {
            return Err(Error::Configuration(
                "require at most 65,536 tuples and 256 occurrences",
            ));
        }
        if self.warmups > MAX_WARMUPS
            || self.repetitions.get() > MAX_REPETITIONS
            || self.workers.get() > MAX_WORKERS
            || self.gpu_timeout.is_zero()
        {
            return Err(Error::Configuration(
                "require at most six warmups, sixty repetitions, 64 workers and a positive device wait",
            ));
        }
        Ok(())
    }
    pub(super) fn gpu_limits(&self) -> zetesis_wgpu::AggregateGpuLimits {
        zetesis_wgpu::AggregateGpuLimits {
            max_occurrences: MAX_OCCURRENCES,
            max_batch_bytes: self.max_gpu_bytes,
            max_host_work: self.max_gpu_host_work,
            max_device_work: self.max_gpu_device_work,
            timeout: self.gpu_timeout,
        }
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde serialize_with receives borrowed fields"
)]
fn backend<S: Serializer>(value: &Backend, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(value.label())
}
fn duration<S: Serializer>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u128(value.as_nanos())
}

/// Installed command view of the complete numeric reduction experiment.
#[derive(Clone, Debug, Args)]
pub struct Options {
    /// Require actual Metal/Vulkan or explicitly run the two CPU routes.
    #[arg(long, value_enum, default_value_t)]
    pub backend: Backend,
    /// Complete tuple counts; zero exercises genuine empty extrema.
    #[arg(long, value_delimiter = ',', default_value = "0,64,4096")]
    pub tuples: Vec<usize>,
    /// Ordered acquired occurrence counts.
    #[arg(long, value_delimiter = ',', default_value = "1,32,128")]
    pub batches: Vec<NonZeroUsize>,
    /// Exact retained native operations.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "count,sum,sum-plus,min,max"
    )]
    pub functions: Vec<Function>,
    /// Untimed-population warmup observations per route.
    #[arg(long, default_value_t = 2)]
    pub warmups: usize,
    /// Twelve balances both two-route and four-route rotations.
    #[arg(long, default_value = "12")]
    pub repetitions: NonZeroUsize,
    /// Independently owned Rayon workers.
    #[arg(long, default_value = "4")]
    pub workers: NonZeroUsize,
    /// Independent work ceilings for acquisition, reference, CPU and device work.
    #[arg(long, default_value_t = 100_000_000)]
    pub max_work: u64,
}
impl Options {
    /// Validate the product before allocating its bounded case list.
    /// Native admission/numeric preparation retain their published default limits.
    ///
    /// # Errors
    /// Refuses empty/oversized products and dimensions outside the finite scope.
    pub fn configuration(&self) -> Result<Configuration, Error> {
        let count = self
            .tuples
            .len()
            .checked_mul(self.batches.len())
            .and_then(|count| count.checked_mul(self.functions.len()))
            .filter(|count| (1..=MAX_CASES).contains(count))
            .ok_or(Error::Configuration("require one through 64 cases"))?;
        let mut cases = super::reserve(count)?;
        for &function in &self.functions {
            for &tuples in &self.tuples {
                for &occurrences in &self.batches {
                    cases.push(Case {
                        function,
                        tuples,
                        occurrences,
                    });
                }
            }
        }
        let configuration = Configuration {
            cases,
            backend: self.backend,
            warmups: self.warmups,
            repetitions: self.repetitions,
            workers: self.workers,
            max_acquisition_work: self.max_work,
            max_acquisition_bytes: 64 * 1024 * 1024,
            max_reference_work: self.max_work,
            max_reduction_work: self.max_work,
            max_gpu_bytes: 128 * 1024 * 1024,
            max_gpu_host_work: self.max_work,
            max_gpu_device_work: self.max_work,
            gpu_timeout: Duration::from_secs(30),
        };
        configuration.validate()?;
        Ok(configuration)
    }
}
