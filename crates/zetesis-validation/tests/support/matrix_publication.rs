//! Semantic exhaustion does not establish that the complete family was delivered.

use super::*;

fn stopped() -> Value {
    json!({"schema":2,"format":"zetesis","models":[{"retained":"opaque partial record"}],
        "statistics":null,"outcome":{"status":"incomplete","completion":"exhausted",
        "coverage":"exhausted","published_models":1,"verified_models":2,"checked":3,
        "interruption":null,"publication_stop":{"phase":"encoding","code":"cancelled"},
        "optimization":{"optimal":true},"error":null}})
}

#[test]
fn exhausted_search_with_stopped_delivery_never_becomes_complete_parity() {
    for phase in ["observation", "encoding", "record_preparation"] {
        for code in ["cancelled", "deadline"] {
            let mut document = stopped();
            document["outcome"]["publication_stop"] = json!({"phase":phase,"code":code});
            let before = document.clone();
            assert_eq!(
                outcome::check(&document, Some(exit(3))).unwrap_err().0,
                Decision::Incomplete
            );
            let mut observed = sample();
            let capture = observed.capture.as_mut().unwrap();
            capture.stdout = serde_json::to_vec(&document).unwrap();
            capture.exit = Some(exit(3));
            assert_eq!(
                qualify(&mut observed, None, Some(&reference()), &request())
                    .unwrap_err()
                    .0,
                Decision::Incomplete
            );
            assert!(observed.observation.is_none());
            assert!(observed.selected_models.is_none());
            assert_eq!(document, before);
        }
    }
}

#[test]
fn incomplete_census_keeps_both_pending_search_and_publication_causes() {
    let mut document = stopped();
    document["outcome"]["completion"] = Value::Null;
    document["outcome"]["coverage"] = json!("unavailable");
    document["outcome"]["optimization"]["optimal"] = json!(false);
    document["outcome"]["interruption"] = json!({
        "kind":"oracle", "code":"work_limit", "detail":"checked prefix awaits drain"
    });
    let (decision, detail) = outcome::check(&document, Some(exit(3))).unwrap_err();
    assert_eq!(decision, Decision::Incomplete);
    assert_eq!(
        detail,
        "native reported incomplete kind=oracle code=work_limit: checked prefix awaits drain; publication stopped phase=encoding code=cancelled; full evidence retained"
    );

    let (decision, detail) = outcome::check(&stopped(), Some(exit(3))).unwrap_err();
    assert_eq!(decision, Decision::Incomplete);
    assert_eq!(
        detail,
        "native reported incomplete; publication stopped phase=encoding code=cancelled; full evidence retained"
    );
}

#[test]
fn publication_before_final_search_classification_keeps_pending_evidence() {
    for interruption in [
        Value::Null,
        json!({"kind":"oracle","code":"work_limit","detail":"checked prefix awaits drain"}),
    ] {
        let mut document = stopped();
        document["outcome"]["completion"] = Value::Null;
        document["outcome"]["coverage"] = json!("unavailable");
        document["outcome"]["optimization"]["optimal"] = json!(false);
        document["outcome"]["interruption"] = interruption;
        assert_eq!(
            outcome::check(&document, Some(exit(3))).unwrap_err().0,
            Decision::Incomplete
        );
    }
}

#[test]
fn preparation_interruption_has_its_own_incomplete_census() {
    for code in ["cancelled", "deadline", "allocation"] {
        let mut document = interrupted();
        document["outcome"]["interruption"] =
            json!({"kind":"preparation","code":code,"detail":"before executor entry"});
        assert_eq!(
            outcome::check(&document, Some(exit(3))).unwrap_err().0,
            Decision::Incomplete
        );
    }
}

#[test]
fn missing_or_malformed_publication_stop_cannot_explain_exhausted_incomplete_status() {
    for stop in [
        Value::Null,
        json!({}),
        json!({"phase":"encoding"}),
        json!({"code":"cancelled"}),
        json!({"phase":"unknown","code":"cancelled"}),
        json!({"phase":"encoding","code":"allocation"}),
        json!({"phase":"encoding","code":"work_limit"}),
        json!({"phase":"encoding","code":3}),
    ] {
        let mut document = stopped();
        document["outcome"]["publication_stop"] = stop;
        assert_eq!(
            outcome::check(&document, Some(exit(3))).unwrap_err().0,
            Decision::InvalidReport
        );
    }
    let mut document = stopped();
    document["outcome"]
        .as_object_mut()
        .unwrap()
        .remove("publication_stop");
    assert_eq!(
        outcome::check(&document, Some(exit(3))).unwrap_err().0,
        Decision::InvalidReport
    );
}

#[test]
fn publication_stop_cannot_contradict_status_counts_or_semantic_evidence() {
    for (field, value) in [
        ("status", json!("satisfiable")),
        ("status", json!("unsatisfiable")),
        ("published_models", json!(2)),
        ("verified_models", Value::Null),
        ("coverage", json!("partial")),
        ("optimization", json!({"optimal":false})),
        (
            "interruption",
            json!({"kind":"oracle","code":"cancelled","detail":"conflict"}),
        ),
    ] {
        let mut document = stopped();
        document["outcome"][field] = value;
        assert_eq!(
            outcome::check(&document, Some(exit(3))).unwrap_err().0,
            Decision::InvalidReport,
            "{field}"
        );
    }
}

#[test]
fn publication_stop_does_not_hide_failed_exit() {
    assert_eq!(
        outcome::check(&stopped(), Some(exit(0))).unwrap_err().0,
        Decision::InvocationFailure
    );
}

#[test]
fn publication_stop_does_not_hide_writer_error() {
    let mut document = stopped();
    document["outcome"]["status"] = json!("failed");
    document["outcome"]["error"] = json!({"kind":"output","secondary_output_failure":true});
    assert_eq!(
        outcome::check(&document, Some(exit(2))).unwrap_err().0,
        Decision::InvocationFailure
    );
    assert_eq!(document["outcome"]["completion"], "exhausted");
    assert_eq!(document["outcome"]["optimization"]["optimal"], true);
}

#[test]
fn publication_stop_without_semantic_progress_is_not_a_producer_outcome() {
    let mut document = stopped();
    document["models"] = json!([]);
    let outcome = &mut document["outcome"];
    outcome["completion"] = Value::Null;
    outcome["coverage"] = json!("unavailable");
    outcome["published_models"] = json!(0);
    outcome["verified_models"] = Value::Null;
    outcome["checked"] = Value::Null;
    outcome["optimization"] = Value::Null;
    assert_eq!(
        outcome::check(&document, Some(exit(3))).unwrap_err().0,
        Decision::InvalidReport
    );
}
