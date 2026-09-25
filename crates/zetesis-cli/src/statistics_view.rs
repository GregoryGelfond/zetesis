//! Human statistics views over solver-owned measurements and outcomes.
//!
//! Rendering does not start timers, rerun work or derive semantic completion.
//! Library consumers can inspect the borrowed values or supply their own view.

use crate::{Completion, PhaseTimings, Publication, SemanticOutcome, SolveConfig, SolveStage};
use std::io::{self, Write};
use zetesis_presentation::{Alignment, Column, Layout, Row, Table};

mod work;

/// Selection of the human table or retained telemetry record protocol.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatisticsView {
    /// Line records consumed by existing measurement tools; no terminal styling.
    #[default]
    Records,
    /// Human tables with explicit units and measurement scope.
    Human,
}

/// Borrowed inputs to a statistics view, independent of command-line parsing.
pub struct Statistics<'a> {
    /// Requested settings; these do not establish the route actually executed.
    pub requested: &'a SolveConfig,
    /// Attempted host intervals, including failed work.
    pub timings: &'a PhaseTimings,
    /// Established semantic evidence, if execution reached a session.
    pub semantic: Option<&'a SemanticOutcome>,
    /// Publication acknowledgements preceding this statistics snapshot.
    pub publication: Option<Publication>,
    /// Whether the controller had encountered a failure before this snapshot.
    pub failed: bool,
}

impl Statistics<'_> {
    /// Write bounded-size human tables. Lazy grounding is reported as interleaved,
    /// never as a zero-duration materialization. Requested policy is named as
    /// such. A compact work view retains each operation's units and scope;
    /// absent receipts are not inferred from requested settings. The complete
    /// counters remain available on `semantic` and `timings`.
    ///
    /// # Errors
    /// Propagates the first writer failure without retrying or flushing.
    pub fn write_human(&self, output: &mut impl Write, layout: Layout) -> io::Result<()> {
        self.timing_table()?.write(output, layout)?;
        writeln!(output)?;
        self.execution_table()?.write(output, layout)?;
        writeln!(output)?;
        work::table(self.semantic, &self.timings.grounding)?.write(output, layout)?;
        writeln!(
            output,
            "Time includes attempted work and output; excludes source loading and this statistics view."
        )?;
        writeln!(
            output,
            "These are host intervals, not GPU kernel times or memory measurements."
        )?;
        writeln!(
            output,
            "Work units differ by operation; subtotals must not be added. Unavailable is not zero."
        )
    }

    fn timing_table(&self) -> io::Result<Table> {
        let mut rows = Vec::new();
        for stage in SolveStage::ALL {
            let value = self.timings.stages.get(stage);
            let note = stage_scope(stage, self.timings.stages.grounding_mode, value);
            rows.push(Row::new([
                stage_name(stage).to_owned(),
                duration(value.map(|value| value.elapsed)),
                note,
            ]));
        }
        rows.push(Row::new([
            "Unattributed".to_owned(),
            duration(self.timings.stages.unattributed),
            "host elapsed".to_owned(),
        ]));
        rows.push(
            Row::new([
                "Total".to_owned(),
                duration(Some(self.timings.driver_elapsed)),
                "driver interval".to_owned(),
            ])
            .conclusion(),
        );
        Table::new(
            "Statistics",
            vec![
                Column::new("Stage", Alignment::Left),
                Column::new("Time (ms)", Alignment::Right),
                Column::new("Scope", Alignment::Left),
            ],
            rows,
        )
        .map_err(io::Error::other)
    }

    fn execution_table(&self) -> io::Result<Table> {
        let completion = self.semantic.and_then(SemanticOutcome::completion).map_or(
            "unavailable",
            |completion| match completion {
                Completion::Exhausted => "exhausted",
                Completion::RequestedModels => "requested answers reached; partial coverage",
                Completion::Interrupted => "interrupted; partial coverage",
            },
        );
        let rows = vec![
            Row::new(["Requested device", self.requested.backend.label()]),
            Row::new(["Requested grounder", self.requested.grounder.label()]),
            Row::new([
                "Requested threads".to_owned(),
                self.requested.workers.to_string(),
            ]),
            Row::new([
                "Requested detailed measurements",
                if self.requested.stats {
                    "enabled"
                } else {
                    "disabled"
                },
            ]),
            Row::new([
                "Recorded execution".to_owned(),
                work::execution(self.semantic),
            ]),
            Row::new([
                "Published answers".to_owned(),
                self.publication.map_or_else(
                    || "unavailable".to_owned(),
                    |value| value.models.to_string(),
                ),
            ]),
            Row::new([
                "Candidate progress".to_owned(),
                self.semantic.map_or_else(
                    || "unavailable".to_owned(),
                    |value| value.candidate_progress().to_string(),
                ),
            ]),
            Row::new(["Search coverage", completion]),
            Row::new([
                "Failure before snapshot",
                if self.failed { "yes" } else { "no" },
            ]),
        ];
        Table::new(
            "Execution",
            vec![
                Column::new("Measure", Alignment::Left),
                Column::new("Value", Alignment::Left),
            ],
            rows,
        )
        .map_err(io::Error::other)
    }
}

fn stage_name(stage: SolveStage) -> &'static str {
    match stage {
        SolveStage::SourcePreparation => "Source preparation",
        SolveStage::Grounding => "Grounding",
        SolveStage::Solving => "Solving",
        SolveStage::ObservationOutput => "Observation and output",
    }
}

fn duration(value: Option<std::time::Duration>) -> String {
    value.map_or_else(
        || "—".to_owned(),
        |value| format!("{:.3}", value.as_secs_f64() * 1000.0),
    )
}

fn stage_scope(
    stage: SolveStage,
    mode: crate::GroundingMode,
    value: Option<crate::StageMeasurement>,
) -> String {
    let scope = match (stage, mode, value) {
        (SolveStage::Grounding, crate::GroundingMode::LazyInterleaved, _) => {
            "interleaved with solving"
        }
        (SolveStage::Grounding, crate::GroundingMode::Mixed, _) => {
            "eager attempts; lazy work interleaved"
        }
        (SolveStage::Grounding, crate::GroundingMode::EagerBaseTerminalDefinitions, _) => {
            "eager base; terminal definitions reconstructed during solving"
        }
        (SolveStage::Solving, crate::GroundingMode::EagerBaseTerminalDefinitions, _) => {
            "base search and full-answer reconstruction"
        }
        (_, _, Some(_)) => "host elapsed",
        (_, _, None) => "not measured",
    };
    if value.is_some_and(|value| value.overflowed) {
        format!("{scope}; incomplete measurement")
    } else {
        scope.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::stage_scope;
    use crate::{GroundingMode, SolveStage, StageMeasurement};

    #[test]
    fn interleaving_does_not_hide_measurement_overflow() {
        for mode in [GroundingMode::Mixed, GroundingMode::LazyInterleaved] {
            let note = stage_scope(
                SolveStage::Grounding,
                mode,
                Some(StageMeasurement {
                    overflowed: true,
                    ..StageMeasurement::default()
                }),
            );
            assert!(note.contains("incomplete measurement"), "{note}");
            assert!(note.contains("interleaved"), "{note}");
        }
    }
}
