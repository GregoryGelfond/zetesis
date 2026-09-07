//! Optional exclusive host stages, independent of solver semantics and rendering.
//!
//! Nested stage guards suspend their parent and restore it on return or unwind.
//! Measurements include failed attempts and never establish semantic completion.
//! Disabled recorders perform no clock reads and allocate no storage dynamically.

use std::cell::Cell;
use std::time::{Duration, Instant};

/// Entered coarse host operation; nested stages have exclusive elapsed time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveStage {
    /// Parsing, admission and source preparation, excluding actual grounding.
    SourcePreparation,
    /// Explicit finite instance materialization, including attempted failures.
    Grounding,
    /// Solver setup, search, coordinator waits and objective scoring.
    Solving,
    /// Observation, rendering and output, including failed writes.
    ObservationOutput,
}
impl SolveStage {
    /// Stable presentation order; these identifiers do not classify semantic work.
    pub const ALL: [Self; 4] = [
        Self::SourcePreparation,
        Self::Grounding,
        Self::Solving,
        Self::ObservationOutput,
    ];
    /// Stable machine-readable identifier.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SourcePreparation => "source_preparation",
            Self::Grounding => "grounding",
            Self::Solving => "solving",
            Self::ObservationOutput => "observation_output",
        }
    }
}

/// Whether explicit grounding intervals account for the selected execution route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroundingMode {
    /// No grounding route or explicit materialization has been entered.
    #[default]
    Unentered,
    /// Only explicit eager materialization has been entered.
    Eager,
    /// Source joins are interleaved with solving; no separate duration is known.
    LazyInterleaved,
    /// Measured eager attempts coexist with unseparated lazy source joins.
    Mixed,
}
impl GroundingMode {
    /// Stable machine-readable identifier.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unentered => "unentered",
            Self::Eager => "eager",
            Self::LazyInterleaved => "lazy_interleaved",
            Self::Mixed => "mixed",
        }
    }
}

/// Exclusive host duration for entered attempts, including an active prefix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageMeasurement {
    /// Entered stage spans; a nested child does not count another parent call.
    pub calls: u64,
    /// Sum of exclusive host intervals; this is neither CPU nor kernel time.
    pub elapsed: Duration,
    /// Counter or time arithmetic overflow made the measurement incomplete.
    pub overflowed: bool,
}
impl StageMeasurement {
    fn enter(&mut self) {
        if let Some(calls) = self.calls.checked_add(1) {
            self.calls = calls;
        } else {
            self.overflowed = true;
        }
    }
    fn add(&mut self, duration: Duration) {
        if let Some(elapsed) = self.elapsed.checked_add(duration) {
            self.elapsed = elapsed;
        } else {
            self.overflowed = true;
        }
    }
}

/// Typed host-stage snapshot, independent of output format and semantic completion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageTimings {
    /// Recorder host interval from construction through this snapshot.
    /// Callers define its scope; no CLI or process-wide boundary is implied.
    pub driver_elapsed: Duration,
    /// Explicit indication of unseparated lazy work, if applicable.
    pub grounding_mode: GroundingMode,
    /// Recorder time outside measured stages. Instrumentation overhead is not
    /// subtracted; bookkeeping can also fall inside the measured stage spans.
    /// `None` indicates invalid nesting or incomplete timing arithmetic.
    pub unattributed: Option<Duration>,
    measurements: [Option<StageMeasurement>; 4],
}
impl StageTimings {
    /// Check that current durations partition the driver interval exactly.
    ///
    /// Requires available unattributed time, no overflowed measurements and an
    /// exact checked sum of every measured duration plus unattributed time.
    /// Public duration fields may be edited; this rechecks their current values.
    /// It establishes arithmetic consistency, not recorder provenance or
    /// successful grounding, solving, or output.
    ///
    /// Takes constant time and space over the four fixed stage slots; reads no clock.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        let Some(mut elapsed) = self.unattributed else {
            return false;
        };
        let mut index = 0;
        // The prefix sum contains unattributed time and exactly the first
        // `index` stage durations. Each step consumes one of four fixed slots.
        while index < self.measurements.len() {
            if let Some(measurement) = self.measurements[index] {
                if measurement.overflowed {
                    return false;
                }
                let Some(total) = elapsed.checked_add(measurement.elapsed) else {
                    return false;
                };
                elapsed = total;
            }
            index += 1;
        }
        elapsed.as_nanos() == self.driver_elapsed.as_nanos()
    }

    /// `None` means unentered, or unavailable for interleaved lazy grounding.
    #[must_use]
    pub const fn get(&self, stage: SolveStage) -> Option<StageMeasurement> {
        self.measurements[stage as usize]
    }
}

