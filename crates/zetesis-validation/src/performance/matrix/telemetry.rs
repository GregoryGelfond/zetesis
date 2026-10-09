//! Reconcile authored route views with bounded typed JSON counters.
use super::{DeviceWork, Execution, FormulaResidualStatistics, Observation, Procedure};
use crate::selected::{Backend, Grounder, NativeExecution, Oracle};
use serde_json::Value;
use zetesis_backend::GpuApi;

mod hybrid;
mod terminal;

pub(super) fn observe(
    document: &Value,
    stderr: &[u8],
    request: NativeExecution,
) -> Result<Observation, String> {
    let timing = super::super::timing::parse_any(stderr)?;
    let hybrid = hybrid::read(document)?;
    let terminal = terminal::read(document)?;
    let hybrid_terminal = terminal_route(terminal.as_ref(), hybrid.is_some(), request)?;
    // An explicit request names the mode the cell must have taken; an
    // automatic request accepts either mode and retains the one observed.
    let taken = match timing.grounding_mode.as_str() {
        "eager" => Grounder::Eager,
        "lazy_interleaved" => Grounder::Lazy,
        "mixed" if hybrid.is_some() => Grounder::Lazy,
        "eager_base_terminal_definitions" if terminal.is_some() && !hybrid_terminal => {
            Grounder::Eager
        }
        "hybrid_base_terminal_definitions" if hybrid_terminal && hybrid.is_some() => Grounder::Lazy,
        _ => return Err("unsupported reported grounding mode".into()),
    };
    if request.grounder != Grounder::Auto && taken != request.grounder {
        return Err("reported grounding differs from requested matrix cell".into());
    }
    let statistics = &document["statistics"];
    if !statistics.is_object()
        || statistics["stage_timings"]["grounding_mode"] != timing.grounding_mode.as_str()
    {
        return Err("typed statistics and authored stage view disagree".into());
    }
    consistent_timings(statistics, &timing)?;
    let text = std::str::from_utf8(stderr).map_err(|e| e.to_string())?;
    let routes: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Backend: "))
        .collect();
    let [route] = routes.as_slice() else {
        return Err("missing or ambiguous actual backend metadata".into());
    };
    // `gpu` asks for the platform's native API; what ran is the concrete one.
    let (backend, adapter) = reported_backend(route)?;
    if backend.resolved_api() != request.backend.resolved_api() {
        return Err("actual backend differs from requested matrix cell".into());
    }
    let effective = one(
        text.lines()
            .filter_map(|line| line.trim_start().strip_prefix("effective execution: ")),
        "effective execution",
    )?;
    let effective_backend = field(effective, "backend")?;
    let grounding = if hybrid_terminal {
        "hybrid_base_terminal_definitions"
    } else if terminal.is_some() {
        "eager_base_terminal_definitions"
    } else if hybrid.is_some() {
        "hybrid"
    } else {
        taken.label()
    };
    if field(effective, "grounder")? != grounding {
        return Err("effective execution and measured grounding disagree".into());
    }
    let procedure = procedure(effective, request.oracle)?;
    let hybrid_mode = if hybrid_terminal {
        "hybrid_base_terminal_definitions"
    } else {
        "mixed"
    };
    if hybrid.is_some() && (timing.grounding_mode != hybrid_mode || procedure == Procedure::Closure)
    {
        return Err(
            "hybrid source checking requires mixed grounding and formula membership".into(),
        );
    }
    if let Some(receipt) = &terminal {
        terminal::execution(receipt, &timing, procedure)?;
    }
    if matches!(
        procedure,
        Procedure::PositiveConsequences | Procedure::StratifiedConsequences
    ) && (backend != Backend::Cpu || (taken != Grounder::Eager && hybrid.is_none()))
    {
        return Err("direct consequences require the eager CPU formula route".into());
    }
    let device = device(
        statistics,
        route,
        effective_backend,
        // An authenticated hybrid receipt establishes eager core membership
        // followed by host acceptance. Device counters describe that core, not
        // the original answer population checked by `hybrid::read`.
        if hybrid.is_some() {
            Grounder::Eager
        } else {
            taken
        },
        backend,
        procedure,
        adapter.as_deref(),
    )?;
    Ok(Observation {
        timing,
        hybrid,
        terminal,
        execution: Execution {
            backend,
            procedure,
            adapter,
            device,
        },
    })
}

