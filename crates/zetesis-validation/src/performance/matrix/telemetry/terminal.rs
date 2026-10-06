//! Complete original-answer receipts for a base, eager or hybrid, with
//! terminal definitions.

use super::number;
use crate::performance::matrix::TerminalStatistics;
use serde_json::Value;

pub(super) fn execution(
    receipt: &TerminalStatistics,
    timing: &crate::performance::Diagnostics,
    procedure: super::Procedure,
) -> Result<(), String> {
    let mode = if receipt.hybrid_base {
        "hybrid_base_terminal_definitions"
    } else {
        "eager_base_terminal_definitions"
    };
    if timing.grounding_mode != mode || procedure == super::Procedure::Closure {
        return Err(
            "terminal definitions require base membership of their kind and host reconstruction"
                .into(),
        );
    }
    let phase = timing
        .phases
        .get("answer_reconstruction")
        .ok_or("terminal definitions lack their reconstruction phase schema")?;
    let calls = phase.as_ref().map_or(0, |value| value.calls);
    if calls != receipt.attempts {
        return Err("reconstruction attempts disagree with the measured phase".into());
    }
    Ok(())
}

pub(super) fn read(document: &Value) -> Result<Option<TerminalStatistics>, String> {
    let statistics = &document["statistics"];
    let receipt = &statistics["terminal_execution"];
    let scope = &statistics["search"]["scope"];
    if receipt.is_null() {
        if scope == "terminal_base" || scope == "terminal_retained_core" {
            return Err("terminal base search lacks its reconstruction receipt".into());
        }
        return Ok(None);
    }
    // Receipts from before the base kind was reported are of an eager base,
    // the only kind those binaries had.
    let hybrid_base = match &receipt["base"] {
        Value::Null => false,
        Value::String(kind) if kind == "eager" => false,
        Value::String(kind) if kind == "hybrid" => true,
        _ => return Err("terminal receipt names an unknown base kind".into()),
    };
    let reconstruction = &receipt["reconstruction"];
    let parsed = TerminalStatistics {
        base_answers: number(receipt, "base_answers")?,
        reconstructed: number(receipt, "reconstructed")?,
        pending: number(receipt, "pending")?,
        attempts: number(reconstruction, "attempts")?,
        completed: number(reconstruction, "completed")?,
        work: number(reconstruction, "work")?,
        substitutions: number(reconstruction, "substitutions")?,
        hybrid_base,
    };
    // An eager base's search answers are its answers; a hybrid base's search
    // ran over its core, whose answers the hybrid receipt reconciles.
    let search = if hybrid_base {
        scope == "terminal_retained_core"
    } else {
        scope == "terminal_base"
            && number(&statistics["search"], "stable_models")? == parsed.base_answers
    };
    if parsed.pending != 0
        || parsed.reconstructed.checked_add(parsed.pending) != Some(parsed.base_answers)
        || parsed.attempts != parsed.base_answers
        || parsed.completed != parsed.reconstructed
        || !search
        || number(&document["outcome"], "verified_models")? != parsed.reconstructed
    {
        return Err(
            "terminal base/reconstruction counts contradict complete original membership".into(),
        );
    }
    Ok(Some(parsed))
}