#[derive(Clone, Copy, Default)]
// Closed intervals are charged only to their active stage. `since` starts the
// remaining open interval; `depth` counts live guards under stack-order use.
// Taking a snapshot settles a copy, leaving the recorder's open interval intact.
struct State {
    active: Option<SolveStage>,
    since: Option<Instant>,
    depth: usize,
    invalid: bool,
    mode: GroundingMode,
    measurements: [Option<StageMeasurement>; 4],
}
impl State {
    fn finish(&mut self, now: Instant) {
        if let (Some(stage), Some(since)) = (self.active, self.since) {
            if let Some(elapsed) = now.checked_duration_since(since) {
                self.measurements[stage as usize]
                    .get_or_insert_default()
                    .add(elapsed);
            } else {
                self.invalid = true;
            }
        }
        self.since = Some(now);
    }
}

/// A reusable single-thread host recorder with nested, exclusive RAII stages.
///
/// Keep guards in stack order. Invalid nesting makes `unattributed` unavailable
/// rather than affecting application control. This recorder does not spawn work,
/// impose budgets, inspect candidates, or infer completed grounding or solving.
///
/// Construction, stage entry/exit and snapshots take constant time and space:
/// four stage slots are copied or visited, and each guard stores one parent.
/// Nested guards use caller stack space proportional to the live nesting depth.
/// Enabled operations read the host clock; disabled operations do not. No event
/// trace is retained, so individual interval history cannot be reconstructed.
pub struct StageRecorder {
    started: Option<Instant>,
    state: Cell<State>,
}
impl StageRecorder {
    /// Create an enabled recorder, or a clock-free disabled recorder.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            started: enabled.then(Instant::now),
            state: Cell::new(State::default()),
        }
    }
    /// Whether clock reads and measurements are enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.started.is_some()
    }

    /// Enter one exclusive stage; dropping the guard restores its parent.
    pub fn enter(&self, stage: SolveStage) -> StageSpan<'_> {
        self.enter_at(stage, self.started.map(|_| Instant::now()))
    }
    fn enter_at(&self, stage: SolveStage, now: Option<Instant>) -> StageSpan<'_> {
        let mut record = self.state.get();
        let previous = record.active;
        if let Some(now) = now {
            record.finish(now);
            record.active = Some(stage);
            if let Some(depth) = record.depth.checked_add(1) {
                record.depth = depth;
            } else {
                record.invalid = true;
            }
            record.measurements[stage as usize]
                .get_or_insert_default()
                .enter();
            if stage == SolveStage::Grounding {
                record.mode = match record.mode {
                    GroundingMode::Unentered | GroundingMode::Eager => GroundingMode::Eager,
                    GroundingMode::LazyInterleaved | GroundingMode::Mixed => GroundingMode::Mixed,
                };
            }
            self.state.set(record);
        }
        StageSpan {
            recorder: self,
            previous,
            depth: record.depth,
            enabled: now.is_some(),
        }
    }
    /// Record that the execution route uses source joins during solving.
    /// This never invents a zero-duration standalone grounding interval.
    pub fn mark_lazy_grounding(&self) {
        if self.enabled() {
            let mut state = self.state.get();
            state.mode = match state.mode {
                GroundingMode::Unentered | GroundingMode::LazyInterleaved => {
                    GroundingMode::LazyInterleaved
                }
                GroundingMode::Eager | GroundingMode::Mixed => GroundingMode::Mixed,
            };
            self.state.set(state);
        }
    }
    /// Snapshot attempted durations, including any active exclusive prefix.
    #[must_use]
    pub fn snapshot(&self) -> Option<StageTimings> {
        self.started
            .map(|start| self.snapshot_at(start, Instant::now()))
    }
    fn snapshot_at(&self, start: Instant, now: Instant) -> StageTimings {
        let mut state = self.state.get();
        state.finish(now);
        let elapsed = now.checked_duration_since(start);
        let sum = state
            .measurements
            .iter()
            .flatten()
            .try_fold(Duration::ZERO, |sum, value| {
                if value.overflowed {
                    None
                } else {
                    sum.checked_add(value.elapsed)
                }
            });
        StageTimings {
            driver_elapsed: elapsed.unwrap_or_default(),
            grounding_mode: state.mode,
            unattributed: (!state.invalid)
                .then_some(())
                .and(elapsed)
                .zip(sum)
                .and_then(|(elapsed, sum)| elapsed.checked_sub(sum)),
            measurements: state.measurements,
        }
    }
    fn restore(&self, previous: Option<SolveStage>, depth: usize, now: Instant) {
        let mut state = self.state.get();
        state.finish(now);
        state.invalid |= state.depth != depth;
        state.depth = state.depth.saturating_sub(1);
        state.active = previous;
        self.state.set(state);
    }
}

/// Active stage guard; drop in stack order to restore the enclosing stage.
#[must_use = "retain the guard until the stage ends"]
pub struct StageSpan<'a> {
    recorder: &'a StageRecorder,
    previous: Option<SolveStage>,
    depth: usize,
    enabled: bool,
}
impl Drop for StageSpan<'_> {
    fn drop(&mut self) {
        if self.enabled {
            self.recorder
                .restore(self.previous, self.depth, Instant::now());
        }
    }
}

#[cfg(test)]
mod tests;
