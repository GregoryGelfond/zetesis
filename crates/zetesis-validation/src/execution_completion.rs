//! Strict current completion telemetry, separate from historical scalar records.

use std::num::NonZeroUsize;

use serde::Serialize;

use super::{field_values, fields, line, number};

#[derive(Clone, Copy)]
pub(crate) struct CompletionRequest {
    pub(crate) workers: NonZeroUsize,
    pub(crate) max_scratch_bytes: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct CompletionExecution {
    pub(crate) profile: &'static str,
    pub(crate) requested_workers: u64,
    pub(crate) peak_effective_workers: u64,
    pub(crate) entered: u64,
    pub(crate) residuals: u64,
    pub(crate) completed: u64,
    pub(crate) residual_completed: u64,
    pub(crate) failed: u64,
    pub(crate) residual_failed: u64,
    pub(crate) peak_admitted_logical_scratch_bytes: u64,
    pub(crate) max_logical_scratch_bytes: u64,
}

pub(super) fn record(
    text: &str,
    concurrency: Option<(u64, u64)>,
    request: Option<CompletionRequest>,
    candidates: u64,
    residuals: u64,
    batch_limit: u64,
) -> Result<Option<CompletionExecution>, String> {
    if let Some((requested, effective)) = concurrency {
        return qualify(
            text,
            requested,
            effective,
            request,
            candidates,
            residuals,
            batch_limit,
        )
        .map(Some);
    }
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

pub(super) fn qualify(
    text: &str,
    requested: u64,
    effective: u64,
    request: Option<CompletionRequest>,
    candidates: u64,
    residuals: u64,
    batch_limit: u64,
) -> Result<CompletionExecution, String> {
    let value = line(text, "  formula completion: ")?
        .strip_suffix("; counters overflowed=false")
        .ok_or("missing, malformed or overflowed completion counters")?;
    let [entered, exact, completed, failed, peak, limit] = field_values(
        value,
        [
            "entered",
            "residuals entered",
            "completed before commit",
            "failed",
            "peak admitted logical scratch bytes",
            "scratch limit",
        ],
    )?;
    let [residual_completed, residual_failed] = fields(
        text,
        "  formula residual completion: ",
        ["completed locally", "failed"],
    )?;
    if requested == 0
        || effective > requested
        || effective > residuals
        || effective > batch_limit
        || (effective == 0) != (residuals == 0)
        || entered != candidates
        || exact != residuals
        || completed != candidates
        || failed != 0
        || residual_completed != residuals
        || residual_failed != 0
        || peak > limit
        || (peak == 0) != (candidates == 0)
        || request.is_some_and(|r| {
            u128::from(requested) != r.workers.get() as u128 || limit != r.max_scratch_bytes
        })
    {
        return Err(
            "bounded completion statistics contradict the complete native request/accounting"
                .into(),
        );
    }
    Ok(CompletionExecution {
        profile: "bounded_completion_v1",
        requested_workers: requested,
        peak_effective_workers: effective,
        entered,
        residuals: exact,
        completed,
        failed,
        residual_completed,
        residual_failed,
        peak_admitted_logical_scratch_bytes: peak,
        max_logical_scratch_bytes: limit,
    })
}

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