fn procedure(effective: &str, request: Oracle) -> Result<Procedure, String> {
    let procedure = match field(effective, "oracle")? {
        "closure" => Procedure::Closure,
        "countermodel" => Procedure::Countermodel,
        "tight-support" => Procedure::TightSupport,
        "positive-consequences" => Procedure::PositiveConsequences,
        "stratified-consequences" => Procedure::StratifiedConsequences,
        _ => return Err("unsupported actual oracle metadata".into()),
    };
    let matches_request = match request {
        Oracle::Auto => true,
        Oracle::Closure => procedure == Procedure::Closure,
        Oracle::Countermodel => procedure == Procedure::Countermodel,
    };
    if !matches_request {
        return Err("actual oracle differs from explicit requested procedure".into());
    }
    Ok(procedure)
}

/// Decode the hardware actually named by the unique backend record. Requested
/// policy and effective execution are reconciled separately by the caller.
fn reported_backend(route: &str) -> Result<(Backend, Option<String>), String> {
    if route.starts_with("cpu") {
        return Ok((Backend::Cpu, None));
    }
    let prefix = route
        .strip_prefix("gpu (")
        .or_else(|| route.strip_prefix("hybrid GPU propagation + exact CPU residual search ("))
        .or_else(|| route.strip_prefix("GPU tight support ("))
        .ok_or("unsupported actual backend metadata")?;
    let (identity, _) = prefix
        .split_once("; vendor=")
        .ok_or("missing actual device vendor")?;
    let (adapter, api) = identity
        .rsplit_once(", ")
        .ok_or("missing actual device API")?;
    let api = match api {
        "Metal" => GpuApi::Metal,
        "Vulkan" => GpuApi::Vulkan,
        _ => return Err("actual device API is neither Metal nor Vulkan".into()),
    };
    if adapter.is_empty() {
        return Err("empty actual device name".into());
    }
    Ok((Backend::Gpu(Some(api)), Some(adapter.to_owned())))
}

fn one<'a>(mut values: impl Iterator<Item = &'a str>, label: &str) -> Result<&'a str, String> {
    let value = values.next().ok_or_else(|| format!("missing {label}"))?;
    if values.next().is_some() {
        return Err(format!("ambiguous {label}"));
    }
    Ok(value)
}
fn field<'a>(line: &'a str, name: &str) -> Result<&'a str, String> {
    let prefix = format!("{name}=");
    one(
        line.split("; ")
            .filter_map(|field| field.strip_prefix(&prefix)),
        name,
    )
}
fn device(
    statistics: &Value,
    route: &str,
    effective: &str,
    grounder: Grounder,
    backend: Backend,
    procedure: Procedure,
    adapter: Option<&str>,
) -> Result<DeviceWork, String> {
    let lazy_stats = statistics
        .get("lazy_execution")
        .ok_or("missing lazy activity field")?;
    let formula_stats = statistics
        .get("execution")
        .ok_or("missing formula activity field")?;
    if backend == Backend::Cpu {
        let expected = if formula_stats.is_null() {
            "cpu"
        } else {
            "cpu batched exact completion"
        };
        if effective != expected {
            return Err("primary and effective CPU routes disagree".into());
        }
        return cpu(statistics);
    }
    let api = backend.resolved_api().ok_or("missing actual device API")?;
    if procedure == Procedure::Closure {
        if effective != "requested GPU policy"
            || !route.starts_with("gpu (")
            || !formula_stats.is_null()
        {
            return Err("primary/effective closure route or activity disagrees".into());
        }
        if grounder == Grounder::Lazy {
            if !route.ends_with("; lazy immutable reduct rounds)") || !lazy_stats.is_object() {
                return Err("lazy GPU route requires its own activity record and route".into());
            }
            return lazy(lazy_stats, api, adapter);
        }
        if !route.contains("; static atoms=") || !lazy_stats.is_null() {
            return Err("static closure route has conflicting lazy activity".into());
        }
        return Ok(DeviceWork::Unavailable {
            reason: "static closure driver does not accumulate dispatch/transfer counters",
        });
    }
    if route.starts_with("GPU tight support (") || effective == "GPU tight support" {
        if grounder != Grounder::Eager
            || procedure != Procedure::TightSupport
            || effective != "GPU tight support"
            || !route.starts_with("GPU tight support (")
            || !formula_stats.is_object()
            || !lazy_stats.is_null()
        {
            return Err("tight GPU route requires its own activity record and route".into());
        }
        return tight(formula_stats, api, adapter);
    }
    if grounder != Grounder::Eager
        || effective != "hybrid GPU propagation + exact CPU residual search"
        || !route.starts_with("hybrid GPU propagation + exact CPU residual search (")
        || !formula_stats.is_object()
        || !lazy_stats.is_null()
    {
        return Err("hybrid GPU route requires its own activity record and route".into());
    }
    formula(formula_stats, api, adapter)
}

