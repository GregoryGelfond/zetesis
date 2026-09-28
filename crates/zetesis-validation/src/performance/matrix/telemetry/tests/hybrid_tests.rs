//! Mixed source grounding requires original-program acceptance evidence.

use super::*;

fn mixed() -> (Value, String, NativeExecution) {
    let (mut document, text) = fixture();
    document["statistics"]["stage_timings"]["grounding_mode"] = json!("mixed");
    document["statistics"]["search"] = json!({"scope":"retained_core","stable_models":64});
    document["statistics"]["hybrid_execution"] = json!({"core_answers":64,"accepted":7,
        "rejected":57,"pending":0,"constraints":{"work":100,"substitutions":90,"scalar_bytes":80}});
    document["outcome"] = json!({"verified_models":7});
    let text = text
        .replace("grounding_mode: eager", "grounding_mode: mixed")
        .replace(
            "oracle=closure; grounder=eager",
            "oracle=countermodel; grounder=hybrid",
        );
    (
        document,
        text,
        NativeExecution {
            grounder: Grounder::Lazy,
            ..Default::default()
        },
    )
}

#[test]
fn mixed_grounding_keeps_core_and_original_counts_distinct() {
    let (document, text, request) = mixed();
    let observation = observe(&document, text.as_bytes(), request).unwrap();
    let receipt = observation.hybrid.unwrap();
    assert_eq!(receipt.core_answers, 64);
    assert_eq!(receipt.accepted, 7);
    assert_eq!(receipt.rejected, 57);
    assert_eq!(receipt.work, 100);
    assert_eq!(receipt.substitutions, 90);
    assert_eq!(receipt.scalar_bytes, 80);
    assert_eq!(observation.timing.grounding_mode, "mixed");
    assert_eq!(
        observation.timing.stages["grounding"]
            .as_ref()
            .unwrap()
            .elapsed_ns,
        200
    );
    assert_eq!(
        observation.timing.stages["solving"]
            .as_ref()
            .unwrap()
            .elapsed_ns,
        500
    );
    assert!(matches!(observation.execution.device, DeviceWork::Cpu));
}

#[test]
fn early_region_counts_remain_separate_from_core_answers() {
    let (mut document, text, request) = mixed();
    document["statistics"]["search"]["region_filter"] = json!({
        "preparations":4,"checks":120,"refuted":43,"failed":0,"overflowed":false
    });
    let observation = observe(&document, text.as_bytes(), request).unwrap();
    let hybrid = observation.hybrid.unwrap();
    assert_eq!(hybrid.core_answers, 64);
    let regions = hybrid.regions.unwrap();
    assert_eq!(regions.preparations, 4);
    assert_eq!(regions.checks, 120);
    assert_eq!(regions.refuted, 43);
}

#[test]
fn failed_region_check_cannot_qualify_completion() {
    let (mut document, text, request) = mixed();
    document["statistics"]["search"]["region_filter"] = json!({
        "preparations":1,"checks":2,"refuted":1,"failed":1,"overflowed":false
    });
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn overflowed_region_counts_cannot_qualify_completion() {
    let (mut document, text, request) = mixed();
    document["statistics"]["search"]["region_filter"] = json!({
        "preparations":1,"checks":2,"refuted":1,"failed":0,"overflowed":true
    });
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn impossible_region_counts_cannot_qualify_completion() {
    let (mut document, text, request) = mixed();
    for receipt in [
        json!({"preparations":1,"checks":2,"refuted":3,"failed":0,"overflowed":false}),
        json!({"preparations":0,"checks":2,"refuted":1,"failed":0,"overflowed":false}),
    ] {
        document["statistics"]["search"]["region_filter"] = receipt;
        assert!(observe(&document, text.as_bytes(), request).is_err());
    }
}

#[test]
fn hybrid_core_certificates_do_not_claim_eager_source_execution() {
    let (document, text, request) = mixed();
    for (name, procedure) in [
        ("tight-support", Procedure::TightSupport),
        ("positive-consequences", Procedure::PositiveConsequences),
    ] {
        let text = text.replace("oracle=countermodel", &format!("oracle={name}"));
        let observed = observe(&document, text.as_bytes(), request).unwrap();
        assert_eq!(observed.execution.procedure, procedure);
        assert!(observed.hybrid.is_some());
    }
}

#[test]
fn mixed_grounding_requires_a_source_checking_receipt() {
    let (mut document, text, request) = mixed();
    document["statistics"]["hybrid_execution"] = Value::Null;
    assert!(observe(&document, text.as_bytes(), request).is_err());
    document["statistics"]["search"]["scope"] = json!("original_theory");
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn hybrid_acceptance_must_match_original_verification() {
    let (mut document, text, request) = mixed();
    document["outcome"]["verified_models"] = json!(64);
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn hybrid_core_population_must_match_the_search_receipt() {
    let (document, text, request) = mixed();
    for search in [
        json!({"scope":"original_theory","stable_models":64}),
        json!({"scope":"retained_core","stable_models":7}),
    ] {
        let mut document = document.clone();
        document["statistics"]["search"] = search;
        assert!(observe(&document, text.as_bytes(), request).is_err());
    }
}

#[test]
fn pending_constraint_checks_are_not_complete_observations() {
    let (mut document, text, request) = mixed();
    document["statistics"]["hybrid_execution"]["rejected"] = json!(56);
    document["statistics"]["hybrid_execution"]["pending"] = json!(1);
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn hybrid_population_sums_cannot_wrap() {
    let (mut document, text, request) = mixed();
    document["statistics"]["hybrid_execution"]["accepted"] = json!(u64::MAX);
    document["statistics"]["hybrid_execution"]["rejected"] = json!(65);
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn hybrid_receipts_require_the_effective_hybrid_label() {
    let (document, text, request) = mixed();
    assert!(
        observe(
            &document,
            text.replace("grounder=hybrid", "grounder=eager").as_bytes(),
            request
        )
        .is_err()
    );
    assert!(observe(&document, text.as_bytes(), NativeExecution::default()).is_err());
}

#[test]
fn ordinary_cpu_observations_have_no_hybrid_receipt() {
    let (document, text) = fixture();
    assert!(
        observe(&document, text.as_bytes(), NativeExecution::default())
            .unwrap()
            .hybrid
            .is_none()
    );
}
