//! Source preparation receipts cannot be relabelled as formula oracle work.

use super::*;

fn source(work: u64, substitutions: u64) -> (Value, String, NativeExecution) {
    let (mut document, text) = prepared_fixture();
    let statistics = &mut document["statistics"];
    for name in [
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
        statistics[name] = Value::Null;
    }
    statistics["objective_work"] = json!(0);
    statistics["source_evaluation"] = json!({
        "scope":"source_admission", "work":work, "substitutions":substitutions,
    });
    statistics["phase_timings"]["schema"] = json!(5);
    statistics["phase_timings"]["measurements"]["answer_reconstruction"] = Value::Null;
    statistics["phase_timings"]["measurements"]["model_construction"] =
        json!({"calls":1,"elapsed_ns":12,"complete":true});
    let text = text
        .replace("schema=3", "schema=5")
        .replace(
            "  phase scope:",
            "  phase answer_reconstruction: unmeasured\n  phase model_construction: calls=1; elapsed_ns=12; complete=true\n  phase scope:",
        )
        .replace("Backend: cpu (fixture)",
            "Backend: cpu (dependency-ordered source evaluation; constraints checked before publication)")
        .replace("backend=cpu; oracle=closure; grounder=eager; workers=1",
            "backend=cpu; operation=direct source evaluation");
    let text = format!(
        "{text}  source evaluation: work={work}; substitutions={substitutions}; scope=source_admission; includes support, arithmetic and exact activity\n"
    );
    (
        document,
        text,
        NativeExecution {
            grounder: Grounder::Auto,
            ..Default::default()
        },
    )
}

#[test]
fn source_admission_retains_full_width_counters() {
    for (work, substitutions) in [(0, 0), (17, 23), (u64::MAX, u64::MAX - 1)] {
        let (document, text, request) = source(work, substitutions);
        let observed = observe(&document, text.as_bytes(), request).unwrap();
        assert_eq!(observed.execution.procedure, Procedure::AcyclicSource);
        assert_eq!(observed.execution.backend, Backend::Cpu);
        assert!(observed.execution.adapter.is_none());
        assert!(matches!(observed.execution.device, DeviceWork::Cpu));
        let receipt = observed.source_evaluation.unwrap();
        assert_eq!((receipt.work, receipt.substitutions), (work, substitutions));
        let encoded = serde_json::to_value(observed).unwrap();
        assert_eq!(encoded["execution"]["procedure"], "acyclic_source");
        assert_eq!(encoded["source_evaluation"]["work"], json!(work));
        assert_eq!(
            encoded["source_evaluation"]["substitutions"],
            json!(substitutions)
        );
        assert_eq!(encoded["timing"]["grounding_mode"], "eager");
    }
}

#[test]
fn missing_or_null_source_fields_keep_old_records_readable() {
    let (mut document, text) = prepared_fixture();
    for present in [false, true] {
        if present {
            document["statistics"]["source_evaluation"] = Value::Null;
        }
        let observed = observe(&document, text.as_bytes(), NativeExecution::default()).unwrap();
        assert_eq!(observed.execution.procedure, Procedure::Closure);
        assert!(observed.source_evaluation.is_none());
        assert!(
            serde_json::to_value(observed)
                .unwrap()
                .get("source_evaluation")
                .is_none()
        );
    }
}

#[test]
fn source_cannot_satisfy_an_explicit_execution_request() {
    let (document, text, request) = source(17, 23);
    for changed in [
        NativeExecution {
            grounder: Grounder::Eager,
            ..request
        },
        NativeExecution {
            grounder: Grounder::Lazy,
            ..request
        },
        NativeExecution {
            oracle: Oracle::Closure,
            ..request
        },
        NativeExecution {
            oracle: Oracle::Countermodel,
            ..request
        },
        NativeExecution {
            backend: Backend::Gpu(Some(GpuApi::Metal)),
            ..request
        },
    ] {
        assert!(observe(&document, text.as_bytes(), changed).is_err());
    }
}

#[test]
fn source_receipt_requires_its_known_shape_and_scope() {
    let (document, text, request) = source(17, 23);
    for value in [
        Value::Null,
        json!(false),
        json!([]),
        json!({}),
        json!({"scope":"search", "work":17, "substitutions":23}),
        json!({"scope":"source_admission", "work":-1, "substitutions":23}),
        json!({"scope":"source_admission", "work":17.5, "substitutions":23}),
        json!({"scope":"source_admission", "work":"17", "substitutions":23}),
        json!({"scope":"source_admission", "work":17, "substitutions":null}),
        json!({"scope":"source_admission", "work":17, "substitutions":23, "future":0}),
    ] {
        let mut changed = document.clone();
        changed["statistics"]["source_evaluation"] = value;
        assert!(observe(&changed, text.as_bytes(), request).is_err());
    }
    let mut missing = document;
    missing["statistics"]
        .as_object_mut()
        .unwrap()
        .remove("source_evaluation");
    assert!(observe(&missing, text.as_bytes(), request).is_err());
}

#[test]
fn source_requires_agreeing_unique_operation_metadata() {
    let (document, text, request) = source(17, 23);
    for changed in [
        text.replace("operation=direct source evaluation", "operation=unknown"),
        text.replace("operation=direct source evaluation", "oracle=closure; grounder=eager"),
        text.replace("operation=direct source evaluation", "operation=direct source evaluation; oracle=closure"),
        text.replace("Backend: cpu (dependency-ordered source evaluation; constraints checked before publication)", "Backend: cpu (fixture)"),
        text.replace("work=17;", "work=18;"),
        text.replace("substitutions=23;", "substitutions=24;"),
        text.replace("scope=source_admission", "scope=search"),
        format!("{text}  source evaluation: work=17; substitutions=23; scope=source_admission\n"),
        format!("{text}Backend: cpu (fixture)\n"),
    ] {
        assert!(observe(&document, changed.as_bytes(), request).is_err());
    }
}

#[test]
fn source_refuses_competing_activity_receipts() {
    let (document, text, request) = source(17, 23);
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
        let mut changed = document.clone();
        changed["statistics"][field] = json!({});
        assert!(
            observe(&changed, text.as_bytes(), request).is_err(),
            "{field}"
        );
    }
    let mut changed = document;
    changed["statistics"]["objective_work"] = json!(1);
    assert!(observe(&changed, text.as_bytes(), request).is_err());
}

#[test]
fn source_requires_eager_preparation_evidence() {
    let (mut document, text, request) = source(17, 23);
    document["statistics"]["stage_timings"]["grounding_mode"] = json!("mixed");
    let text = text.replace("grounding_mode: eager", "grounding_mode: mixed");
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn source_refuses_consistent_formula_phase_activity() {
    let (mut document, text, request) = source(17, 23);
    document["statistics"]["phase_timings"]["measurements"]["candidate_setup"] =
        json!({"calls":1,"elapsed_ns":0,"complete":true});
    let text = text.replace(
        "phase candidate_setup: unmeasured",
        "phase candidate_setup: calls=1; elapsed_ns=0; complete=true",
    );
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn an_unknown_operation_cannot_hide_in_an_old_oracle_record() {
    let (document, text) = prepared_fixture();
    let text = text.replace("oracle=closure;", "oracle=closure; operation=unknown;");
    assert!(observe(&document, text.as_bytes(), NativeExecution::default()).is_err());
}
