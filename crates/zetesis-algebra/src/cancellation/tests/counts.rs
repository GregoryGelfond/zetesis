//! Machine-sized named-count admission, independent of work and commit owners.

use super::super::{Meter, MeterCountError, MeterWideCountError};

#[test]
fn count_admission_keeps_zero_and_inclusive_machine_boundaries() {
    for (recorded, amount, limit, expected) in [
        (0, 0, 0, 0),
        (7, 0, 7, 7),
        (7, 2, 9, 9),
        (usize::MAX - 1, 1, usize::MAX, usize::MAX),
        (usize::MAX, 0, usize::MAX, usize::MAX),
    ] {
        assert_eq!(Meter::check_count(recorded, amount, limit), Ok(expected));
    }
}

#[test]
fn count_refusals_keep_the_exact_representable_demand() {
    for (recorded, amount, limit, observed) in [
        (0, 1, 0, 1),
        (7, 0, 3, 7),
        (7, 2, 8, 9),
        (usize::MAX - 1, 1, usize::MAX - 1, usize::MAX),
    ] {
        assert_eq!(
            Meter::check_count(recorded, amount, limit),
            Err(MeterCountError::Limit { observed, limit })
        );
    }
}

#[test]
fn count_overflow_precedes_even_a_maximum_ceiling() {
    for limit in [0, usize::MAX - 1, usize::MAX] {
        assert_eq!(
            Meter::check_count(usize::MAX, 1, limit),
            Err(MeterCountError::Overflow)
        );
    }
}

#[test]
fn wide_count_admission_keeps_zero_and_inclusive_boundaries() {
    for (recorded, amount, limit, expected) in [
        (0, 0, 0, 0),
        (7, 0, 7, 7),
        (7, 2, 9, 9),
        (u128::MAX - 1, 1, u128::MAX, u128::MAX),
        (u128::MAX, 0, u128::MAX, u128::MAX),
    ] {
        assert_eq!(
            Meter::check_wide_count(recorded, amount, limit),
            Ok(expected)
        );
    }
}

#[test]
fn wide_count_refusals_keep_the_complete_demand() {
    for (recorded, amount, limit, observed) in [
        (0, 1, 0, 1),
        (7, 0, 3, 7),
        (7, 2, 8, 9),
        (u128::MAX - 1, 1, u128::MAX - 1, u128::MAX),
    ] {
        assert_eq!(
            Meter::check_wide_count(recorded, amount, limit),
            Err(MeterWideCountError::Limit { observed, limit })
        );
    }
}

#[test]
fn wide_count_overflow_precedes_even_a_maximum_ceiling() {
    for limit in [0, u128::MAX - 1, u128::MAX] {
        assert_eq!(
            Meter::check_wide_count(u128::MAX, 1, limit),
            Err(MeterWideCountError::Overflow)
        );
    }
}
