//! Independent paired typed/text telemetry fixture.
use serde_json::{Value, json};

pub(crate) fn fixture() -> (Value, String) {
    let measurement =
        |calls, elapsed_ns| json!({"calls":calls,"elapsed_ns":elapsed_ns,"complete":true});
    let mut phases = json!({});
    for name in [
        "execution_setup",
        "candidate_setup",
        "candidate_generation",
        "original_validation",
        "gpu_host_oracle",
        "exact_reduct_membership",
        "closure_membership",
        "objective_scoring_retention",
        "objective_feedback",
        "observation_output",
    ] {
        phases[name] = Value::Null;
    }
    phases["admission_materialization"] = measurement(1, 40);
    let document = json!({"statistics": {
        "stage_timings":{"grounding_mode":"eager","driver_elapsed_ns":1000,"complete":true,"unattributed_elapsed_ns":160,
            "measurements":{"source_preparation":measurement(1,100),"grounding":measurement(1,200),
                "solving":measurement(3,500),"observation_output":measurement(2,40)}},
        "phase_timings":{"driver_elapsed_ns":1000,"measurements":phases},"execution":null,"lazy_execution":null
    }});
    let text = format!(
        "Backend: cpu (fixture)\n  effective execution: backend=cpu; oracle=closure; grounder=eager; workers=1\n{}{}",
        zetesis_test_support::fixtures::PHASE_STATISTICS,
        zetesis_test_support::fixtures::STAGE_STATISTICS
    );
    (document, text)
}
