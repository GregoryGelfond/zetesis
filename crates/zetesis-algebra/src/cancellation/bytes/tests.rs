use std::cell::Cell;
use std::mem::{align_of, size_of};

use super::{MeterBytes, MeterBytesError};

#[test]
fn a_borrowed_peak_admits_the_inclusive_ceiling() {
    let mut peak = 3;
    assert_eq!(MeterBytes::observe_peak(9, 9, &mut peak), Ok(()));
    assert_eq!(peak, 9);
}

#[test]
fn a_refused_observation_remains_in_the_borrowed_peak() {
    let mut peak = 8;
    assert_eq!(
        MeterBytes::observe_peak(16, 12, &mut peak),
        Err(MeterBytesError::Limit {
            observed: 16,
            limit: 12,
        })
    );
    assert_eq!(peak, 16);
}

#[test]
fn decreasing_observations_preserve_the_borrowed_peak() {
    let mut peak = 0;
    for total in [16, 8, 0] {
        assert_eq!(MeterBytes::observe_peak(total, 16, &mut peak), Ok(()));
        assert_eq!(peak, 16);
    }
}

#[test]
fn historical_peak_does_not_refuse_a_fitting_total() {
    let mut peak = 16;
    assert_eq!(MeterBytes::observe_peak(3, 3, &mut peak), Ok(()));
    assert_eq!(peak, 16);
    assert_eq!(MeterBytes::observe_peak(0, 0, &mut peak), Ok(()));
    assert_eq!(peak, 16);
}

#[test]
fn borrowed_peak_observations_preserve_full_width_demands() {
    let mut peak = 0;
    assert_eq!(
        MeterBytes::observe_peak(u128::MAX, u128::MAX, &mut peak),
        Ok(())
    );
    assert_eq!(peak, u128::MAX);
    assert_eq!(
        MeterBytes::observe_peak(u128::MAX, 0, &mut peak),
        Err(MeterBytesError::Limit {
            observed: u128::MAX,
            limit: 0,
        })
    );
    assert_eq!(peak, u128::MAX);
}

#[test]
fn the_byte_dimension_has_exactly_one_counter_layout() {
    assert_eq!(size_of::<MeterBytes>(), size_of::<u128>());
    assert_eq!(align_of::<MeterBytes>(), align_of::<u128>());
}

#[test]
fn both_constructors_start_with_the_same_zero_capacity() {
    for mut bytes in [MeterBytes::new(), MeterBytes::default()] {
        assert_eq!(bytes.current, 0);
        let mut peak = 0;
        let empty = bytes.reserve::<u8>(0, 0, &mut peak).unwrap();
        assert!(empty.is_empty());
        bytes.release(empty).unwrap();
        assert_eq!(bytes.current, 0);
        assert_eq!(peak, 0);
    }
}

#[test]
fn a_rejected_request_preserves_the_observed_prefix() {
    let mut bytes = MeterBytes::new();
    let mut peak = 0;
    bytes.observe(9, 9, &mut peak).unwrap();
    bytes.observe(4, 9, &mut peak).unwrap();
    assert_eq!(
        bytes.reserve::<u8>(6, 9, &mut peak),
        Err(MeterBytesError::Limit {
            observed: 10,
            limit: 9,
        })
    );
    assert_eq!(bytes.current, 4);
    assert_eq!(peak, 9);
}

#[test]
fn an_observed_excess_is_retained_before_refusal() {
    let mut bytes = MeterBytes::new();
    let mut peak = 0;
    bytes.observe(9, 9, &mut peak).unwrap();
    // The scalar observation denotes actual capacity, without assuming that
    // any allocator must return a particular amount of excess capacity.
    assert_eq!(
        bytes.observe(12, 11, &mut peak),
        Err(MeterBytesError::Limit {
            observed: 12,
            limit: 11,
        })
    );
    assert_eq!(bytes.current, 12);
    assert_eq!(peak, 12);
    bytes.observe(3, 3, &mut peak).unwrap();
    assert_eq!(bytes.current, 3);
    assert_eq!(peak, 12);
}

#[test]
fn reservations_and_releases_follow_actual_vector_capacity() {
    let mut bytes = MeterBytes::new();
    let mut peak = 0;
    let header = 17;
    bytes.observe(header, u128::MAX, &mut peak).unwrap();
    let first = bytes.reserve::<u64>(3, u128::MAX, &mut peak).unwrap();
    let first_payload = first.capacity() as u128 * size_of::<u64>() as u128;
    assert!(first.capacity() >= 3);
    assert_eq!(bytes.current, header + first_payload);
    let second = bytes.reserve::<u8>(5, u128::MAX, &mut peak).unwrap();
    let second_payload = second.capacity() as u128;
    let maximum = header + first_payload + second_payload;
    assert_eq!(bytes.current, maximum);
    assert_eq!(peak, maximum);
    bytes.release(first).unwrap();
    assert_eq!(bytes.current, header + second_payload);
    bytes.release(second).unwrap();
    assert_eq!(bytes.current, header);
    assert_eq!(peak, maximum);
}

