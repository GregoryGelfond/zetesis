//! Strict qualification of the native CLI's authored formula statistics.
//!
//! These are execution records from a trusted solver, not hardware attestation.
//! Missing or changed telemetry cannot become evidence of physical execution.

use serde::Serialize;

mod completion;
pub(crate) use completion::CompletionRequest;

use crate::corpus_comparison::normalize::Answer;
use zetesis_backend::Backend;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Status {
    GpuExercised,
    OuterUnsatWithoutMembership,
}

#[derive(Debug, Serialize)]
pub(crate) struct FormulaExecution {
    pub(crate) status: Status,
    pub(crate) adapter: String,
    pub(crate) api: String,
    pub(crate) vendor_id: u32,
    pub(crate) gpu_batches: u64,
    pub(crate) gpu_candidates: u64,
    pub(crate) gpu_work: u64,
    pub(crate) gpu_sweeps: u64,
    pub(crate) gpu_decided: u64,
    pub(crate) cpu_residuals: u64,
    pub(crate) countermodel_witnesses: u64,
    pub(crate) verified_stable_models: u64,
    pub(crate) peak_authored_gpu_bytes: u64,
    pub(crate) completion: Option<completion::CompletionExecution>,
}

#[cfg(test)]
fn formula(
    stderr: &str,
    backend: Backend,
    batch_size: usize,
    answer: &Answer,
) -> Result<FormulaExecution, String> {
    qualify(stderr, backend, batch_size, None, answer)
}

#[cfg(test)]
fn formula_for_request(
    stderr: &str,
    backend: Backend,
    batch_size: usize,
    request: CompletionRequest,
    answer: &Answer,
) -> Result<FormulaExecution, String> {
    qualify(stderr, backend, batch_size, Some(request), answer)
}

/// Ordinary controls do not prescribe a device batch or completion scratch
/// share. Qualify every observed consistency relation, then verify that the
/// reported completion plan stays inside the public thread/memory request.
pub(crate) fn formula_for_policy(
    stderr: &str,
    backend: Backend,
    threads: std::num::NonZeroUsize,
    memory_bytes: Option<u64>,
    answer: &Answer,
) -> Result<FormulaExecution, String> {
    let concurrency = route(stderr, backend)?.concurrency;
    let storage = storage(stderr, concurrency.is_some())?;
    let batch =
        usize::try_from(storage.batch_limit).map_err(|_| "device batch does not fit host width")?;
    let execution = qualify(stderr, backend, batch, None, answer)?;
    if let Some(completion) = &execution.completion {
        if u128::from(completion.requested_workers) > threads.get() as u128
            || memory_bytes.is_some_and(|memory| {
                completion.max_logical_scratch_bytes > memory || storage.byte_limit > memory
            })
        {
            return Err("reported execution plan exceeds public resource controls".into());
        }
    } else if memory_bytes.is_some() {
        return Err("legacy telemetry cannot establish an explicit memory allowance".into());
    }
    Ok(execution)
}

