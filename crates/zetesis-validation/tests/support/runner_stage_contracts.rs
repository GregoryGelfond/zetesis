//! Synthetic stage evidence cannot promote incomplete answers or device routes.

use serde_json::json;

use super::{NATIVE, PHASE_TIMINGS, check, emitting, loaded, options};
use crate::corpus_comparison::NativeOracle;
use zetesis_backend::{Backend, GpuApi};

const STAGES: &str = include_str!("stage_statistics.txt");

#[test]
fn stage_evidence_is_retained_independently_of_solver_completion() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.native_stats = true;
    let partial = STAGES
        .replace("complete=true", "complete=false")
        .replace("unattributed: elapsed_ns=160", "unattributed: unavailable");
    for (stdout, timing, exit, expected, available, complete, malformed) in [
        (NATIVE, STAGES.to_owned(), 0, "pass", 1, 1, 0),
        (
            NATIVE,
            "Stage timings: truncated\n".to_owned(),
            0,
            "pass",
            0,
            0,
            1,
        ),
        (NATIVE, String::new(), 0, "pass", 0, 0, 0),
        (NATIVE, partial.clone(), 0, "pass", 1, 0, 0),
        ("", partial, 2, "native_error", 1, 0, 0),
        (
            "INCOMPLETE: work\n",
            STAGES.to_owned(),
            3,
            "native_incomplete",
            1,
            1,
            0,
        ),
        (
            "",
            format!("source admission: budget\n{STAGES}"),
            2,
            "native_source_refused",
            1,
            1,
            0,
        ),
    ] {
        let stderr = format!("{timing}{PHASE_TIMINGS}");
        options.zetesis = emitting(directory.path(), "native", stdout, &stderr, exit);
        let (report, passed) = super::run(&options, &loaded);
        assert!(!passed, "one synthetic case is never the full corpus");
        assert_eq!(report["full_native_target_passed"], false);
        assert_eq!(report["full_physical_formula_route_passed"], false);
        let result = &report["cases"][0];
        assert_eq!(result["status"], expected, "{result:#}");
        assert_eq!(
            result["native_answer_parity_passed"] == true,
            expected == "pass"
        );
        assert_eq!(result["native_process"]["stderr"], stderr);
        assert_eq!(
            report["stage_timing_cases"],
            json!({"available": available, "complete": complete, "malformed": malformed})
        );
        assert_eq!(
            report["phase_timing_cases"],
            json!({"available": 1, "complete": 1, "malformed": 0})
        );
        if available == 1 {
            assert_eq!(result["native_stage_timings"]["driver_elapsed_ns"], 1000);
            assert_eq!(result["native_stage_timings"]["grounding_mode"], "eager");
            assert_eq!(
                result["native_stage_timings"]["stages"]["grounding"]["elapsed_ns"],
                200
            );
            assert_eq!(result["native_stage_timings"]["complete"], complete == 1);
            assert_eq!(
                result["native_stage_timings"]["unattributed_elapsed_ns"].is_null(),
                complete == 0
            );
        }
    }
}

#[test]
fn interleaved_grounding_is_not_reported_as_a_measured_zero() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    let timing = STAGES
        .replace("grounding_mode: eager", "grounding_mode: lazy_interleaved")
        .replace(
            "grounding: calls=1; elapsed_ns=200; complete=true",
            "grounding: unavailable=interleaved",
        )
        .replace(
            "unattributed: elapsed_ns=160",
            "unattributed: elapsed_ns=360",
        );
    options.zetesis = emitting(directory.path(), "native", NATIVE, &timing, 0);
    let result = check(&options, &loaded, "pass");
    assert_eq!(result["native_stage_timings"]["complete"], true);
    assert_eq!(
        result["native_stage_timings"]["grounding_mode"],
        "lazy_interleaved"
    );
    assert!(result["native_stage_timings"]["stages"]["grounding"].is_null());
    assert_eq!(
        result["native_stage_timings"]["stages"]["solving"]["elapsed_ns"],
        500
    );
}

#[test]
fn complete_stage_timing_never_qualifies_a_physical_device_route() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.native_backend = Backend::Gpu(Some(GpuApi::Metal));
    options.native_oracle = NativeOracle::Countermodel;
    options.zetesis = emitting(directory.path(), "native", NATIVE, STAGES, 0);
    let result = check(&options, &loaded, "native_execution_unqualified");
    assert_eq!(result["native_answer_parity_passed"], true);
    assert_eq!(result["native_stage_timings"]["complete"], true);
    assert!(result.get("native_formula_execution").is_none());
}

#[test]
fn capture_truncation_remains_a_resource_failure_with_malformed_timing() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.max_output_bytes = 300;
    options.zetesis = emitting(directory.path(), "native", "", STAGES, 0);
    let result = check(&options, &loaded, "native_output_limit");
    assert_eq!(result["native_process"]["status"], "output_limit");
    assert!(result["native_stage_timing_error"].is_string());
    assert!(result.get("native_stage_timings").is_none());
    assert!(result.get("native_answer_parity_passed").is_none());
}
