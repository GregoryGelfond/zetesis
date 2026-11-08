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
    let generation = route(stderr, backend)?.generation;
    let storage = storage(stderr, generation)?;
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
        generation,
    } = route(stderr, backend)?;
    let [batches, candidates, work, sweeps, decided, residuals] = gpu_counters(stderr, generation)?;
    let Storage {
        pending,
        queued,
        peak_bytes,
        batch_limit,
        byte_limit,
        sweep_limit,
        work_limit,
        adaptive,
        submitted,
        slates,
    } = storage(stderr, generation)?;
    let submission_limit = submission_limit(batch_limit, adaptive, slates, batches, candidates)?;
    if submitted.is_some_and(|(submitted_batches, submitted_candidates)| {
        submitted_batches < batches || submitted_candidates < candidates
    }) {
        return Err("formula GPU submissions are fewer than their decoded results".into());
    }
    let (
        [
            _,
            _,
            search_candidates,
            queries,
            witnesses,
            _,
            stable_models,
        ],
        classical_queries,
    ) = countermodel_counters(stderr)?;
    let completion = completion::record(
        stderr,
        generation,
        request,
        candidates,
        residuals,
        submission_limit,
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
        // A record of the removed clause search also counts its outer queries:
        // one per proposal, then exactly one final query establishing
        // exhaustion.
        || classical_queries.is_some_and(|queries| candidates.checked_add(1) != Some(queries))
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
            || u128::from(candidates) > u128::from(batches) * u128::from(submission_limit)
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

/// The decoded device counters: batches, candidates, charged work, completed
/// sweeps, GPU-decided candidates and CPU residuals. Released executables name
/// the line "decoded" and the work "primitive work"; the checkpoints did not.
fn gpu_counters(text: &str, generation: Generation) -> Result<[u64; 6], String> {
    let (prefix, work) = match generation {
        Generation::Released { .. } => ("  formula GPU decoded: ", "primitive work"),
        Generation::ScalarCheckpoint | Generation::BoundedCheckpoint { .. } => {
            ("  formula GPU: ", "propagation work")
        }
    };
    fields(
        text,
        prefix,
        [
            "batches",
            "candidates",
            work,
            "completed sweeps",
            "GPU-decided committed",
            "CPU residuals completed",
        ],
    )
}

/// The countermodel line's counters, and the outer queries that records of
/// the removed clause search also carry before the verified count.
fn countermodel_counters(text: &str) -> Result<([u64; 7], Option<u64>), String> {
    let value = line(text, "  countermodel: ")?;
    if value.contains("; classical queries=") {
        let [
            work,
            decisions,
            candidates,
            queries,
            witnesses,
            restrictions,
            classical,
            stable,
        ] = field_values(
            value,
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
        )?;
        let counters = [
            work,
            decisions,
            candidates,
            queries,
            witnesses,
            restrictions,
            stable,
        ];
        return Ok((counters, Some(classical)));
    }
    let counters = field_values(
        value,
        [
            "search work",
            "decisions",
            "candidates",
            "queries",
            "witnesses",
            "candidate restrictions",
            "verified stable models",
        ],
    )?;
    Ok((counters, None))
}

/// The statistics a native executable printed, identified by its execution
/// header. A record is read only as the generation its header names; no field
/// is borrowed from another generation's layout.
#[derive(Clone, Copy, Debug)]
pub(super) enum Generation {
    /// Pre-release checkpoints of 2026-09-07 to 2026-09-12, before bounded
    /// completion: one scalar CPU search worker. No tagged release prints it.
    ScalarCheckpoint,
    /// Pre-release checkpoints of 2026-09-12, the first bounded-completion
    /// statistics (`peak effective workers`). No tagged release prints it.
    BoundedCheckpoint { requested: u64, effective: u64 },
    /// Every tagged release from v0.1.7 (2026-10-01) on, including qualified
    /// baseline 5816005a and the current command line: bounded completion
    /// reported as `peak preflight workers`, the maximum query concurrency
    /// selected at preflight.
    Released { requested: u64, effective: u64 },
}

struct PhysicalRoute {
    adapter: String,
    api: String,
    vendor_id: u32,
    generation: Generation,
}

fn route(stderr: &str, backend: Backend) -> Result<PhysicalRoute, String> {
    if line(stderr, "  completion: ")? != "exhausted" {
        return Err("formula execution statistics do not establish exhaustion".into());
    }
    let effective = line(stderr, "  effective execution: ")?;
    let header = effective.strip_prefix("backend=hybrid GPU propagation + exact CPU residual search; oracle=countermodel; grounder=eager; ")
        .ok_or("requested formula GPU route was not reported")?;
    let (adapter, generation) =
        if let Some(adapter) = header.strip_prefix("CPU search workers=1; adapter=") {
            (adapter, Generation::ScalarCheckpoint)
        } else {
            let (requested, remaining) = header
                .split_once("; ")
                .ok_or("missing completion concurrency")?;
            let requested = number(requested, "CPU completion requested workers")?;
            let (effective, adapter) = remaining
                .split_once("; adapter=")
                .ok_or("missing effective completion concurrency")?;
            let generation = if effective.starts_with("peak preflight workers=") {
                Generation::Released {
                    requested,
                    effective: number(effective, "peak preflight workers")?,
                }
            } else {
                Generation::BoundedCheckpoint {
                    requested,
                    effective: number(effective, "peak effective workers")?,
                }
            };
            (adapter, generation)
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
        generation,
    })
}

