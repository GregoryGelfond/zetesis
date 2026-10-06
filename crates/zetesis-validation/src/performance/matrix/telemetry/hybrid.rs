//! A complete source-checking receipt authenticates the mixed grounding route.

use super::number;
use crate::performance::matrix::{HybridStatistics, RegionChecks};
use serde_json::Value;

pub(super) fn read(document: &Value) -> Result<Option<HybridStatistics>, String> {
    let statistics = &document["statistics"];
    let receipt = &statistics["hybrid_execution"];
    let scope = &statistics["search"]["scope"];
    if receipt.is_null() {
        if scope == "retained_core" || scope == "terminal_retained_core" {
            return Err("retained-core search lacks its source-checking receipt".into());
        }
        return Ok(None);
    }
    // Under a terminal base the accepted core answers are the base answers
    // reconstruction consumed; otherwise they are the published answers.
    let terminal = &statistics["terminal_execution"];
    let constraints = &receipt["constraints"];
    let parsed = HybridStatistics {
        regions: region_checks(&statistics["search"]["region_filter"])?,
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
        || number(&statistics["search"], "stable_models")? != parsed.core_answers
        || if terminal.is_null() {
            scope != "retained_core"
                || number(&document["outcome"], "verified_models")? != parsed.accepted
        } else {
            scope != "terminal_retained_core"
                || number(terminal, "base_answers")? != parsed.accepted
        }
    {
        return Err(
            "hybrid core/checker counts contradict complete original-program acceptance".into(),
        );
    }
    Ok(Some(parsed))
}

fn region_checks(receipt: &Value) -> Result<Option<RegionChecks>, String> {
    if receipt.is_null() {
        return Ok(None);
    }
    let parsed = RegionChecks {
        preparations: number(receipt, "preparations")?,
        checks: number(receipt, "checks")?,
        refuted: number(receipt, "refuted")?,
    };
    if receipt["overflowed"].as_bool() != Some(false)
        || number(receipt, "failed")? != 0
        || parsed.refuted > parsed.checks
        || (parsed.checks != 0 && parsed.preparations == 0)
    {
        return Err("region restriction receipt does not establish completed checking".into());
    }
    Ok(Some(parsed))
}
