//! Optional timestamp storage attached to an ordinary resident transport.

use crate::{GpuError, GpuErrorKind};
use std::time::Duration;

/// Two resolved timestamps appended to the existing mapped result buffer.
pub(crate) const READBACK_BYTES: u64 = 16;
/// Logical timing payload: two query slots, their resolve buffer and readback.
/// Query-slot allowance is not a measurement of driver-private allocation.
pub(crate) const OVERHEAD_BYTES: u64 = 3 * READBACK_BYTES;

pub(crate) fn readback_size(results: u64, timed: bool, maximum: u64) -> Result<u64, GpuError> {
    results
        .checked_add(if timed { READBACK_BYTES } else { 0 })
        .filter(|bytes| *bytes <= maximum)
        .ok_or_else(|| {
            GpuError::new(
                GpuErrorKind::Capacity,
                "compute readback exceeds device buffer capacity",
            )
        })
}

pub(crate) struct PassTimer {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
}

impl PassTimer {
    pub(crate) fn new(device: &wgpu::Device) -> Option<Self> {
        device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Self {
                queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("compute pass timestamps"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }),
                resolved: crate::runtime::buffer(
                    device,
                    "resolved compute timestamps",
                    READBACK_BYTES,
                    wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                ),
            })
    }

    pub(crate) fn writes(&self) -> wgpu::ComputePassTimestampWrites<'_> {
        wgpu::ComputePassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }
    }

    pub(crate) fn resolve(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        readback: &wgpu::Buffer,
        offset: u64,
    ) {
        encoder.resolve_query_set(&self.queries, 0..2, &self.resolved, 0);
        encoder.copy_buffer_to_buffer(&self.resolved, 0, readback, offset, READBACK_BYTES);
    }
}

pub(crate) fn split_readback<'a>(
    words: &'a [u32],
    timer: Option<&PassTimer>,
    period_ns: f32,
) -> Result<(&'a [u32], Option<Duration>), GpuError> {
    if timer.is_none() {
        return Ok((words, None));
    }
    split_timestamps(words, period_ns)
}

fn split_timestamps(words: &[u32], period_ns: f32) -> Result<(&[u32], Option<Duration>), GpuError> {
    let boundary = words.len().checked_sub(4).ok_or_else(|| {
        GpuError::new(
            GpuErrorKind::Readback,
            "compute timestamp readback is truncated",
        )
    })?;
    let (results, timestamps) = words.split_at(boundary);
    let begin = u64::from(timestamps[0]) | (u64::from(timestamps[1]) << 32);
    let end = u64::from(timestamps[2]) | (u64::from(timestamps[3]) << 32);
    Ok((results, duration(begin, end, period_ns)))
}

// wgpu timestamps can wrap. A reversed interval is unavailable: guessing its
// device-specific counter width would manufacture a duration. Invalid timing
// never turns an otherwise valid logical result into a rejection or a failure.
fn duration(begin: u64, end: u64, period_ns: f32) -> Option<Duration> {
    if !period_ns.is_finite() || period_ns <= 0.0 {
        return None;
    }
    let ticks = end.checked_sub(begin)?;
    Duration::try_from_secs_f64(Duration::from_nanos(ticks).as_secs_f64() * f64::from(period_ns))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_tail_is_separate_from_logical_results() {
        let words = [7, 9, 10, 0, 14, 0];
        let (results, measured) = split_timestamps(&words, 2.0).unwrap();
        assert_eq!(results, &[7, 9]);
        assert_eq!(measured, Some(Duration::from_nanos(8)));
    }

    #[test]
    fn reversed_timestamps_are_unavailable() {
        let (results, measured) = split_timestamps(&[7, 14, 0, 10, 0], 1.0).unwrap();
        assert_eq!(results, &[7]);
        assert_eq!(measured, None);
    }

    #[test]
    fn invalid_periods_are_unavailable() {
        for period in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(duration(10, 14, period), None);
        }
        assert_eq!(duration(10, 10, 1.0), Some(Duration::ZERO));
    }

    #[test]
    fn missing_timestamp_words_are_refused() {
        assert_eq!(
            split_timestamps(&[0; 3], 1.0).unwrap_err().kind(),
            GpuErrorKind::Readback
        );
    }
}
