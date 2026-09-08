//! Exact numeric aggregate reduction over acquired original/frozen eligibility.

mod device;
mod packing;
mod preparation;
mod transport;

use crate::{GpuError, GpuErrorKind};
use std::{fmt, time::Duration};
use zetesis_cpu::{Control, Stop};

pub use device::GpuAggregateOracle;
pub use preparation::AggregateGpuPlan;

const SHADER: &str = include_str!("reduce.wgsl");
const LANES: u32 = 64;
const PARAM_BYTES: u64 = 32;
const RESULT_WORDS: usize = 10;

/// Explicit numeric capability limitation, never a source or semantic refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateGpuCapability {
    /// A selected extrema contribution could be a nonnumeric ASP term.
    NonNumericExtremum {
        /// Retained tuple occurrence.
        tuple: usize,
    },
    /// A guard compares with a nonnumeric ASP term.
    NonNumericGuard {
        /// Retained guard occurrence.
        guard: usize,
    },
    /// An integer guard is outside this kernel's exact i32 carrier.
    GuardRange {
        /// Retained guard occurrence.
        guard: usize,
    },
    /// A count's complete tuple carrier exceeds `i32::MAX`.
    CountRange {
        /// Complete tuple count.
        tuples: usize,
    },
    /// A positive carrier prefix exceeds `i32::MAX`, independent of cancellation.
    PositiveSum {
        /// Exact positive prefix at the first failed check.
        total: i64,
    },
    /// A negative carrier prefix is below `i32::MIN`, independent of cancellation.
    NegativeSum {
        /// Exact negative prefix at the first failed check.
        total: i64,
    },
}

/// Incomplete primitive operation; no variant is an aggregate truth value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateGpuError {
    /// The exact CPU operation remains applicable; this device representation is not.
    Capability(AggregateGpuCapability),
    /// Shared caller cancellation or deadline was observed.
    Stopped(Stop),
    /// Identity, capacity, allocation, adapter, execution or readback failure.
    Gpu(GpuError),
}

impl fmt::Display for AggregateGpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(value) => write!(f, "numeric aggregate GPU capability: {value:?}"),
            Self::Stopped(value) => value.fmt(f),
            Self::Gpu(value) => value.fmt(f),
        }
    }
}
impl std::error::Error for AggregateGpuError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capability(_) => None,
            Self::Stopped(value) => Some(value),
            Self::Gpu(value) => Some(value),
        }
    }
}
impl From<GpuError> for AggregateGpuError {
    fn from(error: GpuError) -> Self {
        match error.interruption {
            Some(stop) => Self::Stopped(stop),
            None => Self::Gpu(error),
        }
    }
}

/// Host-only numeric preparation ceilings; independent of formula acquisition.
#[derive(Clone, Copy, Debug)]
pub struct AggregateGpuPlanLimits {
    /// Complete tuple occurrences, including neutral keys.
    pub max_tuples: usize,
    /// Guard occurrences, preserving their declared order.
    pub max_guards: usize,
    /// Requested packed tuple/guard payload, including empty-buffer padding.
    /// Borrowed Group/Theory, Arc headers and allocator overhead are excluded.
    pub max_bytes: u64,
    /// One charged visit per tuple and guard.
    pub max_work: u64,
}
impl Default for AggregateGpuPlanLimits {
    fn default() -> Self {
        Self {
            max_tuples: 65_536,
            max_guards: 64,
            max_bytes: 64 * 1024 * 1024,
            max_work: 100_000_000,
        }
    }
}