struct Storage {
    pending: u64,
    queued: u64,
    peak_bytes: u64,
    /// The configured batch size: every slate's size under fixed batching, the
    /// first and reset slate size under adaptive batching.
    batch_limit: u64,
    byte_limit: u64,
    sweep_limit: u64,
    work_limit: u64,
    /// Whether slates adapt their size, which only released executables from
    /// the GPU release on report.
    adaptive: bool,
    /// Submitted batches and candidates, which released records report.
    submitted: Option<(u64, u64)>,
    /// Committed slates and the largest, which records from the GPU release on
    /// report.
    slates: Option<Slates>,
}

#[derive(Clone, Copy)]
struct Slates {
    committed: u64,
    largest: Option<u64>,
}

fn storage(text: &str, generation: Generation) -> Result<Storage, String> {
    match generation {
        Generation::Released { .. } => released_storage(text),
        Generation::BoundedCheckpoint { .. } => {
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
                adaptive: false,
                submitted: None,
                slates: None,
            })
        }
        Generation::ScalarCheckpoint => {
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
                adaptive: false,
                submitted: None,
                slates: None,
            })
        }
    }
}

/// The released layout. Batch limits are fixed (`candidates=`) through
/// qualified baseline 5816005a, or adaptive from the GPU release on. The
/// resources line ends at the peak bytes in 5816005a, adds
/// `GPU kernel timing=unavailable` through v0.6.0, and the workgroup size from
/// the GPU release on. Adaptive limits always come with the slates line.
fn released_storage(text: &str) -> Result<Storage, String> {
    let [pending, queued] = fields(
        text,
        "  formula accounting: ",
        ["pending candidates", "queued verified models"],
    )?;
    let limits = line(text, "  formula batch limits: ")?;
    let (adaptive, [batch_limit, byte_limit]) =
        if let Some(adaptive) = limits.strip_prefix("adaptive slates; ") {
            (
                true,
                field_values(adaptive, ["initial and reset slate size", "pending bytes"])?,
            )
        } else {
            (
                false,
                field_values(limits, ["candidates", "pending bytes"])?,
            )
        };
    let [sweep_limit, work_limit] = fields(
        text,
        "  formula GPU limits: ",
        ["propagation sweeps/candidate", "propagation work/candidate"],
    )?;
    let resources = line(text, "  formula GPU resources: ")?;
    let (peak, suffix) = resources
        .split_once("; ")
        .map_or((resources, None), |(peak, suffix)| (peak, Some(suffix)));
    let peak_bytes = number(peak, "peak authored GPU bytes")?;
    let suffix_known = match suffix {
        None | Some("GPU kernel timing=unavailable" | "workgroup size=unavailable") => true,
        Some(size) => number(size, "workgroup size").is_ok(),
    };
    if !suffix_known {
        return Err("unsupported formula GPU-resource statistics".into());
    }
    let submitted = line(text, "  formula GPU submitted: ")?
        .strip_suffix("; completed work/sweeps for unreturned results=unavailable")
        .ok_or("unsupported formula GPU-submission statistics")?;
    let [submitted_batches, submitted_candidates] =
        field_values(submitted, ["batches", "candidates"])?;
    let slates = if text
        .lines()
        .any(|line| line.starts_with("  formula GPU slates: "))
    {
        Some(slates(text)?)
    } else if adaptive {
        return Err("adaptive slates without their slate statistics".into());
    } else {
        None
    };
    if batch_limit == 0 || byte_limit == 0 || work_limit > u64::from(u32::MAX) {
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
        adaptive,
        submitted: Some((submitted_batches, submitted_candidates)),
        slates,
    })
}

fn slates(text: &str) -> Result<Slates, String> {
    let value = line(text, "  formula GPU slates: ")?
        .strip_suffix("; each slate takes one or more submitted batches")
        .ok_or("unsupported formula GPU-slate statistics")?;
    let mut parts = value.split("; ");
    let committed = number(parts.next().ok_or("missing committed slates")?, "committed")?;
    let largest = match parts.next().ok_or("missing largest slate")? {
        "largest=none" => None,
        largest => Some(number(largest, "largest")?),
    };
    number(
        parts.next().ok_or("missing slate feedback resets")?,
        "feedback resets",
    )?;
    if parts.next().is_some() {
        return Err("unexpected formula GPU-slate fields".into());
    }
    Ok(Slates { committed, largest })
}

/// The most candidates one device submission may carry: the batch size under
/// fixed batching, the largest committed slate under adaptive batching. Slate
/// statistics, when reported, must agree with the decoded counters: every
/// committed slate decoded at least one batch, the largest slate is one of the
/// committed candidates' slates, and fixed slates never exceed the batch size.
fn submission_limit(
    batch_limit: u64,
    adaptive: bool,
    slates: Option<Slates>,
    batches: u64,
    candidates: u64,
) -> Result<u64, String> {
    let Some(Slates { committed, largest }) = slates else {
        return Ok(batch_limit);
    };
    let consistent = (committed == 0) == (candidates == 0)
        && (committed == 0) == largest.is_none()
        && committed <= batches
        && largest.is_none_or(|largest| largest <= candidates)
        && (adaptive || largest.is_none_or(|largest| largest <= batch_limit));
    if !consistent {
        return Err("formula GPU slate statistics contradict the decoded counters".into());
    }
    Ok(if adaptive {
        largest.unwrap_or(0)
    } else {
        batch_limit
    })
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