fn qualify(
    stderr: &str,
    backend: Backend,
    batch_size: usize,
    request: Option<CompletionRequest>,
    answer: &Answer,
) -> Result<FormulaExecution, String> {
    let PhysicalRoute {
        adapter,
        api,
        vendor_id,
        concurrency,
    } = route(stderr, backend)?;
    let [batches, candidates, work, sweeps, decided, residuals] = gpu_counters(stderr)?;
    let Storage {
        pending,
        queued,
        peak_bytes,
        batch_limit,
        byte_limit,
        sweep_limit,
        work_limit,
    } = storage(stderr, concurrency.is_some())?;
    let [
        _,
        _,
        search_candidates,
        queries,
        witnesses,
        _,
        classical_queries,
        stable_models,
    ] = countermodel_counters(stderr)?;
    let completion = completion::record(
        stderr,
        concurrency,
        request,
        candidates,
        residuals,
        batch_limit,
    )?;
    if u128::from(batch_limit) != batch_size as u128
        || pending != 0
        || queued != 0
        || candidates != search_candidates
        || queries != residuals
        || witnesses > residuals
        || decided.checked_add(residuals) != Some(candidates)
        || decided.checked_add(residuals - witnesses) != Some(stable_models)
        || stable_models < answer.model_count
        || (answer.cost.is_none() && stable_models != answer.model_count)
        // The completed CLI uses only the batch route: one query per proposal,
        // followed by exactly one final query establishing exhaustion.
        || candidates.checked_add(1) != Some(classical_queries)
    {
        return Err(
            "formula execution counters contradict complete candidate/model accounting".into(),
        );
    }
    let status = if candidates == 0 {
        if answer.satisfiable || batches != 0 || work != 0 || sweeps != 0 || peak_bytes != 0 {
            return Err(
                "zero-dispatch execution is not a completed outer-search UNSAT case".into(),
            );
        }
        Status::OuterUnsatWithoutMembership
    } else {
        if batches == 0
            || batches > candidates
            || work == 0
            || peak_bytes == 0
            || u128::from(candidates) > u128::from(batches) * u128::from(batch_limit)
            || peak_bytes > byte_limit
            || u128::from(work) > u128::from(candidates) * u128::from(work_limit)
            || u128::from(sweeps) > u128::from(candidates) * u128::from(sweep_limit)
        {
            return Err("formula GPU dispatch counts or authored limits are inconsistent".into());
        }
        Status::GpuExercised
    };
    Ok(FormulaExecution {
        status,
        adapter,
        api,
        vendor_id,
        gpu_batches: batches,
        gpu_candidates: candidates,
        gpu_work: work,
        gpu_sweeps: sweeps,
        gpu_decided: decided,
        cpu_residuals: residuals,
        countermodel_witnesses: witnesses,
        verified_stable_models: stable_models,
        peak_authored_gpu_bytes: peak_bytes,
        completion,
    })
}

fn gpu_counters(text: &str) -> Result<[u64; 6], String> {
    fields(
        text,
        "  formula GPU: ",
        [
            "batches",
            "candidates",
            "propagation work",
            "completed sweeps",
            "GPU-decided committed",
            "CPU residuals completed",
        ],
    )
}

fn countermodel_counters(text: &str) -> Result<[u64; 8], String> {
    fields(
        text,
        "  countermodel: ",
        [
            "search work",
            "decisions",
            "candidates",
            "queries",
            "witnesses",
            "candidate restrictions",
            "classical queries",
            "verified stable models",
        ],
    )
}

struct PhysicalRoute {
    adapter: String,
    api: String,
    vendor_id: u32,
    concurrency: Option<(u64, u64)>,
}

fn route(stderr: &str, backend: Backend) -> Result<PhysicalRoute, String> {
    if line(stderr, "  completion: ")? != "exhausted" {
        return Err("formula execution statistics do not establish exhaustion".into());
    }
    let effective = line(stderr, "  effective execution: ")?;
    let header = effective.strip_prefix("backend=hybrid GPU propagation + exact CPU residual search; oracle=countermodel; grounder=eager; ")
        .ok_or("requested formula GPU route was not reported")?;
    let (adapter, concurrency) =
        if let Some(adapter) = header.strip_prefix("CPU search workers=1; adapter=") {
            (adapter, None)
        } else {
            let (requested, remaining) = header
                .split_once("; ")
                .ok_or("missing completion concurrency")?;
            let (effective, adapter) = remaining
                .split_once("; adapter=")
                .ok_or("missing effective completion concurrency")?;
            (
                adapter,
                Some((
                    number(requested, "CPU completion requested workers")?,
                    number(effective, "peak effective workers")?,
                )),
            )
        };
    let (name_api, vendor) = adapter
        .rsplit_once("; vendor=0x")
        .ok_or("missing physical adapter vendor")?;
    let (name, api) = name_api
        .rsplit_once(", ")
        .ok_or("missing physical adapter API")?;
    if name.is_empty()
        || name.chars().any(char::is_control)
        || vendor.is_empty()
        || vendor.len() > 8
        || !vendor.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("malformed physical adapter identity".into());
    }
    let vendor_id = u32::from_str_radix(vendor, 16).map_err(|error| error.to_string())?;
    if !matches_backend(backend, api) {
        return Err("actual adapter does not match the explicit physical backend".into());
    }
    Ok(PhysicalRoute {
        adapter: adapter.into(),
        api: api.into(),
        vendor_id,
        concurrency,
    })
}

