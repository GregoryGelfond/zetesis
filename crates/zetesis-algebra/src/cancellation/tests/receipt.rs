//! Whole-request admission preserves the caller's accepted-work receipt.

use super::super::{Meter, MeterWorkError};

#[test]
fn admitted_receipts_include_the_whole_request() {
    for (before, count, limit, expected) in [
        (0, 0, 0, 0),
        (7, 0, 7, 7),
        (7, 1, 8, 8),
        (u64::MAX - 1, 1, u64::MAX, u64::MAX),
        (u64::MAX, 0, u64::MAX, u64::MAX),
    ] {
        let mut accepted = before;
        assert_eq!(Meter::charge_receipt(&mut accepted, count, limit), Ok(()));
        assert_eq!(accepted, expected);
    }
}

#[test]
fn refused_receipts_keep_the_preceding_accepted_work() {
    let mut accepted = 7;
    assert_eq!(
        Meter::charge_receipt(&mut accepted, 2, 8),
        Err(MeterWorkError::Limit {
            observed: 9,
            limit: 8,
        })
    );
    assert_eq!(accepted, 7);
}

#[test]
fn zero_requests_check_imported_work_against_the_new_limit() {
    let mut accepted = 7;
    assert_eq!(
        Meter::charge_receipt(&mut accepted, 0, 3),
        Err(MeterWorkError::Limit {
            observed: 7,
            limit: 3,
        })
    );
    assert_eq!(accepted, 7);
}

#[test]
fn representable_refusals_keep_the_full_wide_demand() {
    for (before, count, observed) in [
        (u64::MAX, 1, u128::from(u64::MAX) + 1),
        (0, u128::from(u64::MAX) + 1, u128::from(u64::MAX) + 1),
        (0, u128::MAX, u128::MAX),
    ] {
        let mut accepted = before;
        assert_eq!(
            Meter::charge_receipt(&mut accepted, count, u64::MAX),
            Err(MeterWorkError::Limit {
                observed,
                limit: u128::from(u64::MAX),
            })
        );
        assert_eq!(accepted, before);
    }
}

#[test]
fn unrepresentable_demands_refuse_without_wrapping_the_receipt() {
    for before in [1, u64::MAX] {
        let mut accepted = before;
        assert_eq!(
            Meter::charge_receipt(&mut accepted, u128::MAX, u64::MAX),
            Err(MeterWorkError::Overflow)
        );
        assert_eq!(accepted, before);
    }
}

#[test]
fn each_request_uses_its_current_ceiling() {
    let mut accepted = 7;
    Meter::charge_receipt(&mut accepted, 1, 8).unwrap();
    assert_eq!(
        Meter::charge_receipt(&mut accepted, 0, 7),
        Err(MeterWorkError::Limit {
            observed: 8,
            limit: 7,
        })
    );
    assert_eq!(accepted, 8);
    Meter::charge_receipt(&mut accepted, 2, 10).unwrap();
    assert_eq!(accepted, 10);
}

#[test]
fn successful_receipt_charges_compose_additively() {
    for (before, first, second, limit) in [(0, 0, 0, 0), (3, 2, 5, 10), (7, 0, 1, 8)] {
        let mut sequential = before;
        Meter::charge_receipt(&mut sequential, first, limit).unwrap();
        Meter::charge_receipt(&mut sequential, second, limit).unwrap();
        let mut combined = before;
        Meter::charge_receipt(&mut combined, first + second, limit).unwrap();
        assert_eq!(sequential, combined);
    }
}

#[test]
fn preflight_returns_the_complete_admissible_receipt() {
    for (recorded, count, limit, proposed) in [
        (0, 0, 0, 0),
        (7, 0, 7, 7),
        (7, 3, 10, 10),
        (u64::MAX - 1, 1, u64::MAX, u64::MAX),
        (u64::MAX, 0, u64::MAX, u64::MAX),
    ] {
        assert_eq!(Meter::check_receipt(recorded, count, limit), Ok(proposed));
    }
}

#[test]
fn preflight_preserves_wide_typed_refusals() {
    for (recorded, count, limit, observed) in [
        (7, 0, 3, 7),
        (7, 2, 8, 9),
        (u64::MAX, 1, u64::MAX, u128::from(u64::MAX) + 1),
        (0, u128::MAX, u64::MAX, u128::MAX),
    ] {
        assert_eq!(
            Meter::check_receipt(recorded, count, limit),
            Err(MeterWorkError::Limit {
                observed,
                limit: u128::from(limit),
            })
        );
    }
    for recorded in [1, u64::MAX] {
        assert_eq!(
            Meter::check_receipt(recorded, u128::MAX, u64::MAX),
            Err(MeterWorkError::Overflow)
        );
    }
}

#[test]
fn wide_preflight_preserves_the_complete_u128_domain() {
    for (recorded, count, limit, expected) in [
        (0, 0, 0, 0),
        (u128::from(u64::MAX), 1, u128::MAX, u128::from(u64::MAX) + 1),
        (u128::MAX - 1, 1, u128::MAX, u128::MAX),
        (u128::MAX, 0, u128::MAX, u128::MAX),
    ] {
        assert_eq!(
            Meter::check_wide_receipt(recorded, count, limit),
            Ok(expected)
        );
    }
}

#[test]
fn wide_preflight_keeps_overflow_distinct_from_a_full_width_limit() {
    assert_eq!(
        Meter::check_wide_receipt(u128::MAX, 0, u128::MAX - 1),
        Err(MeterWorkError::Limit {
            observed: u128::MAX,
            limit: u128::MAX - 1,
        })
    );
    for limit in [0, u128::MAX] {
        assert_eq!(
            Meter::check_wide_receipt(u128::MAX, 1, limit),
            Err(MeterWorkError::Overflow)
        );
    }
}

#[test]
fn receipt_widths_have_the_same_admission_on_their_shared_domain() {
    for recorded in [0, 7, u64::MAX] {
        for count in [0, 1, u128::from(u64::MAX), u128::MAX] {
            for limit in [0, 7, u64::MAX] {
                assert_eq!(
                    Meter::check_receipt(recorded, count, limit).map(u128::from),
                    Meter::check_wide_receipt(u128::from(recorded), count, u128::from(limit))
                );
            }
        }
    }
}
