//! Per-case differential decisions and machine-readable evidence.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::Options;
use crate::corpus::{Case, Loaded};
use crate::execution;
use crate::normalize;
use crate::phase;
use crate::process::{self, Capture};

pub(crate) fn run(options: &Options, loaded: &Loaded) -> (Value, bool) {
    let mut cases = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for case in &loaded.manifest.cases {
        let result = check_case(options, loaded, case);
        let status = result["status"]
            .as_str()
            .expect("authored case result has a status");
        *counts.entry(status.to_owned()).or_default() += 1;
        eprintln!("{status}: {}", case.path);
        cases.push(result);
    }
    let expected = if options.reference_only {
        "reference_pass"
    } else {
        "pass"
    };
    let gpu_exercised = cases
        .iter()
        .filter(|case| case["native_formula_execution"]["status"] == "gpu_exercised")
        .count();
    let outer_unsat = cases
        .iter()
        .filter(|case| {
            case["native_formula_execution"]["status"] == "outer_unsat_without_membership"
        })
        .count();
    let answer_parity = cases.len() == 94
        && cases
            .iter()
            .all(|case| case["native_answer_parity_passed"] == true);
    let passed =
        counts.get(expected) == Some(&94) && (!options.physical_formula() || gpu_exercised > 0);
    let physical_status = if !options.physical_formula() {
        "not_requested"
    } else if counts.get("pass") == Some(&94) && gpu_exercised == 0 {
        "not_exercised"
    } else if passed {
        "qualified"
    } else {
        "unqualified"
    };
    let timing_available = cases
        .iter()
        .filter(|case| case["native_phase_timings"].is_object())
        .count();
    let timing_complete = cases
        .iter()
        .filter(|case| case["native_phase_timings"]["complete"] == true)
        .count();
    let timing_malformed = cases
        .iter()
        .filter(|case| case["native_phase_timing_error"].is_string())
        .count();
    (
        json!({
            "schema_version": 1,
            "mode": if options.reference_only { "reference_only" } else { "native_full_target" },
            "native_oracle": options.native_oracle.label(),
            "native_backend": options.native_backend.label(),
            "native_batch_size": options.native_batch_size.get(),
            "native_completion_workers": options.native_completion_workers.get(),
            "native_max_completion_scratch_bytes": options.native_max_completion_scratch_bytes,
            "native_stats": options.native_stats,
            "effective_native_stats": options.effective_native_stats(),
            "physical_formula_route_required": options.physical_formula(),
            "full_native_answer_parity_passed": !options.reference_only && answer_parity,
            "full_native_target_passed": !options.reference_only && passed,
            "full_physical_formula_route_passed": options.physical_formula() && passed,
            "physical_formula_status": physical_status,
            "formula_execution_cases": { "gpu_exercised": gpu_exercised, "outer_unsat_without_membership": outer_unsat },
            "phase_timing_cases": { "available": timing_available, "complete": timing_complete, "malformed": timing_malformed },
            "requested_mode_passed": passed,
            "revision": loaded.manifest.revision,
            "manifest_sha256": loaded.manifest_sha256,
            "manifest_reference_toolchain": loaded.manifest.reference_toolchain,
            "corpus_root": loaded.root,
            "case_count": cases.len(),
            "status_counts": counts,
            "limits": { "timeout_ms": options.timeout_ms, "combined_output_bytes": options.max_output_bytes },
            "comparison_scope": "Complete final-optimal displays with both symbol and model multiplicities preserved, raw final model counts, cost vectors, and supported original contracts. Exactly the first final-cost optN incumbent is removed and its exact displayed-symbol multiset must recur. Native optimized SAT requires per-model Optimization: vectors plus exhausted coverage. Hidden atom identities are not reconstructed from display. Reference-only success never establishes native solver support.",
            "cases": cases,
        }),
        passed,
    )
}