fn number(value: &Value, key: &str) -> Result<u64, String> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("missing or non-u64 execution counter: {key}"))
}
fn lazy(value: &Value, api: GpuApi, adapter: Option<&str>) -> Result<DeviceWork, String> {
    let submitted = number(value, "submitted_candidates")?;
    let completed = number(value, "completed_candidates")?;
    if value["backend"] != api.name()
        || value["adapter"].as_str() != adapter
        || number(value, "stopped_candidates")? != 0
        || number(value, "queued_results")? != 0
        || completed != submitted
    {
        return Err("lazy device completion/identity contradicts exhausted coverage".into());
    }
    let dispatches = number(value, "dispatches")?;
    let world_instances = number(value, "world_instances")?;
    let uploaded_bytes = number(value, "uploaded_bytes")?;
    let downloaded_bytes = number(value, "downloaded_bytes")?;
    if (dispatches == 0 && (world_instances != 0 || uploaded_bytes != 0 || downloaded_bytes != 0))
        || (dispatches > 0
            && (world_instances == 0 || uploaded_bytes == 0 || downloaded_bytes == 0))
    {
        return Err("lazy dispatch and transfer counts disagree".into());
    }
    Ok(DeviceWork::Lazy {
        dispatches,
        world_instances,
        uploaded_bytes,
        downloaded_bytes,
        completed_candidates: completed,
    })
}
fn formula(value: &Value, api: GpuApi, adapter: Option<&str>) -> Result<DeviceWork, String> {
    if !value["tight_work_per_candidate"].is_null() || !value["gpu_scheduled_work"].is_null() {
        return Err("propagation route reports tight-support work".into());
    }
    let activity = formula_activity(value, api, adapter)?;
    Ok(DeviceWork::Formula {
        batches: activity.batches,
        candidates: activity.candidates,
        work: activity.work,
        cpu_residuals: activity.residuals,
        gpu_residuals: residual_reasons(value, activity.residuals)?,
        peak_accounted_bytes: activity.peak_bytes,
    })
}

fn residual_reasons(
    value: &Value,
    expected: u64,
) -> Result<Option<FormulaResidualStatistics>, String> {
    let reasons = &value["gpu_residuals"];
    if reasons.is_null() {
        return Ok(None);
    }
    let reasons = FormulaResidualStatistics {
        fixed_point: number(reasons, "fixed_point")?,
        round_limit: number(reasons, "round_limit")?,
        work_limit: number(reasons, "work_limit")?,
    };
    // Complete observations already reconcile every decoded candidate with a
    // committed device or CPU verdict. Completion attempts can include retries
    // and are deliberately not the population counted by this device receipt.
    if reasons
        .fixed_point
        .checked_add(reasons.round_limit)
        .and_then(|sum| sum.checked_add(reasons.work_limit))
        != Some(expected)
    {
        return Err("decoded residual reasons contradict complete formula work".into());
    }
    Ok(Some(reasons))
}

