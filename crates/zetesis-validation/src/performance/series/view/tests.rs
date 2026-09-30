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
