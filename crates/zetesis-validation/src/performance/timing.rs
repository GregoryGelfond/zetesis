//! Owned observations from the existing strict native phase/stage parsers.
use super::{phase, stage};
use serde::Serialize;
use std::collections::BTreeMap;

/// One complete host-wall observation; zero elapsed time is a valid clock value.
#[derive(Debug, Serialize)]
pub struct Measurement {
    /// Number of entered calls covered by the aggregate observation.
    pub calls: u64,
    /// Accumulated monotonic host-wall nanoseconds, not CPU time.
    pub elapsed_ns: u64,
}
/// Separate instrumented native observation; raw source attribution remains in stderr.
#[derive(Debug, Serialize)]
pub struct Diagnostics {
    /// Driver interval excluding source loading and statistics output.
    pub driver_elapsed_ns: u64,
    /// Actual native grounding mode; lazy grounding remains interleaved.
    pub grounding_mode: String,
    /// Exclusive source-preparation, grounding, solving and observation stages.
    pub stages: BTreeMap<String, Option<Measurement>>,
    /// Unattributed portion completing the exclusive stage partition.
    pub unattributed_elapsed_ns: u64,
    /// Additional coarse phases, distinct from the exclusive stage partition.
    pub phases: BTreeMap<String, Option<Measurement>>,
    /// Recognized phase schema; older records omit phases introduced later.
    pub phase_schema: u8,
}
fn measurements(
    values: BTreeMap<&str, Option<phase::Measurement>>,
) -> BTreeMap<String, Option<Measurement>> {
    values
        .into_iter()
        .map(|(name, value)| {
            (
                name.into(),
                value.map(|v| Measurement {
                    calls: v.calls,
                    elapsed_ns: v.elapsed_ns,
                }),
            )
        })
        .collect()
}
pub(super) fn parse(bytes: &[u8]) -> Result<Diagnostics, String> {
    let diagnostics = parse_any(bytes)?;
    if diagnostics.grounding_mode != "eager" {
        return Err("native phase/stage evidence is incomplete or not eager".into());
    }
    Ok(diagnostics)
}

pub(super) fn parse_any(bytes: &[u8]) -> Result<Diagnostics, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let phases = phase::parse(text)?.ok_or("missing native phase timings")?;
    let stages = stage::parse(text)?.ok_or("missing native stage timings")?;
    if !phases.complete || !stages.complete || phases.driver_elapsed_ns != stages.driver_elapsed_ns
    {
        return Err("native phase/stage evidence is incomplete".into());
    }
    Ok(Diagnostics {
        driver_elapsed_ns: stages.driver_elapsed_ns,
        grounding_mode: stages.grounding_mode.into(),
        stages: measurements(stages.stages),
        unattributed_elapsed_ns: stages
            .unattributed_elapsed_ns
            .ok_or("missing stage remainder")?,
        phases: measurements(phases.phases),
        phase_schema: phases.schema_version,
    })
}
