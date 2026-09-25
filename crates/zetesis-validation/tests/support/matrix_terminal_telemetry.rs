//! Base work needs full reconstruction evidence before original-family qualification.

use super::*;

fn terminal() -> (Value, String, NativeExecution) {
    let (mut document, text) = prepared_fixture();
    document["statistics"]["phase_timings"]["schema"] = json!(4);
    document["statistics"]["phase_timings"]["measurements"]["answer_reconstruction"] =
        json!({"calls":4,"elapsed_ns":12,"complete":true});
    document["statistics"]["stage_timings"]["grounding_mode"] =
        json!("eager_base_terminal_definitions");
    document["statistics"]["search"] = json!({"scope":"terminal_base","stable_models":4});
    document["statistics"]["terminal_execution"] = json!({"base_answers":4,"reconstructed":4,
        "pending":0,"reconstruction":{"attempts":4,"completed":4,"work":100,"substitutions":7}});
    document["outcome"] = json!({"verified_models":4});
    let text = text
        .replace("schema=3", "schema=4")
        .replace(
            "  phase scope:",
            "  phase answer_reconstruction: calls=4; elapsed_ns=12; complete=true\n  phase scope:",
        )
        .replace(
            "grounding_mode: eager",
            "grounding_mode: eager_base_terminal_definitions",
        )
        .replace(
            "oracle=closure; grounder=eager",
            "oracle=countermodel; grounder=eager_base_terminal_definitions",
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
fn composite_route_retains_original_completion_and_base_scope() {
    let (document, text, request) = terminal();
    let observation = observe(&document, text.as_bytes(), request).unwrap();
    let receipt = observation.terminal.unwrap();
    assert_eq!(
        (receipt.base_answers, receipt.reconstructed, receipt.pending),
        (4, 4, 0)
    );
    assert_eq!((receipt.work, receipt.substitutions), (100, 7));
    assert_eq!(observation.timing.phase_schema, 4);
    assert_eq!(
        observation.timing.grounding_mode,
        "eager_base_terminal_definitions"
    );
    assert!(observation.hybrid.is_none());
    for grounder in [Grounder::Eager, Grounder::Lazy] {
        assert!(
            observe(
                &document,
                text.as_bytes(),
                NativeExecution {
                    grounder,
                    ..request
                }
            )
            .is_err()
        );
    }
}

#[test]
fn incomplete_or_misidentified_reconstruction_cannot_qualify() {
    let (document, text, request) = terminal();
    let mutations = [
        ("/statistics/terminal_execution/pending", json!(1)),
        ("/statistics/terminal_execution/reconstructed", json!(3)),
        ("/statistics/terminal_execution/base_answers", json!(5)),
        (
            "/statistics/terminal_execution/reconstruction/completed",
            json!(3),
        ),
        (
            "/statistics/terminal_execution/reconstruction/attempts",
            json!(5),
        ),
        (
            "/statistics/phase_timings/measurements/answer_reconstruction/calls",
            json!(3),
        ),
        ("/statistics/search/scope", json!("original_theory")),
        ("/statistics/search/stable_models", json!(5)),
        ("/outcome/verified_models", json!(3)),
        ("/statistics/terminal_execution", Value::Null),
    ];
    for (path, value) in mutations {
        let mut changed = document.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(
            observe(&changed, text.as_bytes(), request).is_err(),
            "{path}"
        );
    }
    assert!(
        observe(
            &document,
            text.replace("grounder=eager_base_terminal_definitions", "grounder=eager")
                .as_bytes(),
            request
        )
        .is_err()
    );
}
