//! Reconcile authored route views with bounded typed JSON counters.
use super::{DeviceWork, Execution, Observation, Procedure};
use crate::selected::{Backend, Grounder, NativeExecution, Oracle};
use serde_json::Value;

pub(super) fn observe(
    document: &Value,
    stderr: &[u8],
    request: NativeExecution,
) -> Result<Observation, String> {
    let timing = super::super::timing::parse_any(stderr)?;
    // An explicit request names the mode the cell must have taken; an
    // automatic request accepts either mode and retains the one observed.
    let taken = match timing.grounding_mode.as_str() {
        "eager" => Grounder::Eager,
        "lazy_interleaved" => Grounder::Lazy,
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
    let (backend, adapter) = if route.starts_with("cpu") {
        (Backend::Cpu, None)
    } else {
        let prefix = route
            .strip_prefix("gpu (")
            .or_else(|| route.strip_prefix("hybrid GPU propagation + exact CPU residual search ("))
            .ok_or("unsupported actual backend metadata")?;
        let (adapter, _) = prefix
            .split_once(", Metal; vendor=")
            .ok_or("actual device API is not Metal")?;
        if adapter.is_empty() {
            return Err("empty actual device name".into());
        }
        (Backend::Metal, Some(adapter.to_owned()))
    };
    if backend != request.backend {
        return Err("actual backend differs from requested matrix cell".into());
    }
    let effective = one(
        text.lines()
            .filter_map(|line| line.trim_start().strip_prefix("effective execution: ")),
        "effective execution",
    )?;
    let effective_backend = field(effective, "backend")?;
    if field(effective, "grounder")? != taken.label() {
        return Err("effective execution and measured grounding disagree".into());
    }
    let procedure = match field(effective, "oracle")? {
        "closure" => Procedure::Closure,
        "countermodel" => Procedure::Countermodel,
        "tight-support" => Procedure::TightSupport,
        "positive-consequences" => Procedure::PositiveConsequences,
        _ => return Err("unsupported actual oracle metadata".into()),
    };
    let matches_request = match request.oracle {
        Oracle::Auto => true,
        Oracle::Closure => procedure == Procedure::Closure,
        Oracle::Countermodel => procedure == Procedure::Countermodel,
    };
    if !matches_request {
        return Err("actual oracle differs from explicit requested procedure".into());
    }
    if procedure == Procedure::PositiveConsequences
        && (backend != Backend::Cpu || taken != Grounder::Eager)
    {
        return Err("positive consequences require the eager CPU formula route".into());
    }
    let device = device(
        statistics,
        route,
        effective_backend,
        taken,
        backend,
        procedure,
        adapter.as_deref(),
    )?;
    Ok(Observation {
        timing,
        execution: Execution {
            backend,
            procedure,
            adapter,
            device,
        },
    })
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
    if procedure == Procedure::Closure {
        if effective != "requested GPU policy"
            || !route.starts_with("gpu (")
            || !formula_stats.is_null()
        {
            return Err("primary/effective closure route or activity disagrees".into());
        }
        if grounder == Grounder::Lazy {
            if !route.ends_with("; lazy immutable reduct rounds)") || !lazy_stats.is_object() {
                return Err("lazy Metal requires its own activity record and route".into());
            }
            return lazy(lazy_stats, adapter);
        }
        if !route.contains("; static atoms=") || !lazy_stats.is_null() {
            return Err("static closure route has conflicting lazy activity".into());
        }
        return Ok(DeviceWork::Unavailable {
            reason: "static closure driver does not accumulate dispatch/transfer counters",
        });
    }
    if grounder != Grounder::Eager
        || effective != "hybrid GPU propagation + exact CPU residual search"
        || !route.starts_with("hybrid GPU propagation + exact CPU residual search (")
        || !formula_stats.is_object()
        || !lazy_stats.is_null()
    {
        return Err("hybrid Metal requires its own activity record and route".into());
    }
    formula(formula_stats, adapter)
}

fn number(value: &Value, key: &str) -> Result<u64, String> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("missing or non-u64 execution counter: {key}"))
}
fn lazy(value: &Value, adapter: Option<&str>) -> Result<DeviceWork, String> {
    let submitted = number(value, "submitted_candidates")?;
    let completed = number(value, "completed_candidates")?;
    if value["backend"] != "Metal"
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
fn formula(value: &Value, adapter: Option<&str>) -> Result<DeviceWork, String> {
    let adapter = adapter.ok_or("missing actual formula adapter")?;
    if !value["adapter"]
        .as_str()
        .is_some_and(|reported| reported.starts_with(&format!("{adapter}, Metal; vendor=")))
    {
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
    Ok(DeviceWork::Formula {
        batches,
        candidates,
        work: number(value, "gpu_work")?,
        cpu_residuals: residuals,
        peak_accounted_bytes: number(value, "peak_accounted_bytes")?,
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
#[path = "../../../tests/support/matrix_telemetry.rs"]
mod tests;

fn cpu(statistics: &Value) -> Result<DeviceWork, String> {
    if !statistics["lazy_execution"].is_null() {
        return Err("CPU route contradicts lazy device evidence".into());
    }
    let execution = &statistics["execution"];
    if !execution.is_null() {
        if !execution["adapter"].is_null() {
            return Err("CPU route reports a device adapter".into());
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
