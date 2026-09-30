//! Exact ratios of median intervals.
use super::Ratio;

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
