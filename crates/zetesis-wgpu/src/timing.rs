//! Optional compute-pass timestamps, owned by one device oracle for its lifetime.
//!
//! A timer has four query slots used as two pairs, and one resolve buffer per
//! pair. Each timed batch writes the pair that is not pending through its single
//! compute pass. After that pass, the batch's wgpu command buffer records the
//! resolve of the pending pair, written by an earlier queue submission whose
//! completion the host has already observed, into that pair's resolve buffer,
//! and a copy of it into the batch's readback tail. Because the pair was written
//! outside this command buffer, wgpu-core defers the resolve to a hal command
//! buffer of its own at submission and records the copy in another, so the
//! batch's one queue submission carries two more hal command buffers (each an
//! `MTLCommandBuffer` on Metal) than an untimed batch's one.
//!
//! The pair a batch writes is read by the oracle's next submitting call, or by
//! a flush. One batch is in flight per oracle at a time, so two pairs suffice;
//! an asynchronous schedule would need one more pair per batch in flight and
//! resolution in completion order. A resolve that writes nothing leaves its
//! pair's buffer holding that pair's last-read values, or zero when fresh, never
//! the other pair's interval; validation refuses both.

use crate::{GpuError, GpuErrorKind};
use std::time::Duration;
use zetesis_backend::BatchComputeTime;

/// Two resolved timestamps, appended to a transport's mapped result readback or
/// read by a flush.
pub(crate) const READBACK_BYTES: u64 = 16;
/// Logical timing payload of one timed batch: the oracle's four query slots,
/// two resolve buffers and flush readback, plus the transport's readback tail.
/// Query-slot allowance is not a measurement of driver-private allocation.
pub(crate) const OVERHEAD_BYTES: u64 = 6 * READBACK_BYTES;

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

/// Separate a timed readback's logical words from its 16-byte timestamp tail.
/// The tail holds whatever pair the batch resolved; only the timer knows whether
/// it resolved one.
pub(crate) fn split_readback(
    words: &[u32],
    timed: bool,
) -> Result<(&[u32], Option<[u64; 2]>), GpuError> {
    if !timed {
        return Ok((words, None));
    }
    let boundary = words.len().checked_sub(4).ok_or_else(|| {
        GpuError::new(
            GpuErrorKind::Readback,
            "compute timestamp readback is truncated",
        )
    })?;
    let (results, tail) = words.split_at(boundary);
    Ok((results, Some(timestamps(tail))))
}

fn timestamps(words: &[u32]) -> [u64; 2] {
    [
        u64::from(words[0]) | (u64::from(words[1]) << 32),
        u64::from(words[2]) | (u64::from(words[3]) << 32),
    ]
}

/// What one timed batch's readback establishes for its oracle's timer: the
/// resolved tail and the host window of the batch's own submission.
#[derive(Debug, Default)]
pub(crate) struct BatchTiming {
    /// The pair the batch resolved after its pass, when its readback decoded.
    pub(crate) tail: Option<[u64; 2]>,
    /// From the instant before the submission to its observed completion.
    pub(crate) window: Option<Duration>,
}

/// The report of a nonempty call that reached submission: under enabled timing
/// the timer's (the previous batch, or nothing for a fresh timer); under
/// unsupported timing the call's own completed batch, unavailable; nothing when
/// timing was not requested. A call that submitted nothing reports nothing.
///
/// A call that fails after submission also retires the timer. Its own pair was
/// written and never read, so a later batch writing that pair could otherwise
/// resolve the abandoned interval if its pass wrote nothing; a fresh timer's
/// unwritten slots resolve to zero instead, which rule 2 refuses.
pub(crate) fn settle_call(
    status: zetesis_backend::ComputeTimingStatus,
    timer: &mut Option<ComputeTimer>,
    completed: bool,
    timing: &BatchTiming,
    period_ns: f32,
) -> Option<BatchComputeTime> {
    if completed {
        return match timer {
            Some(active) => active.completed(timing, period_ns),
            None => (status == zetesis_backend::ComputeTimingStatus::Unsupported)
                .then_some(BatchComputeTime::Unavailable),
        };
    }
    retire(timer)
}

/// Release a timer whose pending batch can no longer be read, reporting that
/// batch unavailable: after a failure following submission, or when the
/// oracle clears its residency.
pub(crate) fn retire(timer: &mut Option<ComputeTimer>) -> Option<BatchComputeTime> {
    timer
        .take()
        .and_then(|mut retired| retired.ledger.abandon())
}

