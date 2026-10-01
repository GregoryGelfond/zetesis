//! Compact configuration and exclusive host-stage views.

use crate::{
    BackendView, ColorMode, ConfigurationView, GroundingDisplay, GroundingMode, SolveStage,
};
use std::fmt;
use std::io::{self, Write};
use std::time::Duration;

pub(super) fn configuration(
    output: &mut impl Write,
    color: ColorMode,
    view: ConfigurationView<'_>,
) -> io::Result<()> {
    let grounding = match view.grounding {
        GroundingDisplay::Eager => "eager grounding",
        GroundingDisplay::Lazy => "lazy grounding",
        GroundingDisplay::Hybrid => "hybrid grounding",
        GroundingDisplay::TerminalDefinitions => "eager grounding with answer reconstruction",
    };
    let threads = if view.workers.get() == 1 {
        "thread"
    } else {
        "threads"
    };
    match view.backend {
        BackendView::Cpu => color.metadata(
            output,
            "Backend",
            format_args!("CPU · {} {threads} · {grounding}", view.workers),
        ),
        BackendView::Gpu {
            adapter,
            api,
            cpu_completion,
        } => {
            let completion = if cpu_completion {
                " · CPU completion"
            } else {
                ""
            };
            color.metadata(
                output,
                "Backend",
                format_args!(
                    "{api} ({adapter}) · {} CPU {threads} · {grounding}{completion}",
                    view.workers
                ),
            )
        }
    }
}

pub(super) fn timing(
    output: &mut impl Write,
    color: ColorMode,
    stages: &crate::StageTimings,
) -> io::Result<()> {
    let grounding = duration(stages.get(SolveStage::Grounding));
    let solving = duration(stages.get(SolveStage::Solving));
    if matches!(
        stages.grounding_mode,
        GroundingMode::LazyInterleaved | GroundingMode::Mixed
    ) {
        // Lazy work has no independent interval. Mixed execution also includes
        // its explicit eager attempts; the two exclusive stages do not overlap.
        let combined = solving.and_then(|elapsed| match stages.get(SolveStage::Grounding) {
            Some(_) => elapsed.checked_add(grounding?),
            None => Some(elapsed),
        });
        color.metadata(
            output,
            "Time",
            format_args!("grounding + solving {}", Elapsed(combined)),
        )
    } else {
        color.metadata(
            output,
            "Time",
            format_args!(
                "grounding {} · solving {}",
                Elapsed(grounding),
                Elapsed(solving)
            ),
        )
    }
}

fn duration(measurement: Option<crate::StageMeasurement>) -> Option<Duration> {
    measurement
        .filter(|value| !value.overflowed)
        .map(|value| value.elapsed)
}

struct Elapsed(Option<Duration>);

impl fmt::Display for Elapsed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(duration) => write!(formatter, "{:.3} ms", duration.as_secs_f64() * 1000.0),
            None => formatter.write_str("unavailable"),
        }
    }
}
