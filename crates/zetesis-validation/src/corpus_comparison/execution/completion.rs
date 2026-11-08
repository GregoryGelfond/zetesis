//! Strict bounded completion statistics, separate from historical scalar records.

use std::num::NonZeroUsize;

use serde::Serialize;

use super::{Generation, field_values, fields, line, number};

#[derive(Clone, Copy)]
pub(crate) struct CompletionRequest {
    pub(crate) workers: NonZeroUsize,
    pub(crate) max_scratch_bytes: u64,
}

/// Qualified bounded completion. `bounded_completion_v1` records come from the
/// 2026-09-12 checkpoints and carry one admitted scratch peak;
/// `bounded_completion_v2` records come from released executables and carry
/// the requested envelope and the retained peak separately.
#[derive(Debug, Serialize)]
pub(crate) struct CompletionExecution {
    pub(crate) profile: &'static str,
    pub(crate) requested_workers: u64,
    /// The maximum query concurrency selected at preflight; released
    /// executables label it `peak preflight workers`.
    pub(crate) peak_effective_workers: u64,
    pub(crate) entered: u64,
    pub(crate) residuals: u64,
    pub(crate) completed: u64,
    pub(crate) residual_completed: u64,
    pub(crate) failed: u64,
    pub(crate) residual_failed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) peak_admitted_logical_scratch_bytes: Option<u64>,
    /// The largest result envelope requested before allocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) peak_requested_scratch_bytes: Option<u64>,
    /// The largest retained worker and result storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) peak_retained_scratch_bytes: Option<u64>,
    pub(crate) max_logical_scratch_bytes: u64,
}

/// The counters every bounded generation reports, before their relations to
/// the device counters are checked.
struct Counters {
    requested: u64,
    effective: u64,
    entered: u64,
    residuals: u64,
    completed: u64,
    failed: u64,
    residual_completed: u64,
    residual_failed: u64,
    limit: u64,
}

/// How a generation reports completion scratch.
enum Scratch {
    /// One admitted peak (the 2026-09-12 checkpoints).
    Admitted(u64),
    /// The requested envelope and the retained peak (released executables).
    Separate { requested: u64, retained: u64 },
}

pub(super) fn record(
    text: &str,
    generation: Generation,
    request: Option<CompletionRequest>,
    candidates: u64,
    residuals: u64,
    slate_limit: u64,
) -> Result<Option<CompletionExecution>, String> {
    let (profile, counters, scratch) = match generation {
        Generation::Released {
            requested,
            effective,
        } => {
            let [
                entered,
                exact,
                completed,
                failed,
                requested_bytes,
                retained_bytes,
                limit,
            ] = field_values(
                completion_line(text)?,
                [
                    "entered",
                    "residuals entered",
                    "completed before commit",
                    "failed",
                    "peak requested scratch bytes",
                    "peak shared owner, query and transient/result scratch bytes",
                    "scratch limit",
                ],
            )?;
            // Completion requests the result envelope first and retains it with
            // every worker's storage; either exceeding the limit refuses the
            // batch before candidate work starts, so a completed run cannot.
            if requested_bytes > retained_bytes
                || retained_bytes > limit
                || (requested_bytes == 0) != (candidates == 0)
            {
                return Err(
                    "bounded completion scratch statistics contradict the completed batches".into(),
                );
            }
            let [residual_completed, residual_failed] = residual_completion(text)?;
            (
                "bounded_completion_v2",
                Counters {
                    requested,
                    effective,
                    entered,
                    residuals: exact,
                    completed,
                    failed,
                    residual_completed,
                    residual_failed,
                    limit,
                },
                Scratch::Separate {
                    requested: requested_bytes,
                    retained: retained_bytes,
                },
            )
        }
        Generation::BoundedCheckpoint {
            requested,
            effective,
        } => {
            let [entered, exact, completed, failed, peak, limit] = field_values(
                completion_line(text)?,
                [
                    "entered",
                    "residuals entered",
                    "completed before commit",
                    "failed",
                    "peak admitted logical scratch bytes",
                    "scratch limit",
                ],
            )?;
            if peak > limit || (peak == 0) != (candidates == 0) {
                return Err(
                    "bounded completion scratch statistics contradict the completed batches".into(),
                );
            }
            let [residual_completed, residual_failed] = residual_completion(text)?;
            (
                "bounded_completion_v1",
                Counters {
                    requested,
                    effective,
                    entered,
                    residuals: exact,
                    completed,
                    failed,
                    residual_completed,
                    residual_failed,
                    limit,
                },
                Scratch::Admitted(peak),
            )
        }
        Generation::ScalarCheckpoint => return scalar(text, request),
    };
    check(&counters, request, candidates, residuals, slate_limit)?;
    Ok(Some(execution(profile, &counters, &scratch)))
}

