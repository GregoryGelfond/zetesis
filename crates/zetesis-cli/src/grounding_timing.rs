//! Fixed-space aggregation of optional formula-grounding observations.

use std::cell::Cell;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use zetesis_themelios::{GroundingOutcome, GroundingPhase, GroundingWork};

/// Host intervals and work for all attempted occurrences of one grounding phase.
/// Counts include failed and unwinding attempts; they are not semantic verdicts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundingMeasurement {
    /// Sum of observed host intervals, or `None` on duration overflow.
    pub elapsed: Option<Duration>,
    /// Checked sum of the frontend's selected work populations.
    pub work: GroundingWork,
    outcomes: [Option<u64>; GroundingOutcome::ALL.len()],
}

impl GroundingMeasurement {
    /// Number of phase attempts ending with this outcome, or `None` on overflow.
    #[must_use]
    pub fn count(&self, outcome: GroundingOutcome) -> Option<u64> {
        self.outcomes[outcome_index(outcome)]
    }

    fn record(&mut self, elapsed: Duration, outcome: GroundingOutcome, work: GroundingWork) {
        self.elapsed = self.elapsed.and_then(|value| value.checked_add(elapsed));
        let count = &mut self.outcomes[outcome_index(outcome)];
        *count = count.and_then(|value| value.checked_add(1));
        self.work = self.work.checked_sum(work);
    }
}

impl Default for GroundingMeasurement {
    fn default() -> Self {
        Self {
            elapsed: Some(Duration::ZERO),
            work: GroundingWork::default(),
            outcomes: [Some(0); GroundingOutcome::ALL.len()],
        }
    }
}

/// Optional eager-formula attribution aggregated by phase in constant space.
/// Rule locations are available to custom frontend observers but are not retained
/// here. Durations include counter/timer overhead and are not GPU kernel times.
/// This view does not attribute relational eager or interleaved lazy grounding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroundingTimings {
    measurements: [Option<GroundingMeasurement>; GroundingPhase::ALL.len()],
}

impl GroundingTimings {
    /// Recorded attempts of a phase; `None` means no completed callback pair.
    /// It must not be interpreted as zero work in an unmeasured grounder.
    #[must_use]
    pub fn get(&self, phase: GroundingPhase) -> Option<&GroundingMeasurement> {
        self.measurements[phase_index(phase)].as_ref()
    }
}

fn phase_index(phase: GroundingPhase) -> usize {
    GroundingPhase::ALL
        .iter()
        .position(|&entry| entry == phase)
        .expect("frontend phase catalog covers its variants")
}

fn outcome_index(outcome: GroundingOutcome) -> usize {
    GroundingOutcome::ALL
        .iter()
        .position(|&entry| entry == outcome)
        .expect("frontend outcome catalog covers its variants")
}

#[derive(Default)]
pub(crate) struct Recorder {
    active: Cell<Option<(GroundingPhase, Instant)>>,
    timings: Cell<GroundingTimings>,
}

impl Recorder {
    pub(crate) fn enter(&self, phase: GroundingPhase) {
        assert!(
            self.active.replace(Some((phase, Instant::now()))).is_none(),
            "frontend grounding phases must not nest"
        );
    }

    pub(crate) fn exit(
        &self,
        phase: GroundingPhase,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        let (entered, start) = self.active.take().expect("phase exit follows its entry");
        assert_eq!(phase, entered, "phase exit identifies its entry");
        let elapsed = start.elapsed();
        let mut timings = self.timings.get();
        timings.measurements[phase_index(phase)]
            .get_or_insert_default()
            .record(elapsed, outcome, work);
        self.timings.set(timings);
    }

    pub(crate) fn snapshot(&self) -> GroundingTimings {
        self.timings.get()
    }
}

/// Shared field vocabulary for the human and JSON views of the same data.
pub(crate) fn work_fields(work: &GroundingWork) -> [(&'static str, Option<u64>); 14] {
    [
        ("support_rounds", work.support_rounds),
        ("support_atoms", work.support_atoms),
        ("support_index_entries", work.support_index_entries),
        ("join_probes", work.join_probes),
        ("join_rows", work.join_rows),
        ("binding_snapshots", work.binding_snapshots),
        ("readiness_nodes", work.readiness_nodes),
        ("expression_evaluations", work.expression_evaluations),
        ("expression_nodes", work.expression_nodes),
        ("atom_lookups", work.atom_lookups),
        ("atoms_inserted", work.atoms_inserted),
        ("node_lookups", work.node_lookups),
        ("nodes_inserted", work.nodes_inserted),
        ("roots", work.roots),
    ]
}

pub(crate) fn write(sink: &mut impl Write, timings: &GroundingTimings) -> io::Result<()> {
    writeln!(
        sink,
        "Grounding attribution: scope=eager_formula; clock=host-monotonic; schema=1"
    )?;
    for phase in GroundingPhase::ALL {
        write!(sink, "  grounding {}:", phase.label())?;
        if let Some(value) = timings.get(phase) {
            field(
                sink,
                "elapsed_ns",
                value.elapsed.map(|value| value.as_nanos()),
            )?;
            for outcome in GroundingOutcome::ALL {
                field(sink, outcome.label(), value.count(outcome).map(u128::from))?;
            }
            for (name, count) in work_fields(&value.work) {
                field(sink, name, count.map(u128::from))?;
            }
            writeln!(sink)?;
        } else {
            writeln!(sink, " unmeasured")?;
        }
    }
    writeln!(
        sink,
        "  grounding scope: counters=selected_work_populations; rule_work=joins_filters_emission_interleaved; overhead=included; relational_grounding=unmeasured; kernel_time=unmeasured"
    )
}

fn field(sink: &mut impl Write, name: &str, value: Option<u128>) -> io::Result<()> {
    match value {
        Some(value) => write!(sink, " {name}={value};"),
        None => write!(sink, " {name}=unavailable;"),
    }
}

#[cfg(test)]
#[path = "../tests/support/grounding_timing_contracts.rs"]
mod tests;
