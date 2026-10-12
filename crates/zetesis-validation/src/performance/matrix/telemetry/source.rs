//! Exact source evaluation has admission work, not a fabricated formula oracle.

use super::{field, number, one};
use crate::performance::Diagnostics;
use crate::performance::matrix::{
    DeviceWork, Execution, Observation, Procedure, SourceEvaluationStatistics,
};
use crate::selected::{Backend, Grounder, NativeExecution, Oracle};
use serde_json::Value;

const ROUTE: &str =
    "cpu (dependency-ordered source evaluation; constraints checked before publication)";
const OPERATION: &str = "direct source evaluation";

pub(super) fn read(
    statistics: &Value,
    stderr: &[u8],
    request: NativeExecution,
    timing: &Diagnostics,
) -> Result<Option<SourceEvaluationStatistics>, String> {
    let text = std::str::from_utf8(stderr).map_err(|error| error.to_string())?;
    let effective = one(
        text.lines()
            .filter_map(|line| line.trim_start().strip_prefix("effective execution: ")),
        "effective execution",
    )?;
    let receipt = &statistics["source_evaluation"];
    if receipt.is_null() {
        // Older records have no field or a null one. An operation-labelled
        // route is new evidence and cannot silently acquire an old oracle.
        if has_field(effective, "operation")
            || text
                .lines()
                .any(|line| line.strip_prefix("Backend: ") == Some(ROUTE))
        {
            return Err("source operation lacks its typed admission receipt".into());
        }
        return Ok(None);
    }
    let parsed = receipt_statistics(receipt)?;
    let route = one(
        text.lines()
            .filter_map(|line| line.strip_prefix("Backend: ")),
        "actual backend metadata",
    )?;
    if request.backend != Backend::Cpu
        || request.grounder != Grounder::Auto
        || request.oracle != Oracle::Auto
        || route != ROUTE
        || field(effective, "backend")? != "cpu"
        || field(effective, "operation")? != OPERATION
        || effective.split("; ").count() != 2
    {
        return Err("source evaluation requires its automatic CPU operation metadata".into());
    }
    activity(statistics, timing)?;
    let reported = one(
        text.lines()
            .filter_map(|line| line.trim_start().strip_prefix("source evaluation: ")),
        "source evaluation work",
    )?;
    if field(reported, "scope")? != "source_admission"
        || field(reported, "work")?.parse::<u64>().ok() != Some(parsed.work)
        || field(reported, "substitutions")?.parse::<u64>().ok() != Some(parsed.substitutions)
    {
        return Err("typed and text source admission receipts disagree".into());
    }
    Ok(Some(parsed))
}

fn receipt_statistics(value: &Value) -> Result<SourceEvaluationStatistics, String> {
    if value.as_object().map(serde_json::Map::len) != Some(3)
        || value["scope"] != "source_admission"
    {
        return Err("unknown source evaluation receipt shape or scope".into());
    }
    Ok(SourceEvaluationStatistics {
        work: number(value, "work")?,
        substitutions: number(value, "substitutions")?,
    })
}

fn activity(statistics: &Value, timing: &Diagnostics) -> Result<(), String> {
    for field in [
        "execution",
        "lazy_execution",
        "shared_execution",
        "closure_execution",
        "query_execution",
        "hybrid_execution",
        "terminal_execution",
        "search",
        "candidate_restrictions",
    ] {
        if statistics.get(field) != Some(&Value::Null) {
            return Err(format!(
                "source evaluation has conflicting or missing {field} evidence"
            ));
        }
    }
    if number(statistics, "objective_work")? != 0
        || timing.grounding_mode != "eager"
        || !timing.stages.get("grounding").is_some_and(|value| {
            value
                .as_ref()
                .is_some_and(|measurement| measurement.calls > 0)
        })
        || !timing
            .phases
            .get("admission_materialization")
            .is_some_and(|value| {
                value
                    .as_ref()
                    .is_some_and(|measurement| measurement.calls > 0)
            })
    {
        return Err("source evaluation lacks eager admission or reports objective work".into());
    }
    for (name, value) in &timing.phases {
        if value.is_some()
            && !matches!(
                name.as_str(),
                "admission_materialization"
                    | "execution_setup"
                    | "model_construction"
                    | "observation_output"
            )
        {
            return Err("source evaluation reports conflicting formula or search phases".into());
        }
    }
    Ok(())
}

fn has_field(line: &str, name: &str) -> bool {
    line.split("; ")
        .any(|field| field.split_once('=').is_some_and(|(key, _)| key == name))
}

pub(super) fn observation(receipt: SourceEvaluationStatistics, timing: Diagnostics) -> Observation {
    Observation {
        timing,
        execution: Execution {
            backend: Backend::Cpu,
            procedure: Procedure::AcyclicSource,
            adapter: None,
            device: DeviceWork::Cpu,
        },
        hybrid: None,
        terminal: None,
        source_evaluation: Some(receipt),
    }
}
