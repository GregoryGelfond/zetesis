//! Optional exclusive driver-stage evidence, independent of solver qualification.
//!
//! Completeness describes counter integrity, not answer coverage. Lazy grounding
//! remains inside solving; its separate duration is unavailable rather than zero.

use std::collections::BTreeMap;

use serde::Serialize;

use super::phase::Measurement;

const HEADER: &str = "Stage timings: clock=host-monotonic; scope=driver; schema=1";
const FOOTER: &str = "  stage scope: solving=setup_search_waits_scoring; source_loading=excluded; statistics_output=excluded; timer_overhead=not_separated; lazy_grounding=interleaved; kernel_time=unmeasured";
const LABELS: [&str; 4] = [
    "source_preparation",
    "grounding",
    "solving",
    "observation_output",
];

#[derive(Debug, Serialize)]
pub(crate) struct StageTimings {
    pub(crate) schema_version: u8,
    pub(crate) driver_elapsed_ns: u64,
    /// `lazy_interleaved` has no separate grounding duration; `mixed` retains
    /// only the measured eager spans and has additional grounding inside solving.
    pub(crate) grounding_mode: &'static str,
    /// Counter/time integrity only, including the exclusive partition arithmetic.
    pub(crate) complete: bool,
    pub(crate) unattributed_elapsed_ns: Option<u64>,
    /// None means unmeasured, or unavailable for lazy-interleaved grounding.
    pub(crate) stages: BTreeMap<&'static str, Option<Measurement>>,
}

/// Parse one exact optional stage section without changing answer qualification.
pub(crate) fn parse(stderr: &str) -> Result<Option<StageTimings>, String> {
    let lines: Vec<_> = stderr
        .lines()
        .filter(|line| line.starts_with("Stage timings") || line.starts_with("  stage"))
        .collect();
    if lines.is_empty() {
        return Ok(None);
    }
    if lines[0] != HEADER {
        return Err("unsupported stage timing schema".into());
    }
    if lines.len() != 9 || lines[8] != FOOTER {
        return Err("missing, duplicate or unsupported stage timing section".into());
    }
    let driver_elapsed_ns = field(lines[1], "  stage driver: elapsed_ns=")?;
    let grounding_mode = match lines[2].strip_prefix("  stage grounding_mode: ") {
        Some("unentered") => "unentered",
        Some("eager") => "eager",
        Some("lazy_interleaved") => "lazy_interleaved",
        Some("mixed") => "mixed",
        Some("eager_base_terminal_definitions") => "eager_base_terminal_definitions",
        _ => return Err("missing or unsupported stage grounding mode".into()),
    };
    let mut stages = BTreeMap::new();
    let mut counters_complete = true;
    let mut measured_sum = Some(0_u64);
    for (&label, line) in LABELS.iter().zip(&lines[3..7]) {
        let prefix = format!("  stage {label}: ");
        let value = line
            .strip_prefix(&prefix)
            .ok_or_else(|| format!("missing or reordered stage: {label}"))?;
        let measured = stage_measurement(label, value, grounding_mode)?;
        if let Some(measured) = &measured {
            counters_complete &= measured.complete;
            measured_sum = measured_sum.and_then(|sum| sum.checked_add(measured.elapsed_ns));
        }
        stages.insert(label, measured);
    }
    let unattributed_elapsed_ns = if lines[7] == "  stage unattributed: unavailable" {
        None
    } else {
        let unattributed = field(lines[7], "  stage unattributed: elapsed_ns=")?;
        if !counters_complete
            || measured_sum.and_then(|sum| sum.checked_add(unattributed)) != Some(driver_elapsed_ns)
        {
            return Err("inconsistent exclusive stage timing partition".into());
        }
        Some(unattributed)
    };
    Ok(Some(StageTimings {
        schema_version: 1,
        driver_elapsed_ns,
        grounding_mode,
        complete: counters_complete && unattributed_elapsed_ns.is_some(),
        unattributed_elapsed_ns,
        stages,
    }))
}

fn stage_measurement(label: &str, value: &str, mode: &str) -> Result<Option<Measurement>, String> {
    if label == "grounding" {
        match mode {
            "lazy_interleaved" if value == "unavailable=interleaved" => return Ok(None),
            "lazy_interleaved" => {
                return Err("interleaved grounding cannot be measured separately".into());
            }
            "unentered" if value != "unmeasured" => {
                return Err("unentered grounding must be unmeasured".into());
            }
            "eager" if value == "unmeasured" => {
                return Err("eager grounding requires a measurement".into());
            }
            _ => {}
        }
    }
    if value == "unmeasured" {
        Ok(None)
    } else {
        measurement(value)
            .map(Some)
            .map_err(|reason| format!("stage {label}: {reason}"))
    }
}

fn field(line: &str, prefix: &str) -> Result<u64, String> {
    line.strip_prefix(prefix)
        .ok_or("missing stage timing field")
        .and_then(integer)
        .map_err(str::to_owned)
}

fn measurement(value: &str) -> Result<Measurement, &'static str> {
    let (calls, remainder) = value
        .strip_prefix("calls=")
        .and_then(|value| value.split_once("; elapsed_ns="))
        .ok_or("missing calls or elapsed time")?;
    let (elapsed, complete) = remainder
        .split_once("; complete=")
        .ok_or("missing measurement completeness")?;
    let calls = integer(calls)?;
    if calls == 0 {
        return Err("an attempted stage must record a call");
    }
    Ok(Measurement {
        calls,
        elapsed_ns: integer(elapsed)?,
        complete: match complete {
            "true" => true,
            "false" => false,
            _ => return Err("invalid measurement completeness"),
        },
    })
}

fn integer(value: &str) -> Result<u64, &'static str> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("expected nonnegative integer timing counter");
    }
    value
        .parse()
        .map_err(|_| "timing counter exceeds u64 evidence profile")
}

#[cfg(test)]
mod tests;