/// One of a timer's two timestamp pairs: slots {0, 1} or {2, 3}.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pair {
    First,
    Second,
}

impl Pair {
    const fn other(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }

    const fn begin(self) -> u32 {
        match self {
            Self::First => 0,
            Self::Second => 2,
        }
    }

    // Each pair resolves into its own buffer. A resolve destination's offset must
    // be a multiple of `wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT` (256 bytes), so two
    // 16-byte buffers replace the halves of one 32-byte buffer.
    const fn resolve_buffer(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }
}

/// A completed batch whose pair has not been resolved yet.
#[derive(Clone, Copy, Debug)]
struct Pending {
    pair: Pair,
    window: Duration,
}

/// The timing protocol without device state: which pair a batch writes and
/// resolves, and which batch each resolved interval reports.
///
/// Invariant: a pair is resolved only after the host observed completion of
/// the submission that wrote it, and the pair a batch writes is never the
/// pending one. Each completed batch becomes pending and is reported once:
/// by the next completing call or flush from its own interval, or as
/// unavailable when a failure or release abandons it.
#[derive(Debug, Default)]
pub(crate) struct PairLedger {
    pending: Option<Pending>,
    // The value last resolved from each slot, whatever its outcome.
    history: [Option<u64>; 4],
}

impl PairLedger {
    fn written(&self) -> Pair {
        self.pending
            .map_or(Pair::First, |pending| pending.pair.other())
    }

    fn resolving(&self) -> Option<Pair> {
        self.pending.map(|pending| pending.pair)
    }

    pub(crate) const fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// A submitting call completed. Report the pending batch from the tail its
    /// command buffer resolved; this call's batch, whose host window is
    /// `window`, becomes pending.
    fn completed(&mut self, timing: &BatchTiming, period_ns: f32) -> Option<BatchComputeTime> {
        let written = self.written();
        let report = self
            .pending
            .take()
            .map(|pending| self.report(pending, timing.tail, period_ns));
        self.pending = Some(Pending {
            pair: written,
            // A completed call always observed its completion; without a
            // window no later interval of this batch can be accepted.
            window: timing.window.unwrap_or_default(),
        });
        report
    }

    /// A flush resolved the pending pair into `tail`.
    fn flushed(&mut self, tail: [u64; 2], period_ns: f32) -> Option<BatchComputeTime> {
        self.pending
            .take()
            .map(|pending| self.report(pending, Some(tail), period_ns))
    }

    /// The pending batch can no longer be read: a submitting call or a flush
    /// failed after submission, or the timer is released. A failed call's own
    /// batch did not complete; its pair is abandoned with it.
    fn abandon(&mut self) -> Option<BatchComputeTime> {
        self.pending.take().map(|_| BatchComputeTime::Unavailable)
    }

    fn report(
        &mut self,
        pending: Pending,
        tail: Option<[u64; 2]>,
        period_ns: f32,
    ) -> BatchComputeTime {
        let Some([begin, end]) = tail else {
            return BatchComputeTime::Unavailable;
        };
        let slot = pending.pair.begin() as usize;
        let previous = [self.history[slot], self.history[slot + 1]];
        self.history[slot] = Some(begin);
        self.history[slot + 1] = Some(end);
        interval(begin, end, previous, period_ns, pending.window)
            .map_or(BatchComputeTime::Unavailable, BatchComputeTime::Measured)
    }
}

/// The duration a resolved pair establishes, when every rule holds:
///
/// 1. neither value is all ones, Metal's failed-sample value;
/// 2. the begin is nonzero, since an unwritten slot of a fresh query set
///    resolves to zero;
/// 3. neither value repeats its slot's last resolved value, which would mean
///    the slot was not rewritten (this holds whether or not the counter wraps);
/// 4. the end follows the begin, and
/// 5. the period and the converted duration are valid ([`duration`]);
/// 6. the duration is no longer than the host window of the submission that
///    wrote the pair. This rejects stale begins and an overstated period without
///    a shared clock domain; durations trust the adapter's period from below.
fn interval(
    begin: u64,
    end: u64,
    previous: [Option<u64>; 2],
    period_ns: f32,
    window: Duration,
) -> Option<Duration> {
    if begin == u64::MAX || end == u64::MAX || begin == 0 {
        return None;
    }
    if previous[0] == Some(begin) || previous[1] == Some(end) {
        return None;
    }
    duration(begin, end, period_ns).filter(|elapsed| *elapsed <= window)
}