/// One exact numeric reduction batch, separate from outer search and acquisition.
#[derive(Clone, Copy, Debug)]
pub struct AggregateGpuLimits {
    /// Ordered eligibility occurrences, also bounded by granted dispatch limits.
    pub max_occurrences: usize,
    /// Prospective authored host wire, GPU group/transport, packed masks, mapped
    /// readback allowance and returned records after planned cache eviction. Incoming residency is
    /// reported separately; caller masks/Group, allocator/driver overhead and
    /// deferred retirement are excluded. This is not total live memory or RSS.
    pub max_batch_bytes: u64,
    /// Group identity checks, mask-word initialization, tuple-mask packing,
    /// decoded records and two guard comparisons per guard per occurrence.
    pub max_host_work: u64,
    /// Two complete tuple/guard scans and two 63-combine reduction trees per
    /// occurrence, including the unused frozen phase of original-only records.
    pub max_device_work: u64,
    /// Submission/readback wait with shared control polling. This does not bound
    /// initialization, allocation, compilation or error-scope drainage.
    pub timeout: Duration,
}
impl Default for AggregateGpuLimits {
    fn default() -> Self {
        Self {
            max_occurrences: 1024,
            max_batch_bytes: 128 * 1024 * 1024,
            max_host_work: 100_000_000,
            max_device_work: 100_000_000,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Exact numeric measure or genuine empty-extremum endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateGpuValue {
    /// Checked integer carrier; no floating-point reduction occurs.
    Integer(i32),
    /// Empty maximum, `#inf`.
    Infimum,
    /// Empty minimum, `#sup`.
    Supremum,
}

/// Complete measure and conjunction of the retained guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateGpuEvaluation {
    value: AggregateGpuValue,
    holds: bool,
}
impl AggregateGpuEvaluation {
    /// Computed numeric measure or exact empty endpoint.
    #[must_use]
    pub const fn value(self) -> AggregateGpuValue {
        self.value
    }
    /// True exactly when all retained guards hold.
    #[must_use]
    pub const fn holds(self) -> bool {
        self.holds
    }
}

/// One ordered acquired eligibility occurrence; no stable-membership claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateGpuReduction {
    original: AggregateGpuEvaluation,
    frozen: Option<AggregateGpuEvaluation>,
}
impl AggregateGpuReduction {
    /// Original element eligibility reduced in M.
    #[must_use]
    pub const fn original(self) -> AggregateGpuEvaluation {
        self.original
    }
    /// Element eligibility frozen in M and evaluated in J, if acquired.
    #[must_use]
    pub const fn frozen(self) -> Option<AggregateGpuEvaluation> {
        self.frozen
    }
    /// Original guard truth conjoined with frozen guard truth. This is an
    /// aggregate suboperation; head permission and source completeness remain external.
    #[must_use]
    pub fn reduct_truth(self) -> Option<bool> {
        self.frozen.map(|value| self.original.holds && value.holds)
    }
}

/// Authored activity from the latest attempt, retained after a later failure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregateGpuActivity {
    /// Actual compute queue submissions.
    pub submissions: u64,
    /// Ordered occurrences included in submitted work.
    pub submitted_occurrences: u64,
    /// Reserved complete-scan device work in submitted commands.
    pub scheduled_work: u64,
    /// All occurrences, committed only after the entire readback vector validates.
    /// A later malformed record leaves this zero; no partial truth vector escapes.
    pub completed_occurrences: u64,
    /// Complete-batch work committed with `completed_occurrences`.
    pub completed_work: u64,
    /// Authored buffer initialization and queue-write payload.
    pub uploaded_bytes: u64,
    /// Complete readback payload, committed only after every record validates.
    pub downloaded_bytes: u64,
}

/// Successful nonempty batch resource observations, not driver-memory receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateGpuBatchStats {
    /// The prepared numeric group required immutable device upload.
    pub group_uploaded: bool,
    /// Exact occurrence-count mask/result transport required allocation.
    pub transport_allocated: bool,
    /// Old owned host wire plus GPU group/transport before planned eviction.
    pub incoming_resident_bytes: u64,
    /// Requested retained immutable GPU wire payload.
    pub resident_group_bytes: u64,
    /// Requested uniform, masks, output and readback payload.
    pub resident_transport_bytes: u64,
    /// Prospective payload governed by `max_batch_bytes` after planned eviction.
    pub accounted_bytes: u64,
    /// Completed host identity/packing/decoding work allowance.
    pub host_work: u64,
    /// Completed ordered eligibility occurrences.
    pub occurrences: u64,
    /// Complete device work across occurrences.
    pub device_work: u64,
}

fn poll(control: &Control) -> Result<(), GpuError> {
    control.poll().map_err(|stop| GpuError {
        kind: GpuErrorKind::Device,
        detail: "aggregate operation interrupted".to_owned(),
        interruption: Some(stop),
    })
}
fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}

#[cfg(test)]
#[path = "../../tests/aggregate/mod.rs"]
mod tests;
