//! A complete source-checking receipt authenticates the mixed grounding route.

use super::number;
use crate::performance::matrix::HybridStatistics;
use serde_json::Value;

pub(super) fn read(document: &Value) -> Result<Option<HybridStatistics>, String> {
    let statistics = &document["statistics"];
    let receipt = &statistics["hybrid_execution"];
    if receipt.is_null() {
        if statistics["search"]["scope"] == "retained_core" {
            return Err("retained-core search lacks its source-checking receipt".into());
        }
        return Ok(None);
    }
    let constraints = &receipt["constraints"];
    let parsed = HybridStatistics {
        core_answers: number(receipt, "core_answers")?,
        accepted: number(receipt, "accepted")?,
        rejected: number(receipt, "rejected")?,
        pending: number(receipt, "pending")?,
        work: number(constraints, "work")?,
        substitutions: number(constraints, "substitutions")?,
        scalar_bytes: number(constraints, "scalar_bytes")?,
    };
    if parsed.pending != 0
        || parsed.accepted.checked_add(parsed.rejected) != Some(parsed.core_answers)
        || statistics["search"]["scope"] != "retained_core"
        || number(&statistics["search"], "stable_models")? != parsed.core_answers
        || number(&document["outcome"], "verified_models")? != parsed.accepted
    {
        return Err(
            "hybrid core/checker counts contradict complete original-program acceptance".into(),
        );
    }
    Ok(Some(parsed))
}
