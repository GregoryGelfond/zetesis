//! Optional exclusive host stages, independent of solver semantics and rendering.
//!
//! Nested stage guards suspend their parent and restore it on return or unwind.
//! Measurements include failed attempts and never establish semantic completion.
//! Disabled recorders perform no clock reads and allocate no storage dynamically.

use std::sync::{Mutex, MutexGuard};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

/// Entered coarse host operation; nested stages have exclusive elapsed time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveStage {
    /// Parsing, admission and source preparation, excluding actual grounding.
    SourcePreparation,
    /// Explicit finite instance materialization, including attempted failures.
    Grounding,
    /// Solver setup, search, coordinator waits, objective scoring and projected
    /// answer identity. Display and rendering remain observation/output work.
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
    /// Eager base materialization followed by terminal-definition reconstruction
    /// during solving. Grounding intervals do not cover the full original theory.
    /// A shared recorder can also contain other eager attempts.
    EagerBaseTerminalDefinitions,
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
            Self::EagerBaseTerminalDefinitions => "eager_base_terminal_definitions",
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
    /// Materialization and source work retained in the measured scope.
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
    owner: Option<ThreadId>,
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

/// A thread-safe host recorder with nested, exclusive RAII stages.
///
/// Enter and drop guards in stack order on the same thread. Concurrent stage
/// scopes from different threads, transferred live guards, invalid nesting and
/// poisoned bookkeeping make `unattributed` unavailable rather than affecting
/// application control.
/// Sequential use can move between threads. Internal locks protect only fixed
/// bookkeeping; a guard holds no lock during application work. This recorder
/// does not spawn work, impose budgets, inspect candidates, or infer completed
/// grounding or solving.
///
/// Construction, stage entry/exit and snapshots use constant space and fixed
/// work over four stage slots; concurrent bookkeeping can briefly wait for a
/// lock. Each guard stores one parent.
/// Nested guards use caller stack space proportional to the live nesting depth.
/// Enabled operations read the host clock; disabled operations do not. No event
/// trace is retained, so individual interval history cannot be reconstructed.
pub struct StageRecorder {
    started: Option<Instant>,
    state: Mutex<State>,
}
impl StageRecorder {
    /// Create an enabled recorder, or a clock-free disabled recorder.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            started: enabled.then(Instant::now),
            state: Mutex::new(State::default()),
        }
    }
    /// Whether clock reads and measurements are enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.started.is_some()
    }

    /// Enter one exclusive stage; dropping the guard restores its parent.
    /// Overlap with an active stage on another thread invalidates attribution.
    pub fn enter(&self, stage: SolveStage) -> StageSpan<'_> {
        if !self.enabled() {
            return StageSpan {
                recorder: self,
                previous: None,
                depth: 0,
                enabled: false,
            };
        }
        let mut record = self.lock_state();
        self.enter_record(&mut record, stage, Some(Instant::now()))
    }

    #[cfg(test)]
    fn enter_at(&self, stage: SolveStage, now: Option<Instant>) -> StageSpan<'_> {
        self.enter_record(&mut self.lock_state(), stage, now)
    }

    fn enter_record(
        &self,
        record: &mut State,
        stage: SolveStage,
        now: Option<Instant>,
    ) -> StageSpan<'_> {
        let previous = record.active;
        if let Some(now) = now {
            let current = thread::current().id();
            if record.depth == 0 {
                record.owner = Some(current);
            } else if record.owner != Some(current) {
                record.invalid = true;
            }
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
                    GroundingMode::EagerBaseTerminalDefinitions => {
                        GroundingMode::EagerBaseTerminalDefinitions
                    }
                };
            }
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
            let mut state = self.lock_state();
            state.mode = match state.mode {
                GroundingMode::Unentered | GroundingMode::LazyInterleaved => {
                    GroundingMode::LazyInterleaved
                }
                GroundingMode::Eager
                | GroundingMode::Mixed
                | GroundingMode::EagerBaseTerminalDefinitions => GroundingMode::Mixed,
            };
        }
    }

    /// Record an eager base whose terminal definitions are reconstructed during
    /// solving. No eager interval for the full original theory is implied, and
    /// no duration is created. Lazy routes sharing this recorder remain mixed.
    pub fn mark_terminal_definitions(&self) {
        if self.enabled() {
            let mut state = self.lock_state();
            state.mode = match state.mode {
                GroundingMode::Unentered
                | GroundingMode::Eager
                | GroundingMode::EagerBaseTerminalDefinitions => {
                    GroundingMode::EagerBaseTerminalDefinitions
                }
                GroundingMode::LazyInterleaved | GroundingMode::Mixed => GroundingMode::Mixed,
            };
        }
    }
    /// Snapshot attempted durations, including any active exclusive prefix.
    /// This closes no live guard and retains no lock in the returned value.
    #[must_use]
    pub fn snapshot(&self) -> Option<StageTimings> {
        self.started.map(|start| {
            let state = self.lock_state();
            Self::snapshot_record(*state, start, Instant::now())
        })
    }

    #[cfg(test)]
    fn snapshot_at(&self, start: Instant, now: Instant) -> StageTimings {
        Self::snapshot_record(*self.lock_state(), start, now)
    }

    fn snapshot_record(mut state: State, start: Instant, now: Instant) -> StageTimings {
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

    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| {
            let mut state = poisoned.into_inner();
            state.invalid = true;
            state
        })
    }

    fn restore(&self, previous: Option<SolveStage>, depth: usize, now: Instant) {
        let mut state = self.lock_state();
        state.finish(now);
        state.invalid |= state.depth != depth || state.owner != Some(thread::current().id());
        state.depth = state.depth.saturating_sub(1);
        state.active = previous;
        if state.depth == 0 {
            state.owner = None;
        }
    }
}

/// Active stage guard; drop in stack order on its creating thread to restore
/// the enclosing stage with complete exclusive attribution.
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