struct Storage {
    pending: u64,
    queued: u64,
    peak_bytes: u64,
    batch_limit: u64,
    byte_limit: u64,
    sweep_limit: u64,
    work_limit: u64,
}

fn storage(text: &str, current: bool) -> Result<Storage, String> {
    if current {
        let [pending, queued] = fields(
            text,
            "  formula accounting: ",
            ["pending candidates", "queued verified models"],
        )?;
        let [batch_limit, byte_limit] = fields(
            text,
            "  formula batch limits: ",
            ["candidates", "pending bytes"],
        )?;
        let (sweep_limit, work_limit, peak_bytes) = completion::gpu_limits(text)?;
        if batch_limit == 0 || byte_limit == 0 {
            return Err("unsupported formula batch-limit statistics".into());
        }
        Ok(Storage {
            pending,
            queued,
            peak_bytes,
            batch_limit,
            byte_limit,
            sweep_limit,
            work_limit,
        })
    } else {
        let [pending, queued, peak_bytes] = fields(
            text,
            "  formula accounting: ",
            [
                "pending candidates",
                "queued verified models",
                "peak authored GPU bytes",
            ],
        )?;
        let (batch_limit, byte_limit, sweep_limit, work_limit) = batch_limits(text)?;
        Ok(Storage {
            pending,
            queued,
            peak_bytes,
            batch_limit,
            byte_limit,
            sweep_limit,
            work_limit,
        })
    }
}

/// Whether the adapter's reported API is the one the request resolves to: the
/// named API, or the platform's native one for `gpu`. The CPU matches none.
fn matches_backend(backend: Backend, api: &str) -> bool {
    backend
        .resolved_api()
        .is_some_and(|resolved| resolved.name() == api)
}

fn line<'a>(text: &'a str, prefix: &str) -> Result<&'a str, String> {
    let mut matches = text.lines().filter_map(|line| line.strip_prefix(prefix));
    let value = matches
        .next()
        .ok_or_else(|| format!("missing native statistics: {prefix}"))?;
    if matches.next().is_some() {
        return Err(format!("duplicate native statistics: {prefix}"));
    }
    Ok(value)
}

fn number(text: &str, key: &str) -> Result<u64, String> {
    let value = text
        .strip_prefix(key)
        .and_then(|value| value.strip_prefix('='))
        .ok_or_else(|| format!("missing native statistic field: {key}"))?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("invalid native statistic field: {key}"));
    }
    value
        .parse()
        .map_err(|error| format!("invalid {key}: {error}"))
}

fn fields<const N: usize>(text: &str, prefix: &str, keys: [&str; N]) -> Result<[u64; N], String> {
    field_values(line(text, prefix)?, keys)
}

fn field_values<const N: usize>(value: &str, keys: [&str; N]) -> Result<[u64; N], String> {
    let mut parts = value.split("; ");
    let mut result = [0; N];
    for (slot, key) in result.iter_mut().zip(keys) {
        *slot = number(parts.next().ok_or("missing native statistic field")?, key)?;
    }
    if parts.next().is_some() {
        return Err("unexpected native statistic fields".into());
    }
    Ok(result)
}

fn batch_limits(text: &str) -> Result<(u64, u64, u64, u64), String> {
    let mut parts = line(text, "  formula batch limits: ")?.split("; ");
    let candidates = number(
        parts.next().ok_or("missing formula batch limit")?,
        "candidates",
    )?;
    let bytes = number(
        parts.next().ok_or("missing formula byte limit")?,
        "pending bytes",
    )?;
    let sweeps = number(
        parts.next().ok_or("missing formula sweep limit")?,
        "propagation sweeps",
    )?;
    let work = parts
        .next()
        .and_then(|part| part.strip_suffix(" (u32 ceiling)"))
        .ok_or("missing formula propagation allowance")?;
    let work = number(work, "propagation work/candidate")?;
    if parts.next() != Some("GPU kernel timing=unavailable")
        || parts.next().is_some()
        || candidates == 0
        || bytes == 0
        || work > u64::from(u32::MAX)
    {
        return Err("unsupported formula batch-limit statistics".into());
    }
    Ok((candidates, bytes, sweeps, work))
}

#[cfg(test)]
mod tests;