struct FormulaActivity {
    batches: u64,
    candidates: u64,
    work: u64,
    residuals: u64,
    peak_bytes: u64,
}

fn formula_activity(
    value: &Value,
    api: GpuApi,
    adapter: Option<&str>,
) -> Result<FormulaActivity, String> {
    let adapter = adapter.ok_or("missing actual formula adapter")?;
    if !value["adapter"].as_str().is_some_and(|reported| {
        reported.starts_with(&format!("{adapter}, {}; vendor=", api.name()))
    }) {
        return Err("formula adapter disagrees with actual route".into());
    }
    let candidates = number(value, "gpu_candidates")?;
    let residuals = number(value, "cpu_residuals")?;
    let batches = number(value, "gpu_batches")?;
    if number(value, "gpu_decided")?.checked_add(residuals) != Some(candidates)
        || number(value, "pending_candidates")? != 0
        || number(value, "queued_models")? != 0
        || (batches == 0) != (candidates == 0)
        || batches > candidates
    {
        return Err("formula device accounting contradicts exhausted coverage".into());
    }
    completion(&value["completion"], candidates, residuals)?;
    Ok(FormulaActivity {
        batches,
        candidates,
        work: number(value, "gpu_work")?,
        residuals,
        peak_bytes: number(value, "peak_accounted_bytes")?,
    })
}

fn tight(value: &Value, api: GpuApi, adapter: Option<&str>) -> Result<DeviceWork, String> {
    let activity = formula_activity(value, api, adapter)?;
    let submitted_batches = number(value, "gpu_submitted_batches")?;
    let submitted_candidates = number(value, "gpu_submitted_candidates")?;
    let scheduled_work = number(value, "gpu_scheduled_work")?;
    let work_per_candidate_limit = number(value, "tight_work_per_candidate")?;
    if !value["gpu_residuals"].is_null()
        || value.get("gpu_limits") != Some(&Value::Null)
        || number(value, "gpu_rounds")? != 0
        || activity.residuals != 0
        || number(&value["completion"], "residuals")? != 0
        || submitted_batches < activity.batches
        || submitted_candidates < activity.candidates
        || (submitted_batches == 0) != (submitted_candidates == 0)
        || submitted_batches > submitted_candidates
        || activity.work > scheduled_work
        || u128::from(scheduled_work)
            > u128::from(submitted_candidates) * u128::from(work_per_candidate_limit)
        || u128::from(activity.work)
            > u128::from(activity.candidates) * u128::from(work_per_candidate_limit)
    {
        return Err("tight support counters or limits contradict the reported primitive".into());
    }
    // One immutable plan performs the same complete scan in every candidate.
    // With no decoded candidate, submitted work remains a scheduling receipt;
    // it must not be relabelled as completed work or propagation sweeps.
    if activity.candidates > 0
        && (activity.work % activity.candidates != 0
            || u128::from(scheduled_work) * u128::from(activity.candidates)
                != u128::from(activity.work) * u128::from(submitted_candidates))
    {
        return Err("tight support scan work disagrees across candidate counts".into());
    }
    Ok(DeviceWork::TightSupport {
        batches: activity.batches,
        candidates: activity.candidates,
        work: activity.work,
        submitted_batches,
        submitted_candidates,
        scheduled_work,
        work_per_candidate_limit,
        peak_accounted_bytes: activity.peak_bytes,
    })
}

/// Attempt counters may include uncommitted work and retries. Their `complete`
/// field certifies only that diagnostic aggregation did not overflow.
fn completion(value: &Value, candidates: u64, committed_residuals: u64) -> Result<(), String> {
    let entered = number(value, "entered")?;
    let completed = number(value, "completed")?;
    let failed = number(value, "failed")?;
    let residuals = number(value, "residuals")?;
    let residual_completed = number(value, "residual_completed")?;
    let residual_failed = number(value, "residual_failed")?;
    if value["complete"] != true
        || completed.checked_add(failed) != Some(entered)
        || residual_completed.checked_add(residual_failed) != Some(residuals)
        || residuals > entered
        || residual_completed > completed
        || residual_failed > failed
        || completed < candidates
        || residual_completed < committed_residuals
        || completed - residual_completed < candidates - committed_residuals
    {
        return Err("completion attempts contradict committed formula work".into());
    }
    Ok(())
}