// wgpu timestamps can wrap or fail to resolve a positive interval. Equal
// readings cannot distinguish a sub-resolution interval from an unwritten
// counter, so they do not establish zero compute cost. A reversed interval is
// also unavailable: guessing its device-specific counter width would manufacture
// a duration. Invalid timing never changes an otherwise valid logical result.
fn duration(begin: u64, end: u64, period_ns: f32) -> Option<Duration> {
    if !period_ns.is_finite() || period_ns <= 0.0 {
        return None;
    }
    let ticks = end.checked_sub(begin)?;
    if ticks == 0 {
        return None;
    }
    Duration::try_from_secs_f64(Duration::from_nanos(ticks).as_secs_f64() * f64::from(period_ns))
        .ok()
        .filter(|elapsed| !elapsed.is_zero())
}

/// The timestamp storage and pair ledger one device oracle owns, from its first
/// timed batch until it clears its residency, fails after a submission or drops.
/// Count, theory, cold preparation and workgroup-size changes leave it in place:
/// the pending pair belongs to the oracle's sequence of batches, not to a
/// transport.
pub(crate) struct ComputeTimer {
    queries: wgpu::QuerySet,
    resolved: [wgpu::Buffer; 2],
    flush: wgpu::Buffer,
    ledger: PairLedger,
}

impl ComputeTimer {
    /// Allocate four query slots, a resolve buffer per pair and a flush
    /// readback on a device that enabled timestamp queries.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let resolve = |label| {
            crate::runtime::buffer(
                device,
                label,
                READBACK_BYTES,
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            )
        };
        Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("compute pass timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 4,
            }),
            resolved: [
                resolve("first pair's resolved compute timestamps"),
                resolve("second pair's resolved compute timestamps"),
            ],
            flush: crate::runtime::buffer(
                device,
                "flushed compute timestamps",
                READBACK_BYTES,
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
            ledger: PairLedger::default(),
        }
    }

    /// The next batch's compute-pass writes: the pair that is not pending.
    pub(crate) fn writes(&self) -> wgpu::ComputePassTimestampWrites<'_> {
        let begin = self.ledger.written().begin();
        wgpu::ComputePassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        }
    }

    /// After the batch's pass, record the resolve of the pending pair into that
    /// pair's own resolve buffer and its copy into `readback` at `offset`.
    /// Without a pending pair the tail is left unwritten and ignored.
    pub(crate) fn resolve(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        readback: &wgpu::Buffer,
        offset: u64,
    ) {
        if let Some(pair) = self.ledger.resolving() {
            let begin = pair.begin();
            let resolved = &self.resolved[pair.resolve_buffer()];
            encoder.resolve_query_set(&self.queries, begin..begin + 2, resolved, 0);
            encoder.copy_buffer_to_buffer(resolved, 0, readback, offset, READBACK_BYTES);
        }
    }

    /// Whether a completed batch is still unresolved: its pair is pending.
    pub(crate) const fn unresolved(&self) -> bool {
        self.ledger.pending()
    }

    /// A submitting call completed: see [`PairLedger`].
    pub(crate) fn completed(
        &mut self,
        timing: &BatchTiming,
        period_ns: f32,
    ) -> Option<BatchComputeTime> {
        self.ledger.completed(timing, period_ns)
    }

    /// Submit a resolve-only command for the pending pair and wait at most
    /// `timeout` for it, polling no caller control: a control firing during
    /// this bounded observation must not turn it into an interrupted submission.
    /// The caller holds the context lease and the operation's error scopes.
    pub(crate) fn flush(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        timeout: Duration,
    ) -> crate::runtime::Completion<[u64; 2]> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("compute timestamp flush"),
        });
        self.resolve(&mut encoder, &self.flush, 0);
        let submission = queue.submit([encoder.finish()]);
        crate::runtime::read_polled(
            device,
            &self.flush,
            submission,
            timeout,
            || Ok(()),
            |words| {
                if words.len() == 4 {
                    Ok(timestamps(words))
                } else {
                    Err(GpuError::new(
                        GpuErrorKind::Readback,
                        "compute timestamp flush readback has the wrong size",
                    ))
                }
            },
        )
    }

    /// A flush read the pending pair.
    pub(crate) fn flushed(&mut self, tail: [u64; 2], period_ns: f32) -> Option<BatchComputeTime> {
        self.ledger.flushed(tail, period_ns)
    }
}

#[cfg(test)]
mod tests;
