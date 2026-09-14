//! Stage protocol integrity never substitutes for completed solver evidence.

use super::{FOOTER, HEADER, parse};

const SECTION: &str = include_str!("stage_statistics.txt");
const GROUNDING: &str = "grounding: calls=1; elapsed_ns=200; complete=true";

#[test]
fn stage_absence_preserves_supported_phase_schemas() {
    let legacy = include_str!("phase_statistics.txt");
    assert!(parse(legacy).unwrap().is_none());
    let current = legacy
        .replace("failed_attempts=included", "failed_attempts=included; schema=2")
        .replace(
            "  phase candidate_generation:",
            "  phase certificate_setup: unmeasured\n  phase certified_membership: unmeasured\n  phase candidate_generation:",
        );
    let prepared = current.replace("schema=2", "schema=3").replace(
        "  phase gpu_host_oracle:",
        "  phase reduct_preparation: unmeasured\n  phase gpu_host_oracle:",
    );
    for phase in [legacy, current.as_str(), prepared.as_str()] {
        let combined = format!("Timing summary: 0.001 ms\n{SECTION}{phase}\nother diagnostic\n");
        let stages = parse(&combined).unwrap().unwrap();
        assert!(stages.complete);
        assert_eq!(stages.schema_version, 1);
        assert_eq!(stages.driver_elapsed_ns, 1000);
        assert_eq!(stages.unattributed_elapsed_ns, Some(160));
        assert_eq!(stages.grounding_mode, "eager");
        assert_eq!(
            super::super::phase::parse(&combined)
                .unwrap()
                .unwrap()
                .driver_elapsed_ns,
            1000
        );
    }
}

#[test]
fn zero_duration_attempts_and_unentered_stages_are_distinct() {
    let zero = SECTION
        .replace("grounding_mode: eager", "grounding_mode: unentered")
        .replace(GROUNDING, "grounding: unmeasured")
        .replace(
            "solving: calls=3; elapsed_ns=500; complete=true",
            "solving: unmeasured",
        )
        .replace(
            "source_preparation: calls=1; elapsed_ns=100",
            "source_preparation: calls=1; elapsed_ns=0",
        )
        .replace(
            "unattributed: elapsed_ns=160",
            "unattributed: elapsed_ns=960",
        );
    let timing = parse(&zero).unwrap().unwrap();
    assert!(timing.complete);
    assert_eq!(timing.grounding_mode, "unentered");
    assert!(timing.stages["grounding"].is_none());
    assert!(timing.stages["solving"].is_none());
    let preparation = timing.stages["source_preparation"].as_ref().unwrap();
    assert_eq!((preparation.calls, preparation.elapsed_ns), (1, 0));
}

#[test]
fn lazy_and_mixed_grounding_preserve_the_unavailable_remainder() {
    let lazy = SECTION
        .replace("grounding_mode: eager", "grounding_mode: lazy_interleaved")
        .replace(GROUNDING, "grounding: unavailable=interleaved")
        .replace(
            "unattributed: elapsed_ns=160",
            "unattributed: elapsed_ns=360",
        );
    let timing = parse(&lazy).unwrap().unwrap();
    assert!(
        timing.complete,
        "separate grounding is unavailable, not corrupt"
    );
    assert_eq!(timing.grounding_mode, "lazy_interleaved");
    assert!(timing.stages["grounding"].is_none());
    assert_eq!(timing.stages["solving"].as_ref().unwrap().elapsed_ns, 500);
    let mixed = SECTION.replace("grounding_mode: eager", "grounding_mode: mixed");
    let timing = parse(&mixed).unwrap().unwrap();
    assert_eq!(timing.grounding_mode, "mixed");
    assert_eq!(timing.stages["grounding"].as_ref().unwrap().elapsed_ns, 200);
    let unmeasured = mixed.replace(GROUNDING, "grounding: unmeasured").replace(
        "unattributed: elapsed_ns=160",
        "unattributed: elapsed_ns=360",
    );
    assert!(parse(&unmeasured).unwrap().unwrap().stages["grounding"].is_none());
}

#[test]
fn partial_integrity_and_unavailable_partition_are_retained_as_incomplete() {
    let unavailable = SECTION.replace("unattributed: elapsed_ns=160", "unattributed: unavailable");
    for text in [
        unavailable.clone(),
        unavailable.replace("complete=true", "complete=false"),
        unavailable.replace("elapsed_ns=500", "elapsed_ns=18446744073709551615"),
        unavailable.replace(
            "stage driver: elapsed_ns=1000",
            "stage driver: elapsed_ns=1",
        ),
    ] {
        let timing = parse(&text).unwrap().unwrap();
        assert!(!timing.complete);
        assert!(timing.unattributed_elapsed_ns.is_none());
        assert_eq!(
            timing.stages["source_preparation"].as_ref().unwrap().calls,
            1
        );
    }
}

