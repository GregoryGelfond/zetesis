//! CLI rendering of library-owned host measurements.

use crate::{GroundingMode, SolveStage, StageTimings};
use std::io::{self, Write};

pub(crate) fn write(sink: &mut impl Write, timings: &StageTimings) -> io::Result<()> {
    write_friendly(sink, timings)?;
    writeln!(
        sink,
        "Stage timings: clock=host-monotonic; scope=driver; schema=1"
    )?;
    writeln!(
        sink,
        "  stage driver: elapsed_ns={}",
        timings.driver_elapsed.as_nanos()
    )?;
    writeln!(
        sink,
        "  stage grounding_mode: {}",
        timings.grounding_mode.label()
    )?;
    for stage in SolveStage::ALL {
        if stage == SolveStage::Grounding
            && timings.grounding_mode == GroundingMode::LazyInterleaved
        {
            writeln!(sink, "  stage grounding: unavailable=interleaved")?;
        } else if let Some(value) = timings.get(stage) {
            writeln!(
                sink,
                "  stage {}: calls={}; elapsed_ns={}; complete={}",
                stage.label(),
                value.calls,
                value.elapsed.as_nanos(),
                !value.overflowed
            )?;
        } else {
            writeln!(sink, "  stage {}: unmeasured", stage.label())?;
        }
    }
    if let Some(duration) = timings.unattributed {
        writeln!(
            sink,
            "  stage unattributed: elapsed_ns={}",
            duration.as_nanos()
        )?;
    } else {
        writeln!(sink, "  stage unattributed: unavailable")?;
    }
    writeln!(
        sink,
        "  stage scope: solving=setup_search_waits_scoring; source_loading=excluded; statistics_output=excluded; timer_overhead=not_separated; lazy_grounding=interleaved; kernel_time=unmeasured"
    )
}

fn write_friendly(sink: &mut impl Write, timings: &StageTimings) -> io::Result<()> {
    write!(sink, "Timing summary (host ms): ")?;
    for (index, (stage, label)) in [
        (SolveStage::SourcePreparation, "source preparation"),
        (SolveStage::Grounding, "grounding"),
        (SolveStage::Solving, "solving incl. setup/waits"),
        (SolveStage::ObservationOutput, "output"),
    ]
    .into_iter()
    .enumerate()
    {
        if index != 0 {
            write!(sink, "; ")?;
        }
        write!(sink, "{label}=")?;
        if stage == SolveStage::Grounding
            && timings.grounding_mode == GroundingMode::LazyInterleaved
        {
            write!(sink, "unavailable (lazy work interleaved with solving)")?;
        } else if let Some(value) = timings.get(stage) {
            write!(
                sink,
                "{:.3}{}",
                value.elapsed.as_secs_f64() * 1000.0,
                if value.overflowed {
                    " (incomplete timing)"
                } else {
                    ""
                }
            )?;
            if stage == SolveStage::Grounding && timings.grounding_mode == GroundingMode::Mixed {
                write!(sink, " (eager only; lazy work interleaved with solving)")?;
            }
        } else {
            write!(sink, "unentered")?;
        }
    }
    match timings.unattributed {
        Some(value) => writeln!(sink, "; unattributed={:.3}", value.as_secs_f64() * 1000.0),
        None => writeln!(sink, "; unattributed=unavailable"),
    }
}

#[cfg(test)]
#[path = "../tests/support/stage_timing_contracts.rs"]
mod tests;
