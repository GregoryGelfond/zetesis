//! Optional device compute-pass measurements, independent of host execution time.

use std::time::Duration;

/// Availability of explicitly requested device timestamp measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

/// Accumulated valid compute-pass timestamps from completely decoded batches.
///
/// These durations exclude host preparation, submission, command-buffer copies and
/// waiting. Memory access performed inside the compute pass remains included.
/// They are not the duration of the host oracle or of the complete solve.
/// Unavailable measurements remain separate from measured zero-duration passes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComputeTiming {
    /// Device feature availability, established when its context was created.
    pub status: ComputeTimingStatus,
    /// Complete batches with a valid device duration.
    pub measured_batches: u64,
    /// Complete batches whose requested device duration was unavailable.
    pub unavailable_batches: u64,
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
            elapsed: None,
            overflowed: false,
        }
    }

    /// Whether every recorded batch has a valid duration, with at least one sample.
    /// This does not claim that the solve or every submitted batch completed.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.status == ComputeTimingStatus::Enabled
            && self.measured_batches != 0
            && self.unavailable_batches == 0
            && !self.overflowed
    }

    /// Record one completely decoded batch. Disabled timing performs no update.
    /// Unsupported or invalid timestamps do not become measured zero durations.
    pub fn record(&mut self, sample: Option<Duration>) {
        if self.status == ComputeTimingStatus::NotRequested {
            return;
        }
        let sample = sample.filter(|_| self.status == ComputeTimingStatus::Enabled);
        if let Some(sample) = sample {
            if let (Some(count), Some(elapsed)) = (
                self.measured_batches.checked_add(1),
                self.elapsed.unwrap_or_default().checked_add(sample),
            ) {
                self.measured_batches = count;
                self.elapsed = Some(elapsed);
            } else {
                self.overflowed = true;
            }
        } else if let Some(count) = self.unavailable_batches.checked_add(1) {
            self.unavailable_batches = count;
        } else {
            self.overflowed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_is_distinct_from_measured_zero() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.record(None);
        assert_eq!(timing.elapsed, None);
        assert_eq!(timing.unavailable_batches, 1);
        assert!(!timing.is_complete());
        timing.record(Some(Duration::ZERO));
        assert_eq!(timing.elapsed, Some(Duration::ZERO));
        assert_eq!(timing.measured_batches, 1);
        assert_eq!(timing.unavailable_batches, 1);
    }

    #[test]
    fn complete_timing_requires_a_valid_sample() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        assert!(!timing.is_complete());
        timing.record(Some(Duration::ZERO));
        assert!(timing.is_complete());
        timing.record(None);
        assert!(!timing.is_complete());
        assert!(!ComputeTiming::default().is_complete());
        assert!(!ComputeTiming::new(ComputeTimingStatus::Unsupported).is_complete());
    }

    #[test]
    fn disabled_timing_retains_no_measurements() {
        let mut timing = ComputeTiming::default();
        timing.record(Some(Duration::from_nanos(1)));
        timing.record(None);
        assert_eq!(timing, ComputeTiming::default());
    }

    #[test]
    fn unsupported_timing_never_claims_a_duration() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Unsupported);
        timing.record(Some(Duration::from_nanos(1)));
        assert_eq!(timing.elapsed, None);
        assert_eq!(timing.measured_batches, 0);
        assert_eq!(timing.unavailable_batches, 1);
    }

    #[test]
    fn overflow_preserves_the_prior_partial_measurement() {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.record(Some(Duration::MAX));
        timing.record(Some(Duration::from_nanos(1)));
        assert_eq!(timing.elapsed, Some(Duration::MAX));
        assert_eq!(timing.measured_batches, 1);
        assert!(timing.overflowed);
    }
}