fn consistent_timings(
    statistics: &Value,
    timing: &super::super::Diagnostics,
) -> Result<(), String> {
    let phases = &statistics["phase_timings"];
    // The original unversioned text profile also admitted typed records without
    // a schema field. Newer profiles require their exact explicit version.
    let schema_matches = match phases.get("schema") {
        None => timing.phase_schema == 1,
        Some(schema) => schema.as_u64() == Some(u64::from(timing.phase_schema)),
    };
    if !schema_matches
        || phases["measurements"].as_object().map(serde_json::Map::len) != Some(timing.phases.len())
    {
        return Err("typed and text phase schemas disagree".into());
    }
    for (section, measurements) in [
        ("stage_timings", &timing.stages),
        ("phase_timings", &timing.phases),
    ] {
        let section = &statistics[section];
        if section["driver_elapsed_ns"].as_u64() != Some(timing.driver_elapsed_ns) {
            return Err("typed and text driver clocks disagree".into());
        }
        for (name, expected) in measurements {
            let actual = section["measurements"]
                .get(name)
                .ok_or("missing typed phase/stage measurement")?;
            match expected {
                None if actual.is_null() => {}
                Some(expected)
                    if actual["calls"].as_u64() == Some(expected.calls)
                        && actual["elapsed_ns"].as_u64() == Some(expected.elapsed_ns)
                        && actual["complete"] == true => {}
                _ => return Err("typed and text phase/stage measurements disagree".into()),
            }
        }
    }
    if statistics["stage_timings"]["complete"] != true
        || statistics["stage_timings"]["unattributed_elapsed_ns"].as_u64()
            != Some(timing.unattributed_elapsed_ns)
    {
        return Err("typed and text stage completeness disagree".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

fn cpu(statistics: &Value) -> Result<DeviceWork, String> {
    if !statistics["lazy_execution"].is_null() {
        return Err("CPU route contradicts lazy device evidence".into());
    }
    let execution = &statistics["execution"];
    if !execution.is_null() {
        if !execution["adapter"].is_null()
            || !execution["gpu_residuals"].is_null()
            || !execution["tight_work_per_candidate"].is_null()
            || !execution["gpu_scheduled_work"].is_null()
        {
            return Err(
                "CPU route reports device identity, residual reasons or tight-support work".into(),
            );
        }
        for counter in [
            "gpu_batches",
            "gpu_candidates",
            "gpu_work",
            "gpu_rounds",
            "gpu_decided",
            "peak_accounted_bytes",
            "pending_candidates",
            "queued_models",
        ] {
            if number(execution, counter)? != 0 {
                return Err("CPU route reports GPU work".into());
            }
        }
        let residuals = number(execution, "cpu_residuals")?;
        completion(&execution["completion"], residuals, residuals)?;
    }
    Ok(DeviceWork::Cpu)
}

/// Whether a terminal receipt is of a hybrid base, once its route agrees with
/// the request: an eager base is automatic grounding's, with no source
/// checking; a hybrid one is lazy (or automatic) grounding's, with both receipts.
fn terminal_route(
    terminal: Option<&super::TerminalStatistics>,
    hybrid: bool,
    request: NativeExecution,
) -> Result<bool, String> {
    match terminal {
        Some(receipt) if receipt.hybrid_base => {
            if !hybrid || request.grounder == Grounder::Eager {
                return Err(
                    "a hybrid terminal base requires lazy grounding and its checking receipt"
                        .into(),
                );
            }
            Ok(true)
        }
        Some(_) if hybrid || request.grounder != Grounder::Auto => {
            Err("terminal definitions require automatic grounding and a distinct route".into())
        }
        Some(_) | None => Ok(false),
    }
}
