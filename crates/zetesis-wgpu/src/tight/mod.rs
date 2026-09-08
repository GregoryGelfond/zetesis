//! Checked original satisfaction and ranked producer support on a real device.

mod admission;
mod device;
mod packing;
mod transport;

use crate::GpuError;
pub use device::GpuTightOracle;
use std::{fmt, time::Duration};
use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::TightVerdict;

const SHADER: &str = include_str!("check.wgsl");

/// Bounds for one complete support-checking batch, independent of search.
#[derive(Clone, Copy, Debug)]
pub struct TightGpuLimits {
    /// Candidate occurrences, also bounded by granted device dispatch limits.
    pub max_candidates: usize,
    /// Conservative authored graph, transport, packing and result payload bytes.
    /// Shared caller-owned theory/certificate/candidates, allocator overhead,
    /// driver-private allocations and deferred retirement are excluded. Not RSS.
    pub max_batch_bytes: u64,
    /// Each candidate reserves `nodes + roots + producers + 2*atoms` operations.
    /// Every scan completes, including initialization and scans after an original
    /// failure; this differs from scalar early-exit work. Insufficient work is
    /// refused before dispatch, never converted into a logical result.
    pub max_work_per_candidate: u64,
    /// Bounded submission wait, with control polling at most 50 ms apart while
    /// waiting. Not GPU preemption or a bound on device creation, allocation,
    /// shader compilation or error-scope drainage.
    pub timeout: Duration,
}

impl Default for TightGpuLimits {
    fn default() -> Self {
        Self {
            max_candidates: 1024,
            max_batch_bytes: 128 * 1024 * 1024,
            max_work_per_candidate: 100_000_000,
            timeout: Duration::from_secs(30),
        }
    }
}

/// A complete device support check for one input occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TightGpuCheck {
    verdict: TightVerdict,
    work: u64,
}

impl TightGpuCheck {
    /// Exact scalar certificate verdict, including its ordered witness.
    /// A residual still requires the caller's exact reduct completion protocol.
    #[must_use]
    pub const fn verdict(&self) -> TightVerdict {
        self.verdict
    }

    /// Completed full-scan operations under [`TightGpuLimits`].
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }
}

/// Resource evidence for the last successful nonempty batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TightGpuBatchStats {
    /// A different immutable theory required graph upload.
    pub theory_uploaded: bool,
    /// Exact candidate-count transport was allocated or replaced.
    pub transport_allocated: bool,
    /// Requested immutable node, root and producer buffer bytes, with padding.
    pub resident_theory_bytes: u64,
    /// Requested uniform, candidate, truth, support, result and readback bytes.
    pub resident_transport_bytes: u64,
    /// Conservative authored payload checked against the batch byte ceiling.
    pub accounted_bytes: u64,
    /// Fully checked input occurrences, preserving repetitions.
    pub candidates: u64,
    /// Sum of completed full-scan operations across candidates.
    pub work: u64,
    /// Actual compute submissions; one for this nonempty successful batch.
    pub dispatches: u64,
    /// Authored initialized/write-buffer payload, not measured bus traffic.
    pub uploaded_bytes: u64,
    /// Successfully decoded readback payload, not measured bus traffic.
    pub downloaded_bytes: u64,
}

/// Evidence retained even when a batch fails. Reset at each new check.
///
/// Submitted work is a scheduled allowance, not a claim that an interrupted GPU
/// completed it. Completion counters require a validated full readback. Upload
/// bytes count authored initialization/write calls, not driver bus measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TightGpuActivity {
    /// Actual compute queue submissions.
    pub submissions: u64,
    /// Input occurrences included in those submissions.
    pub submitted_candidates: u64,
    /// Full-scan work scheduled by submitted commands.
    pub scheduled_work: u64,
    /// Occurrences whose complete records were validated before return.
    pub completed_candidates: u64,
    /// Full-scan work attested by those validated records.
    pub completed_work: u64,
    /// Authored initialization/write-buffer bytes issued in this attempt.
    pub uploaded_bytes: u64,
    /// Readback bytes whose complete records were validated.
    pub downloaded_bytes: u64,
}

/// An incomplete invocation or device failure; never a membership verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TightGpuError {
    /// Shared cancellation or deadline was observed.
    Stopped(Stop),
    /// Adapter, identity, capacity, allocation, execution or readback failure.
    Gpu(GpuError),
}

impl fmt::Display for TightGpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped(stop) => stop.fmt(f),
            Self::Gpu(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for TightGpuError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Stopped(stop) => stop,
            Self::Gpu(error) => error,
        })
    }
}
impl From<GpuError> for TightGpuError {
    fn from(error: GpuError) -> Self {
        match error.interruption {
            Some(stop) => Self::Stopped(stop),
            None => Self::Gpu(error),
        }
    }
}

fn poll(control: &Control) -> Result<(), GpuError> {
    control.poll().map_err(|stop| GpuError {
        kind: crate::GpuErrorKind::Device,
        detail: "tight-support operation interrupted".to_owned(),
        interruption: Some(stop),
    })
}

#[cfg(test)]
#[path = "../../tests/tight/module.rs"]
mod tests;
