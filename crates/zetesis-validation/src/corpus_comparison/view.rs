//! Schema-1 compatibility rendering, derived from typed outcomes.
use super::record::Observation;
use super::{CaseResult, Report};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

impl Report {
    /// Render the established schema-1 report without changing its acceptance result.
    /// Platform capture fields retain their documented platform-specific shape.
    ///
    /// # Errors
    /// Returns a serialization error if retained paths or evidence cannot be
    /// represented in JSON; the typed comparison outcome remains available.
    pub fn to_json(&self) -> Result<Value, serde_json::Error> {
        let passed = self.passed();
        let answer_parity = self.answer_parity_passed();
        let gpu_exercised = self.gpu_exercised();
        let outer_unsat = self.outer_unsat();
        let mut counts = BTreeMap::<&str, usize>::new();
        for case in &self.cases {
            *counts.entry(case.decision.label()).or_default() += 1;
        }
        Ok(json!({
            "schema_version": 1,
            "child_cleanup": self.cleanup,
            "cancelled": self.cancelled,
            "mode": if self.request.reference_only { "reference_only" } else { "native_full_target" },
            "native_oracle": self.request.native_oracle.label(),
            "native_invocation": self.invocation,
            "native_backend": self.request.native_backend.label(),
            "native_resource_policy": "ordinary",
            "native_threads": self.request.native_threads.get(),
            "native_memory_bytes": self.request.native_memory_bytes,
            "native_time_limit_seconds": self.request.native_time_limit_seconds,
            "native_stats": self.request.native_stats,
            "effective_native_stats": self.request.effective_native_stats(),
            "physical_formula_route_required": self.request.physical_formula(),
            "full_native_answer_parity_passed": !self.request.reference_only && answer_parity,
            "full_native_target_passed": !self.request.reference_only && passed,
            "full_physical_formula_route_passed": self.request.physical_formula() && passed,
            "physical_formula_status": self.physical_status(),
            "formula_execution_cases": { "gpu_exercised": gpu_exercised, "outer_unsat_without_membership": outer_unsat },
            "phase_timing_cases": timing_summary(self.cases.iter().map(|case| &case.evidence.phase), |value| value.complete),
            "stage_timing_cases": timing_summary(self.cases.iter().map(|case| &case.evidence.stage), |value| value.complete),
            "requested_mode_passed": passed,
            "revision": self.corpus.manifest.revision,
            "manifest_sha256": self.corpus.manifest_sha256,
            "manifest_reference_toolchain": self.corpus.manifest.reference_toolchain,
            "corpus_root": serde_json::to_value(&self.corpus.root)?,
            "corpus_view": self.corpus.view,
            "case_count": self.cases.len(),
            "required_case_count": self.required_cases(),
            "status_counts": counts,
            "limits": { "timeout_ms": self.request.timeout_ms, "combined_output_bytes": self.request.max_output_bytes },
            "comparison_scope": "Complete final-optimal displays with both symbol and model multiplicities preserved, raw final model counts, cost vectors, and supported original contracts. Exactly the first final-cost optN incumbent is removed and its exact displayed-symbol multiset must recur. Native reports require consistent per-model costs plus exhausted coverage and publication. Hidden atom identities are not reconstructed from displays. Reference-only success never establishes native solver support.",
            "cases": self.cases.iter().map(CaseResult::to_json).collect::<Result<Vec<_>, _>>()?,
        }))
    }
}
impl CaseResult {
    /// Render this case with the established optional evidence fields and status.
    ///
    /// # Errors
    /// Returns a serialization error if retained paths or evidence cannot be
    /// represented in JSON; the typed comparison outcome remains available.
    pub fn to_json(&self) -> Result<Value, serde_json::Error> {
        let mut value = json!({"path": self.evidence.source.path, "sha256": self.evidence.source.sha256,
            "status": self.decision.label()});
        if let Some(detail) = self.decision.detail() {
            value["detail"] = detail.into();
        }
        if let Some(original) = &self.evidence.source.original_sha256 {
            value["original_sha256"] = original.clone().into();
            value["example_contract"] = json!(self.evidence.source.example_contract);
        }
        optional(
            &mut value,
            "reference_process",
            self.evidence.reference_process.as_ref(),
        )?;
        optional(
            &mut value,
            "native_process",
            self.evidence.native_process.as_ref(),
        )?;
        optional(
            &mut value,
            "reference_answer",
            self.evidence.reference_answer.as_ref(),
        )?;
        optional(
            &mut value,
            "native_answer",
            self.evidence.native_answer.as_ref(),
        )?;
        optional(
            &mut value,
            "native_arguments",
            self.evidence.native_arguments.as_ref(),
        )?;
        optional(
            &mut value,
            "native_formula_execution",
            self.evidence.execution.as_ref(),
        )?;
        observation(
            &mut value,
            "native_phase_timings",
            "native_phase_timing_error",
            &self.evidence.phase,
        )?;
        observation(
            &mut value,
            "native_stage_timings",
            "native_stage_timing_error",
            &self.evidence.stage,
        )?;
        if self.evidence.answer_parity {
            value["native_answer_parity_passed"] = true.into();
        }
        Ok(value)
    }
}
fn optional<T: Serialize>(
    value: &mut Value,
    key: &str,
    field: Option<&T>,
) -> Result<(), serde_json::Error> {
    if let Some(field) = field {
        value[key] = serde_json::to_value(field)?;
    }
    Ok(())
}
fn observation<T: Serialize>(
    value: &mut Value,
    key: &str,
    error_key: &str,
    field: &Observation<T>,
) -> Result<(), serde_json::Error> {
    match field {
        Observation::Absent => {}
        Observation::Available(field) => value[key] = serde_json::to_value(field)?,
        Observation::Malformed(error) => value[error_key] = error.clone().into(),
    }
    Ok(())
}
fn timing_summary<'a, T: 'a>(
    values: impl Iterator<Item = &'a Observation<T>>,
    complete: impl Fn(&T) -> bool,
) -> Value {
    let (mut available, mut completed, mut malformed) = (0, 0, 0);
    for value in values {
        match value {
            Observation::Absent => {}
            Observation::Available(value) => {
                available += 1;
                completed += usize::from(complete(value));
            }
            Observation::Malformed(_) => malformed += 1,
        }
    }
    json!({"available": available, "complete": completed, "malformed": malformed})
}
