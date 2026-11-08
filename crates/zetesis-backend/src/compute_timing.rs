//! Optional device compute-pass measurements, independent of host execution time.

use std::time::Duration;

/// Availability of explicitly requested device timestamp measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComputeTimingStatus {
    /// The device context was created without requesting timestamps.
    #[default]
    NotRequested,
    /// Timestamps were requested but the adapter does not support them.
    Unsupported,
    /// The context enabled timestamps; individual samples can still be unavailable.
    Enabled,
}

impl ComputeTimingStatus {
    /// Stable spelling used by human and machine-readable statistics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::Unsupported => "unsupported",
            Self::Enabled => "enabled",
        }
    }
}

/// The device compute-pass time of one completed batch.
///
/// A device oracle reports each batch it completes once, after its interval has
/// been read: as the measured duration of that batch's own compute pass, or as
/// unavailable. Device oracles never report a measured zero, because equal
/// counter readings do not distinguish an unwritten counter from zero cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BatchComputeTime {
    /// The valid duration of that batch's own compute pass.
    Measured(Duration),
    /// The batch completed, but its interval was not established.
    Unavailable,
}

/// Accumulated compute-pass times of completed device batches.
///
/// These durations exclude host preparation, submission, command-buffer copies and
/// waiting. Memory access performed inside the compute pass remains included.
/// They are not the duration of the host oracle or of the complete solve.
/// Each completed batch is counted once: measured, unavailable, or unresolved
/// when its interval had not been read when the record was taken. Unavailable
/// measurements remain separate from measured zero-duration passes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ComputeTiming {
    /// Device feature availability, established when its context was created.
    pub status: ComputeTimingStatus,
    /// Completed batches with a valid device duration.
    pub measured_batches: u64,
    /// Completed batches whose requested device duration was unavailable.
    pub unavailable_batches: u64,
    /// Completed batches whose interval had not been read when this record was
    /// taken: at most one per device oracle, the batch an unfinished execution
    /// completed last, or a settled one could not read because it stopped or met
    /// a busy or unhealthy context. Zero after a settlement that read it.
    pub unresolved_batches: u64,
    /// Resolve-only submissions that read a final batch's interval when its
    /// execution settled. Each downloads 16 bytes; no candidate batch is counted.
    pub flush_submissions: u64,
    /// Sum of valid durations; absent until the first valid measurement.
    pub elapsed: Option<Duration>,
    /// A cumulative count or duration could not be represented. Retained values
    /// remain partial evidence; this does not change the solve's outcome.
    pub overflowed: bool,
}

impl ComputeTiming {
    /// Start an empty measurement record for one context's timing capability.
    #[must_use]
    pub const fn new(status: ComputeTimingStatus) -> Self {
        Self {
            status,
            measured_batches: 0,
            unavailable_batches: 0,
            unresolved_batches: 0,
            flush_submissions: 0,
            elapsed: None,
            overflowed: false,
        }
    }

    /// Whether every recorded batch has a valid duration, with at least one sample
    /// and no batch left unresolved. This does not claim that the solve or every
    /// submitted batch completed.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.status == ComputeTimingStatus::Enabled
            && self.measured_batches != 0
            && self.unavailable_batches == 0
            && self.unresolved_batches == 0
            && !self.overflowed
    }

    /// Record one completed batch's report. Disabled timing performs no update.
    /// A measured duration counts as unavailable unless timing is enabled, so
    /// unsupported timestamps never become measured durations.
    pub fn record(&mut self, report: BatchComputeTime) {
        if self.status == ComputeTimingStatus::NotRequested {
            return;
        }
        match report {
            BatchComputeTime::Measured(sample) if self.status == ComputeTimingStatus::Enabled => {
                if let (Some(count), Some(elapsed)) = (
                    self.measured_batches.checked_add(1),
                    self.elapsed.unwrap_or_default().checked_add(sample),
                ) {
                    self.measured_batches = count;
                    self.elapsed = Some(elapsed);
                } else {
                    self.overflowed = true;
                }
            }
            BatchComputeTime::Measured(_) | BatchComputeTime::Unavailable => {
                increment(&mut self.unavailable_batches, &mut self.overflowed);
            }
        }
    }

    /// Record one resolve-only submission made when an execution settled.
    pub fn record_flush_submission(&mut self) {
        increment(&mut self.flush_submissions, &mut self.overflowed);
    }
}

fn increment(count: &mut u64, overflowed: &mut bool) {
    if let Some(next) = count.checked_add(1) {
        *count = next;
    } else {
        *overflowed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_is_distinct_from_measured_zero() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.record(BatchComputeTime::Unavailable);
        assert_eq!(timing.elapsed, None);
        assert_eq!(timing.unavailable_batches, 1);
        assert!(!timing.is_complete());
        timing.record(BatchComputeTime::Measured(Duration::ZERO));
        assert_eq!(timing.elapsed, Some(Duration::ZERO));
        assert_eq!(timing.measured_batches, 1);
        assert_eq!(timing.unavailable_batches, 1);
    }

    #[test]
    fn complete_timing_requires_a_valid_sample() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        assert!(!timing.is_complete());
        timing.record(BatchComputeTime::Measured(Duration::ZERO));
        assert!(timing.is_complete());
        timing.record(BatchComputeTime::Unavailable);
        assert!(!timing.is_complete());
        assert!(!ComputeTiming::default().is_complete());
        assert!(!ComputeTiming::new(ComputeTimingStatus::Unsupported).is_complete());
    }

    #[test]
    fn an_unresolved_batch_leaves_timing_incomplete() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.record(BatchComputeTime::Measured(Duration::from_nanos(5)));
        timing.unresolved_batches = 1;
        assert!(!timing.is_complete());
        timing.unresolved_batches = 0;
        assert!(timing.is_complete());
    }

    #[test]
    fn flush_submissions_are_counted_apart_from_batches() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.record_flush_submission();
        timing.record_flush_submission();
        assert_eq!(timing.flush_submissions, 2);
        assert_eq!(timing.measured_batches + timing.unavailable_batches, 0);
    }

    #[test]
    fn disabled_timing_retains_no_measurements() {
        let mut timing = ComputeTiming::default();
        timing.record(BatchComputeTime::Measured(Duration::from_nanos(1)));
        timing.record(BatchComputeTime::Unavailable);
        assert_eq!(timing, ComputeTiming::default());
    }

    #[test]
    fn unsupported_timing_never_claims_a_duration() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Unsupported);
        timing.record(BatchComputeTime::Measured(Duration::from_nanos(1)));
        assert_eq!(timing.elapsed, None);
        assert_eq!(timing.measured_batches, 0);
        assert_eq!(timing.unavailable_batches, 1);
    }

    #[test]
    fn overflow_preserves_the_prior_partial_measurement() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.record(BatchComputeTime::Measured(Duration::MAX));
        timing.record(BatchComputeTime::Measured(Duration::from_nanos(1)));
        assert_eq!(timing.elapsed, Some(Duration::MAX));
        assert_eq!(timing.measured_batches, 1);
        assert!(timing.overflowed);
    }

    #[test]
    fn a_counter_at_its_limit_flags_overflow_without_wrapping() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.unavailable_batches = u64::MAX;
        timing.record(BatchComputeTime::Unavailable);
        assert_eq!(timing.unavailable_batches, u64::MAX);
        assert!(timing.overflowed);
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.flush_submissions = u64::MAX;
        timing.record_flush_submission();
        assert_eq!(timing.flush_submissions, u64::MAX);
        assert!(timing.overflowed);
    }
}