#[test]
fn malformed_duplicate_unknown_reordered_and_truncated_sections_are_rejected() {
    for text in [
        SECTION.replace(
            HEADER,
            "Stage timings: clock=device; scope=driver; schema=1",
        ),
        SECTION.replace("schema=1", "schema=2"),
        SECTION.replace(FOOTER, ""),
        SECTION.replace("  stage driver: elapsed_ns=1000\n", ""),
        SECTION.replace("grounding_mode: eager", "grounding_mode: unknown"),
        SECTION.replace("stage solving:", "stage source_preparation:"),
        SECTION.replace("stage grounding:", "stage unknown:"),
        SECTION.replace("source_loading=excluded", "source_loading=included"),
        SECTION.replace(
            "timer_overhead=not_separated",
            "timer_overhead=unattributed",
        ),
        format!("{SECTION}{SECTION}"),
        format!("{SECTION}  stage unknown: unmeasured\n"),
        format!("  stage solving: unmeasured\n{SECTION}"),
    ] {
        assert!(parse(&text).is_err(), "{text}");
    }
    let lines: Vec<_> = SECTION.lines().collect();
    for count in 1..lines.len() {
        assert!(parse(&lines[..count].join("\n")).is_err(), "{count} lines");
    }
    assert!(parse("Stage timings").is_err());
}

#[test]
fn counters_and_complete_partition_arithmetic_are_strict() {
    for record in [
        "calls=0; elapsed_ns=200; complete=true",
        "calls=1; elapsed_ns=200; complete=maybe",
        "calls=1; elapsed_ns=200",
        "calls=1; elapsed_ns=; complete=true",
        "calls=1; elapsed_ns=-1; complete=true",
        "calls=1; elapsed_ns=18446744073709551616; complete=true",
        "calls=18446744073709551616; elapsed_ns=200; complete=true",
        "calls=+1; elapsed_ns=200; complete=true",
        "elapsed_ns=200; calls=1; complete=true",
        "calls=1; elapsed_ns=200; complete=true; unknown=1",
    ] {
        let text = SECTION.replace(GROUNDING, &format!("grounding: {record}"));
        assert!(parse(&text).is_err(), "{record}");
    }
    for text in [
        SECTION.replace("elapsed_ns=1000", "elapsed_ns=18446744073709551616"),
        SECTION.replace(
            "unattributed: elapsed_ns=160",
            "unattributed: elapsed_ns=18446744073709551616",
        ),
        SECTION.replace(
            "unattributed: elapsed_ns=160",
            "unattributed: elapsed_ns=159",
        ),
        SECTION.replace("elapsed_ns=500", "elapsed_ns=18446744073709551615"),
        SECTION.replace("complete=true", "complete=false"),
    ] {
        assert!(parse(&text).is_err(), "{text}");
    }
}

#[test]
fn maximum_representable_counters_remain_exact() {
    let text = SECTION
        .replace(
            "stage driver: elapsed_ns=1000",
            "stage driver: elapsed_ns=18446744073709551615",
        )
        .replace(
            "source_preparation: calls=1; elapsed_ns=100; complete=true",
            "source_preparation: unmeasured",
        )
        .replace(
            GROUNDING,
            "grounding: calls=18446744073709551615; elapsed_ns=18446744073709551615; complete=true",
        )
        .replace(
            "solving: calls=3; elapsed_ns=500; complete=true",
            "solving: unmeasured",
        )
        .replace(
            "observation_output: calls=2; elapsed_ns=40; complete=true",
            "observation_output: unmeasured",
        )
        .replace("unattributed: elapsed_ns=160", "unattributed: elapsed_ns=0");
    let timing = parse(&text).unwrap().unwrap();
    assert!(timing.complete);
    assert_eq!(timing.driver_elapsed_ns, u64::MAX);
    assert_eq!(timing.unattributed_elapsed_ns, Some(0));
    let grounding = timing.stages["grounding"].as_ref().unwrap();
    assert_eq!(
        (grounding.calls, grounding.elapsed_ns),
        (u64::MAX, u64::MAX)
    );
}

#[test]
fn grounding_mode_and_availability_must_agree() {
    for text in [
        SECTION.replace("grounding_mode: eager", "grounding_mode: unentered"),
        SECTION.replace("grounding_mode: eager", "grounding_mode: lazy_interleaved"),
        SECTION.replace(GROUNDING, "grounding: unmeasured"),
        SECTION.replace(GROUNDING, "grounding: unavailable=interleaved"),
        SECTION.replace(GROUNDING, "grounding: unavailable=unknown"),
        SECTION.replace(
            "solving: calls=3; elapsed_ns=500; complete=true",
            "solving: unavailable=interleaved",
        ),
        SECTION
            .replace("grounding_mode: eager", "grounding_mode: mixed")
            .replace(GROUNDING, "grounding: unavailable=interleaved"),
        SECTION
            .replace("grounding_mode: eager", "grounding_mode: lazy_interleaved")
            .replace(GROUNDING, "grounding: unmeasured"),
    ] {
        assert!(parse(&text).is_err(), "{text}");
    }
}