#[test]
fn zero_payload_still_checks_the_current_total() {
    let mut bytes = MeterBytes::new();
    let mut peak = 0;
    bytes.observe(7, 7, &mut peak).unwrap();
    let values = bytes.reserve::<()>(usize::MAX, 7, &mut peak).unwrap();
    bytes.release(values).unwrap();
    assert_eq!(bytes.current, 7);
    assert_eq!(peak, 7);
    assert_eq!(
        bytes.reserve::<u8>(0, 6, &mut peak),
        Err(MeterBytesError::Limit {
            observed: 7,
            limit: 6,
        })
    );
    assert_eq!(bytes.current, 7);
    assert_eq!(peak, 7);
}

#[test]
fn overflowing_requests_do_not_change_the_receipt() {
    let mut bytes = MeterBytes::new();
    let mut peak = 0;
    bytes.observe(u128::MAX, u128::MAX, &mut peak).unwrap();
    assert_eq!(
        bytes.reserve::<u8>(1, u128::MAX, &mut peak),
        Err(MeterBytesError::Overflow)
    );
    assert_eq!(bytes.current, u128::MAX);
    assert_eq!(peak, u128::MAX);
}

#[test]
fn allocation_refusal_preserves_the_admitted_observations() {
    let mut bytes = MeterBytes::new();
    let mut peak = 0;
    bytes.observe(7, u128::MAX, &mut peak).unwrap();
    // A nonzero-sized Vec cannot have a byte capacity above isize::MAX.
    // This refuses before an allocator request, not by exhausting host memory.
    assert_eq!(
        bytes.reserve::<u8>(usize::MAX, u128::MAX, &mut peak),
        Err(MeterBytesError::Allocation)
    );
    assert_eq!(bytes.current, 7);
    assert_eq!(peak, 7);
}

#[test]
fn release_drops_the_vector_before_an_accounting_refusal() {
    struct Dropped<'a>(&'a Cell<usize>);
    impl Drop for Dropped<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Cell::new(0);
    let values = vec![Dropped(&drops)];
    let mut bytes = MeterBytes::new();
    assert_eq!(bytes.release(values), Err(MeterBytesError::Overflow));
    assert_eq!(drops.get(), 1);
    assert_eq!(bytes.current, 0);
}

#[test]
fn machine_capacity_refusals_retain_the_observation() {
    let mut peak = 3;
    assert_eq!(MeterBytes::observe_peak_usize(9, 9, &mut peak), Ok(()));
    assert_eq!(peak, 9);
    assert_eq!(
        MeterBytes::observe_peak_usize(16, 12, &mut peak),
        Err(MeterBytesError::Limit {
            observed: 16,
            limit: 12,
        })
    );
    assert_eq!(peak, 16);
    assert_eq!(MeterBytes::observe_peak_usize(3, 3, &mut peak), Ok(()));
    assert_eq!(peak, 16);
    assert_eq!(MeterBytes::observe_peak_usize(0, 0, &mut peak), Ok(()));
    assert_eq!(peak, 16);
}

#[test]
fn machine_capacity_observations_preserve_usize_max() {
    let mut peak = 0;
    assert_eq!(
        MeterBytes::observe_peak_usize(usize::MAX, usize::MAX, &mut peak),
        Ok(())
    );
    assert_eq!(peak, usize::MAX);
    assert_eq!(
        MeterBytes::observe_peak_usize(usize::MAX, usize::MAX - 1, &mut peak),
        Err(MeterBytesError::Limit {
            observed: usize::MAX as u128,
            limit: (usize::MAX - 1) as u128,
        })
    );
    assert_eq!(peak, usize::MAX);
}

#[test]
fn pure_byte_checks_keep_zero_and_full_width_boundaries() {
    for (total, limit) in [(0, 0), (7, 7), (u128::MAX, u128::MAX)] {
        assert_eq!(MeterBytes::check(total, limit), Ok(()));
    }
    for (total, limit) in [(1, 0), (8, 7), (u128::MAX, u128::MAX - 1)] {
        assert_eq!(
            MeterBytes::check(total, limit),
            Err(MeterBytesError::Limit {
                observed: total,
                limit,
            })
        );
    }
}

#[test]
fn u64_peak_observations_preserve_the_native_owner_and_diagnostics() {
    for (initial, total, limit, expected) in [
        (0, 0, 0, 0),
        (7, 9, 9, 9),
        (19, 7, 7, 19),
        (19, 0, 0, 19),
        (0, u64::MAX, u64::MAX, u64::MAX),
    ] {
        let mut peak = initial;
        assert_eq!(
            MeterBytes::observe_peak_u64(total, limit, &mut peak),
            Ok(())
        );
        assert_eq!(peak, expected);
    }
    let mut peak = 7;
    assert_eq!(
        MeterBytes::observe_peak_u64(u64::MAX, 9, &mut peak),
        Err(MeterBytesError::Limit {
            observed: u128::from(u64::MAX),
            limit: 9,
        })
    );
    assert_eq!(peak, u64::MAX);
}