fn check_case(options: &Options, loaded: &Loaded, case: &Case) -> Value {
    let input = loaded.root.join(&case.path);
    let reference_args = vec![
        "--models=0".into(),
        "--outf=2".into(),
        "--opt-mode=optN".into(),
        input.as_os_str().to_owned(),
    ];
    let mut result = json!({ "path": case.path, "sha256": case.sha256 });
    let reference = match invoke(options, &options.clingo, &reference_args, &loaded.root) {
        Ok(capture) => capture,
        Err(error) => return failure(result, "reference_invocation_error", error),
    };
    result["reference_process"] = json!(reference);
    if reference.status != "completed" {
        return failure(
            result,
            &format!("reference_{}", reference.status),
            "reference invocation did not complete",
        );
    }
    if !matches!(reference.exit_code, Some(10 | 20 | 30)) {
        return failure(
            result,
            "reference_error",
            "reference returned an unexpected exit code",
        );
    }
    let answer = match normalize::reference(&reference.stdout) {
        Ok(answer) => answer,
        Err(error) => return failure(result, "reference_output_error", error),
    };
    result["reference_answer"] = json!(answer);
    if let Err(error) = normalize::contracts(case, &answer) {
        return failure(result, "reference_contract_mismatch", error);
    }
    if options.reference_only {
        result["status"] = "reference_pass".into();
        return result;
    }
    check_native(options, loaded, case, &answer, result)
}

fn check_native(
    options: &Options,
    loaded: &Loaded,
    case: &Case,
    answer: &normalize::Answer,
    mut result: Value,
) -> Value {
    let input = loaded.root.join(&case.path);
    let mut native_args = vec![
        "--backend".into(),
        options.native_backend.label().into(),
        "--oracle".into(),
        options.native_oracle.label().into(),
        "--models".into(),
        "0".into(),
        "--batch-size".into(),
        options.native_batch_size.to_string().into(),
        "--completion-workers".into(),
        options.native_completion_workers.to_string().into(),
        "--max-completion-scratch-bytes".into(),
        options
            .native_max_completion_scratch_bytes
            .to_string()
            .into(),
    ];
    if options.effective_native_stats() {
        native_args.push("--stats".into());
    }
    native_args.push(input.as_os_str().to_owned());
    result["native_arguments"] = json!(native_args);
    let native = match invoke(options, &options.zetesis, &native_args, &loaded.root) {
        Ok(capture) => capture,
        Err(error) => return failure(result, "native_invocation_error", error),
    };
    result["native_process"] = json!(native);
    match phase::parse(&native.stderr) {
        Ok(Some(timing)) => result["native_phase_timings"] = json!(timing),
        Ok(None) => {}
        Err(error) => result["native_phase_timing_error"] = error.into(),
    }
    if native.status != "completed" {
        return failure(
            result,
            &format!("native_{}", native.status),
            "native invocation did not complete",
        );
    }
    if native.exit_code == Some(3) || native.stdout.contains("INCOMPLETE:") {
        return failure(
            result,
            "native_incomplete",
            "native solver exhausted a budget or was interrupted",
        );
    }
    if native.exit_code != Some(0) {
        let status = if native.exit_code == Some(2)
            && (native.stderr.contains("source admission:")
                || native.stderr.contains("S0")
                || native.stderr.contains("source expansion"))
        {
            "native_source_refused"
        } else {
            "native_error"
        };
        return failure(
            result,
            status,
            "native solver did not return a completed result",
        );
    }
    let native_answer = match normalize::native(&native.stdout, answer.cost.is_some()) {
        Ok(answer) => answer,
        Err(error) => return failure(result, "native_output_unsupported", error),
    };
    result["native_answer"] = json!(native_answer);
    if !normalize::same(answer, &native_answer) {
        return failure(
            result,
            "mismatch",
            "native and reference completed results differ",
        );
    }
    if let Err(error) = normalize::contracts(case, &native_answer) {
        return failure(result, "native_contract_mismatch", error);
    }
    result["native_answer_parity_passed"] = true.into();
    if options.physical_formula() {
        match execution::formula_for_request(
            &native.stderr,
            options.native_backend,
            options.native_batch_size.get(),
            execution::CompletionRequest {
                workers: options.native_completion_workers,
                max_scratch_bytes: options.native_max_completion_scratch_bytes,
            },
            &native_answer,
        ) {
            Ok(evidence) => result["native_formula_execution"] = json!(evidence),
            Err(error) => return failure(result, "native_execution_unqualified", error),
        }
    }
    result["status"] = "pass".into();
    result
}

fn invoke(
    options: &Options,
    executable: &Path,
    arguments: &[OsString],
    directory: &Path,
) -> Result<Capture, String> {
    process::invoke(
        executable,
        arguments,
        directory,
        Duration::from_millis(options.timeout_ms),
        options.max_output_bytes,
    )
}

fn failure(mut result: Value, status: &str, detail: impl Into<String>) -> Value {
    result["status"] = status.into();
    result["detail"] = detail.into().into();
    result
}

#[cfg(all(test, unix))]
#[path = "../tests/support/runner_contracts.rs"]
mod tests;
