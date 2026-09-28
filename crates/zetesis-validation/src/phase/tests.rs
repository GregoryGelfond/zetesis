//! Timing evidence is optional and never substitutes for answer-set qualification.

use super::{
    CERTIFICATE_HEADER, CERTIFICATE_LABELS, FOOTER, HEADER, LABELS, RECONSTRUCTION_HEADER,
    REDUCT_HEADER, parse,
};

fn section() -> String {
    section_for(HEADER, &LABELS)
}

fn section_for(header: &str, labels: &[&str]) -> String {
    let mut lines = vec![
        header.to_owned(),
        "  phase driver: elapsed_ns=1000".to_owned(),
    ];
    lines.extend(
        labels
            .iter()
            .map(|label| format!("  phase {label}: unmeasured")),
    );
    lines.push(FOOTER.to_owned());
    lines.join("\n")
}

#[test]
fn absence_is_distinct_from_a_complete_measured_zero_duration() {
    assert!(parse("Backend: cpu\n").unwrap().is_none());
    let text = section().replace(
        "phase admission_materialization: unmeasured",
        "phase admission_materialization: calls=1; elapsed_ns=0; complete=true",
    );
    let timing = parse(&format!("other diagnostic\n{text}\ntrailing diagnostic"))
        .unwrap()
        .unwrap();
    assert_eq!(timing.driver_elapsed_ns, 1000);
    assert!(timing.complete);
    let admission = timing.phases["admission_materialization"].as_ref().unwrap();
    assert_eq!((admission.calls, admission.elapsed_ns), (1, 0));
    assert!(timing.phases["gpu_host_oracle"].is_none());
}

#[test]
fn partial_counter_accounting_is_retained_without_claiming_complete_measurement() {
    let text = section().replace(
        "phase observation_output: unmeasured",
        "phase observation_output: calls=3; elapsed_ns=25; complete=false",
    );
    let timing = parse(&text).unwrap().unwrap();
    assert!(!timing.complete);
    let output = timing.phases["observation_output"].as_ref().unwrap();
    assert_eq!((output.calls, output.elapsed_ns), (3, 25));
    assert!(!output.complete);
}

#[test]
fn malformed_duplicate_truncated_and_out_of_range_records_are_refused() {
    let original = section();
    for text in [
        original.replace(HEADER, "Phase timings: clock=device"),
        original.replace(FOOTER, ""),
        original.replace("  phase driver: elapsed_ns=1000", ""),
        original.replace("elapsed_ns=1000", "elapsed_ns=18446744073709551616"),
        original.replace("elapsed_ns=1000", "elapsed_ns=-1"),
        original.replace("phase candidate_setup:", "phase execution_setup:"),
        format!("{original}\n{original}"),
        format!("{original}\n  phase unknown: unmeasured"),
    ] {
        assert!(parse(&text).is_err(), "{text}");
    }
    for record in [
        "calls=0; elapsed_ns=0; complete=true",
        "calls=1; elapsed_ns=1; complete=maybe",
        "calls=1; elapsed_ns=1",
        "calls=1; elapsed_ns=; complete=true",
        "calls=1; elapsed_ns=18446744073709551616; complete=true",
        "calls=18446744073709551616; elapsed_ns=0; complete=true",
        "elapsed_ns=1; calls=1; complete=true",
        "calls=+1; elapsed_ns=1; complete=true",
    ] {
        let text = original.replace(
            "phase exact_reduct_membership: unmeasured",
            &format!("phase exact_reduct_membership: {record}"),
        );
        assert!(parse(&text).is_err(), "{record}");
    }
}

#[test]
fn certificate_schema_is_distinct_and_legacy_evidence_remains_readable() {
    let legacy = include_str!("../../tests/support/phase_statistics.txt");
    let old = parse(legacy).unwrap().unwrap();
    assert_eq!(old.schema_version, 1);
    assert!(!old.phases.contains_key("certificate_setup"));
    let current = section_for(CERTIFICATE_HEADER, &CERTIFICATE_LABELS).replace(
        "phase certified_membership: unmeasured",
        "phase certified_membership: calls=4; elapsed_ns=12; complete=true",
    );
    let new = parse(&current).unwrap().unwrap();
    assert_eq!(new.schema_version, 2);
    assert_eq!(
        new.phases["certified_membership"].as_ref().unwrap().calls,
        4
    );
    for malformed in [
        current.replace("; schema=2", ""),
        current.replace("; schema=2", "; schema=3"),
        current
            .lines()
            .filter(|line| {
                !line.contains("phase certificate_setup:")
                    && !line.contains("phase certified_membership:")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    ] {
        assert!(parse(&malformed).is_err());
    }
}

#[test]
fn reduct_preparation_requires_its_own_phase_schema() {
    let current = section_for(REDUCT_HEADER, &LABELS[..14]).replace(
        "phase reduct_preparation: unmeasured",
        "phase reduct_preparation: calls=1; elapsed_ns=12; complete=true",
    );
    let parsed = parse(&current).unwrap().unwrap();
    assert_eq!(parsed.schema_version, 3);
    assert_eq!(parsed.phases.len(), 14);
    let preparation = parsed.phases["reduct_preparation"].as_ref().unwrap();
    assert_eq!((preparation.calls, preparation.elapsed_ns), (1, 12));
    for malformed in [
        current.replace("; schema=3", ""),
        current.replace("; schema=3", "; schema=2"),
        current.replace("; schema=3", "; schema=4"),
        current.replace(
            "  phase reduct_preparation: calls=1; elapsed_ns=12; complete=true\n",
            "",
        ),
        current.replace("phase reduct_preparation:", "phase future_preparation:"),
    ] {
        assert!(parse(&malformed).is_err());
    }
}

#[test]
fn reconstruction_phase_requires_schema_four_and_retains_failed_attempts() {
    let current = section_for(RECONSTRUCTION_HEADER, &LABELS[..15]).replace(
        "phase answer_reconstruction: unmeasured",
        "phase answer_reconstruction: calls=3; elapsed_ns=12; complete=false",
    );
    let parsed = parse(&current).unwrap().unwrap();
    assert_eq!((parsed.schema_version, parsed.phases.len()), (4, 15));
    assert!(!parsed.complete);
    let value = parsed.phases["answer_reconstruction"].as_ref().unwrap();
    assert_eq!((value.calls, value.elapsed_ns), (3, 12));
    for header in [REDUCT_HEADER, CERTIFICATE_HEADER] {
        assert!(parse(&current.replace(RECONSTRUCTION_HEADER, header)).is_err());
    }
    assert!(
        parse(&current.replace(
            "  phase answer_reconstruction: calls=3; elapsed_ns=12; complete=false\n",
            "",
        ))
        .is_err()
    );
}

#[test]
fn model_construction_requires_schema_five() {
    let current = section().replace(
        "phase model_construction: unmeasured",
        "phase model_construction: calls=4; elapsed_ns=18; complete=true",
    );
    let parsed = parse(&current).unwrap().unwrap();
    assert_eq!((parsed.schema_version, parsed.phases.len()), (5, 16));
    let value = parsed.phases["model_construction"].as_ref().unwrap();
    assert_eq!((value.calls, value.elapsed_ns), (4, 18));
    for header in [RECONSTRUCTION_HEADER, REDUCT_HEADER, CERTIFICATE_HEADER] {
        assert!(parse(&current.replace(HEADER, header)).is_err());
    }
    assert!(
        parse(&current.replace(
            "  phase model_construction: calls=4; elapsed_ns=18; complete=true\n",
            ""
        ))
        .is_err()
    );
}
