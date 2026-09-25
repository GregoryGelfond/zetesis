//! Complete original-answer receipts for an eager base with terminal definitions.

use super::number;
use crate::performance::matrix::TerminalStatistics;
use serde_json::Value;

pub(super) fn execution(
    receipt: &TerminalStatistics,
    timing: &crate::performance::Diagnostics,
    procedure: super::Procedure,
) -> Result<(), String> {
    if timing.grounding_mode != "eager_base_terminal_definitions"
        || procedure == super::Procedure::Closure
    {
        return Err(
            "terminal definitions require eager base membership and host reconstruction".into(),
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
    if receipt.is_null() {
        if statistics["search"]["scope"] == "terminal_base" {
            return Err("terminal base search lacks its reconstruction receipt".into());
        }
        return Ok(None);
    }
    let reconstruction = &receipt["reconstruction"];
    let parsed = TerminalStatistics {
        base_answers: number(receipt, "base_answers")?,
        reconstructed: number(receipt, "reconstructed")?,
        pending: number(receipt, "pending")?,
        attempts: number(reconstruction, "attempts")?,
        completed: number(reconstruction, "completed")?,
        work: number(reconstruction, "work")?,
        substitutions: number(reconstruction, "substitutions")?,
    };
    if parsed.pending != 0
        || parsed.reconstructed.checked_add(parsed.pending) != Some(parsed.base_answers)
        || parsed.attempts != parsed.base_answers
        || parsed.completed != parsed.reconstructed
        || statistics["search"]["scope"] != "terminal_base"
        || number(&statistics["search"], "stable_models")? != parsed.base_answers
        || number(&document["outcome"], "verified_models")? != parsed.reconstructed
    {
        return Err(
            "terminal base/reconstruction counts contradict complete original membership".into(),
        );
    }
    Ok(Some(parsed))
}
