//! Optional host phase measurements, separate from answer and device qualification.
//!
//! Recorded timing is not a kernel timestamp or a proof of disjoint intervals.
//! Missing, partial or malformed timing never changes an answer-parity decision.

use std::collections::BTreeMap;

use serde::Serialize;

const HEADER: &str =
    "Phase timings: clock=host-monotonic; scope=driver; failed_attempts=included; schema=2";
const LEGACY_HEADER: &str =
    "Phase timings: clock=host-monotonic; scope=driver; failed_attempts=included";
const FOOTER: &str = "  phase scope: source_loading=excluded; statistics_output=excluded; timer_overhead=unattributed; kernel_time=unmeasured";
const LEGACY_LABELS: [&str; 11] = [
    "admission_materialization",
    "execution_setup",
    "candidate_setup",
    "candidate_generation",
    "original_validation",
    "gpu_host_oracle",
    "exact_reduct_membership",
    "closure_membership",
    "objective_scoring_retention",
    "objective_feedback",
    "observation_output",
];

const LABELS: [&str; 13] = [
    "admission_materialization",
    "execution_setup",
    "candidate_setup",
    "certificate_setup",
    "certified_membership",
    "candidate_generation",
    "original_validation",
    "gpu_host_oracle",
    "exact_reduct_membership",
    "closure_membership",
    "objective_scoring_retention",
    "objective_feedback",
    "observation_output",
];

#[derive(Debug, Serialize)]
pub(crate) struct PhaseTimings {
    /// Exact recognized timing schema; legacy records have no certificate phases.
    pub(crate) schema_version: u8,
    /// Driver host interval; source loading and statistics output are excluded.
    pub(crate) driver_elapsed_ns: u64,
    /// False if any attempted measurement reports counter overflow.
    pub(crate) complete: bool,
    /// None means unmeasured, not a zero-duration attempt.
    pub(crate) phases: BTreeMap<&'static str, Option<Measurement>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Measurement {
    pub(crate) calls: u64,
    pub(crate) elapsed_ns: u64,
    pub(crate) complete: bool,
}

/// Read one exact optional section, retaining failure attempts and unmeasured phases.
///
/// The evidence profile uses representable `u64` integer nanoseconds. Out-of-range
/// values are an explicit measurement error, rather than lossy JSON or a panic.
pub(crate) fn parse(stderr: &str) -> Result<Option<PhaseTimings>, String> {
    let lines: Vec<_> = stderr
        .lines()
        .filter(|line| line.starts_with("Phase timings:") || line.starts_with("  phase "))
        .collect();
    if lines.is_empty() {
        return Ok(None);
    }
    let (schema_version, labels): (u8, &[&'static str]) = match lines[0] {
        HEADER => (2, &LABELS),
        LEGACY_HEADER => (1, &LEGACY_LABELS),
        _ => return Err("unsupported phase timing schema".into()),
    };
    if lines.len() != labels.len() + 3 || lines[labels.len() + 2] != FOOTER {
        return Err("missing, duplicate or unsupported phase timing section".into());
    }
    let driver_elapsed_ns = lines[1]
        .strip_prefix("  phase driver: elapsed_ns=")
        .ok_or("missing phase driver interval")
        .and_then(integer)
        .map_err(str::to_owned)?;
    let mut phases = BTreeMap::new();
    let mut complete = true;
    for (&label, line) in labels.iter().zip(&lines[2..labels.len() + 2]) {
        let prefix = format!("  phase {label}: ");
        let value = line
            .strip_prefix(&prefix)
            .ok_or_else(|| format!("missing or reordered phase: {label}"))?;
        let measured = if value == "unmeasured" {
            None
        } else {
            let measured =
                measurement(value).map_err(|reason| format!("phase {label}: {reason}"))?;
            complete &= measured.complete;
            Some(measured)
        };
        phases.insert(label, measured);
    }
    Ok(Some(PhaseTimings {
        schema_version,
        driver_elapsed_ns,
        complete,
        phases,
    }))
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
        return Err("an attempted phase must record a call");
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
#[path = "../tests/support/phase_contracts.rs"]
mod tests;
