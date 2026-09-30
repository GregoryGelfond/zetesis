//! CLI rendering of library-owned host measurements.

use crate::{PhaseTimings, SolvePhase};
use std::io::{self, Write};
use zetesis_sat::PhaseMeasurement;
pub(crate) use zetesis_solve::SolveMeasurements as Recorder;

pub(crate) fn write(sink: &mut impl Write, timings: &PhaseTimings) -> io::Result<()> {
    writeln!(
        sink,
        "Phase timings: clock=host-monotonic; scope=driver; failed_attempts=included; schema=5"
    )?;
    writeln!(
        sink,
        "  phase driver: elapsed_ns={}",
        timings.driver_elapsed.as_nanos()
    )?;
    for phase in SolvePhase::ALL {
        write_phase(sink, phase, timings.get(phase))?;
    }
    writeln!(
        sink,
        "  phase scope: source_loading=excluded; statistics_output=excluded; timer_overhead=unattributed; kernel_time=unmeasured"
    )
}

fn write_phase(
    sink: &mut impl Write,
    phase: SolvePhase,
    value: Option<PhaseMeasurement>,
) -> io::Result<()> {
    if let Some(value) = value {
        writeln!(
            sink,
            "  phase {}: calls={}; elapsed_ns={}; complete={}",
            phase.label(),
            value.calls,
            value.elapsed.as_nanos(),
            !value.overflowed
        )
    } else {
        writeln!(sink, "  phase {}: unmeasured", phase.label())
    }
}

#[cfg(test)]
mod tests;
