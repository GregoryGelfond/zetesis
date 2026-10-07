//! Exact ratios of median intervals and the reference's reported seconds.
use super::{Ratio, decimal_seconds_ns};

#[test]
fn ratios_order_by_their_exact_quotients() {
    // Both quotients round to one as doubles; their cross products differ.
    let smaller = Ratio::against_reference(u64::MAX, u64::MAX - 1);
    let larger = Ratio::against_reference(u64::MAX - 1, u64::MAX - 2);
    assert!(smaller < larger);
    assert_eq!(
        Ratio::against_reference(1, 2),
        Ratio::against_reference(2, 4)
    );
}

#[test]
fn ratios_show_thousandths_rounded_half_up() {
    for (numerator, denominator, shown) in [
        (3, 8, "0.375"),
        (1, 3, "0.333"),
        (2, 3, "0.667"),
        (1, 2_000, "0.001"),
        (1, 2_001, "0.000"),
        (0, 5, "0.000"),
        (2, 1, "2.000"),
        (u64::MAX, 1, "18446744073709551615.000"),
    ] {
        assert_eq!(
            Ratio::against_reference(numerator, denominator).to_string(),
            shown,
            "{numerator}/{denominator}"
        );
    }
}

#[test]
fn a_zero_reference_median_reads_as_one_nanosecond() {
    assert_eq!(Ratio::against_reference(7, 0).to_string(), "7.000");
}

#[test]
fn a_ratio_between_reports_needs_a_positive_earlier_median() {
    assert!(Ratio::between(7, 0).is_none());
    assert_eq!(Ratio::between(3, 8).unwrap().to_string(), "0.375");
}

#[test]
fn reported_seconds_convert_exactly_to_nanoseconds() {
    for (text, nanoseconds) in [
        ("0.012", 12_000_000),
        ("12.5", 12_500_000_000),
        ("0", 0),
        ("0.000000001", 1),
        ("0.1000000000", 100_000_000),
        ("18446744073.709551615", u64::MAX),
    ] {
        assert_eq!(decimal_seconds_ns(text), Some(nanoseconds), "{text}");
    }
}

#[test]
fn other_spellings_of_seconds_read_as_no_time() {
    for text in [
        "1e-3",
        "-0.001",
        "0.0000000001",
        "18446744074",
        "\"0.012\"",
        "",
        ".5",
    ] {
        assert_eq!(decimal_seconds_ns(text), None, "{text}");
    }
}

#[test]
fn historical_profiles_keep_their_bounded_policy() {
    let historical = serde_json::json!({"backend":"cpu", "grounder":"eager", "oracle":"auto", "workers":2, "completion_workers":1, "batch_size":64, "max_expansion_work":10_000_000});
    let shown = super::Profile(&historical).to_string();
    assert!(shown.contains("historical resource profile"));
    assert!(shown.contains("expansion work limit=10000000"));
    assert!(!shown.contains("requested policy=ordinary"));
}

#[test]
fn ordinary_profiles_do_not_imply_unlimited_execution() {
    let profile = serde_json::to_value(crate::selected::NativeExecution {
        memory_bytes: Some(123),
        ..Default::default()
    })
    .unwrap();
    let shown = super::Profile(&profile).to_string();
    assert!(shown.contains("requested policy=ordinary"));
    assert!(shown.contains("memory allowance=123 bytes"));
    assert!(!shown.contains("unlimited"));
    assert!(!shown.contains("completion workers"));
}

#[test]
fn historical_and_ordinary_profile_identities_remain_distinct() {
    let historical = serde_json::json!({"backend":"cpu", "grounder":"eager", "oracle":"auto", "workers":1, "completion_workers":1, "batch_size":64});
    let ordinary = serde_json::to_value(crate::selected::NativeExecution::default()).unwrap();
    let report = |profile| serde_json::json!({"report":{"plan":{"profiles":[profile]}}});
    let old = report(historical);
    let new = report(ordinary);
    let old = super::profiles(&super::Labelled {
        label: "old",
        report: &old,
    })
    .unwrap();
    let new = super::profiles(&super::Labelled {
        label: "new",
        report: &new,
    })
    .unwrap();
    assert_ne!(old, new);
}