/// A scalar record has no completion statistics, and cannot establish a
/// bounded request other than the scalar route's own.
fn scalar(
    text: &str,
    request: Option<CompletionRequest>,
) -> Result<Option<CompletionExecution>, String> {
    if request.is_some_and(|r| r.workers.get() != 1 || r.max_scratch_bytes != 268_435_456) {
        return Err(
            "legacy scalar telemetry cannot establish the requested bounded completion settings"
                .into(),
        );
    }
    if text.lines().any(|line| {
        line.starts_with("  formula completion: ")
            || line.starts_with("  formula residual completion: ")
            || line.starts_with("  formula GPU limits: ")
    }) {
        return Err("legacy route mixed with current completion statistics".into());
    }
    Ok(None)
}

fn completion_line(text: &str) -> Result<&str, String> {
    line(text, "  formula completion: ")?
        .strip_suffix("; counters overflowed=false")
        .ok_or_else(|| "missing, malformed or overflowed completion counters".into())
}

fn residual_completion(text: &str) -> Result<[u64; 2], String> {
    fields(
        text,
        "  formula residual completion: ",
        ["completed locally", "failed"],
    )
}

/// The relations every bounded completion record must satisfy against the
/// device counters and the request: every candidate entered and completed,
/// every residual completed locally, nothing failed, and the preflight
/// concurrency never above the requested workers, the residuals or the
/// largest slate.
fn check(
    counters: &Counters,
    request: Option<CompletionRequest>,
    candidates: u64,
    residuals: u64,
    slate_limit: u64,
) -> Result<(), String> {
    let Counters {
        requested,
        effective,
        entered,
        residuals: exact,
        completed,
        failed,
        residual_completed,
        residual_failed,
        limit,
    } = *counters;
    if requested == 0
        || effective > requested
        || effective > residuals
        || effective > slate_limit
        || (effective == 0) != (residuals == 0)
        || entered != candidates
        || exact != residuals
        || completed != candidates
        || failed != 0
        || residual_completed != residuals
        || residual_failed != 0
        || request.is_some_and(|r| {
            u128::from(requested) != r.workers.get() as u128 || limit != r.max_scratch_bytes
        })
    {
        return Err(
            "bounded completion statistics contradict the complete native request/accounting"
                .into(),
        );
    }
    Ok(())
}

fn execution(profile: &'static str, counters: &Counters, scratch: &Scratch) -> CompletionExecution {
    let (admitted, requested, retained) = match *scratch {
        Scratch::Admitted(peak) => (Some(peak), None, None),
        Scratch::Separate {
            requested,
            retained,
        } => (None, Some(requested), Some(retained)),
    };
    CompletionExecution {
        profile,
        requested_workers: counters.requested,
        peak_effective_workers: counters.effective,
        entered: counters.entered,
        residuals: counters.residuals,
        completed: counters.completed,
        failed: counters.failed,
        residual_completed: counters.residual_completed,
        residual_failed: counters.residual_failed,
        peak_admitted_logical_scratch_bytes: admitted,
        peak_requested_scratch_bytes: requested,
        peak_retained_scratch_bytes: retained,
        max_logical_scratch_bytes: counters.limit,
    }
}

/// The 2026-09-12 checkpoints' device limits line, which also carried the peak
/// authored bytes.
pub(super) fn gpu_limits(text: &str) -> Result<(u64, u64, u64), String> {
    let mut parts = line(text, "  formula GPU limits: ")?.split("; ");
    let sweeps = number(
        parts.next().ok_or("missing GPU sweep limit")?,
        "propagation sweeps",
    )?;
    let work = parts
        .next()
        .and_then(|p| p.strip_suffix(" (u32 ceiling)"))
        .ok_or("missing GPU work limit")?;
    let work = number(work, "propagation work/candidate")?;
    let peak = number(
        parts.next().ok_or("missing GPU allocation accounting")?,
        "peak authored GPU bytes",
    )?;
    if parts.next() != Some("GPU kernel timing=unavailable")
        || parts.next().is_some()
        || work > u64::from(u32::MAX)
    {
        return Err("unsupported formula GPU-limit statistics".into());
    }
    Ok((sweeps, work, peak))
}
