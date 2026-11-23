use super::{
    CoordWidth, IntWidth, PRIME_GROUPS, ResidualSkip, admit_reservation, coord_bytes, coord_width,
    group_count, group_range, group_start, int_bytes, int_width, reservation_fits,
};
use crate::formula_support::parallel_executor::TASKS;

#[test]
fn prime_groups_are_four_times_the_executor_slot_count() {
    assert_eq!(PRIME_GROUPS, 56);
    assert_eq!(PRIME_GROUPS, 4 * TASKS);
}

#[test]
fn group_geometry_depends_only_on_width_and_the_fixed_group_cap() {
    for width in [0, 1, 4, 14, 15, 55, 56, 57, 100, 1264] {
        let count = group_count(width, PRIME_GROUPS);
        assert_eq!(count, width.min(PRIME_GROUPS));
        assert_eq!(group_start(width, PRIME_GROUPS, 0), 0);
        assert_eq!(group_start(width, PRIME_GROUPS, count), width);
        let mut covered = 0;
        for index in 0..count {
            let range = group_range(width, PRIME_GROUPS, index);
            assert_eq!(range.start, group_start(width, PRIME_GROUPS, index));
            assert_eq!(range.end, group_start(width, PRIME_GROUPS, index + 1));
            assert!(!range.is_empty());
            assert_eq!(range.start, covered);
            covered = range.end;
        }
        assert_eq!(covered, width);
    }
}

#[test]
fn a_short_prefix_list_is_one_item_per_group() {
    let width = 10;
    assert_eq!(group_count(width, PRIME_GROUPS), 10);
    for index in 0..10 {
        assert_eq!(group_range(width, PRIME_GROUPS, index), index..index + 1);
    }
}

#[test]
fn remainder_groups_lead_and_the_partition_is_stable() {
    // 57 items, 56 groups: one extra in group 0, then 55 singletons after a
    // doubleton — width / 56 = 1, remainder 1.
    assert_eq!(group_range(57, PRIME_GROUPS, 0), 0..2);
    assert_eq!(group_range(57, PRIME_GROUPS, 1), 2..3);
    assert_eq!(group_range(57, PRIME_GROUPS, 55), 56..57);
    // Identical bounds if asked twice: geometry is a pure function of the data.
    assert_eq!(
        group_range(1264, PRIME_GROUPS, 0),
        group_range(1264, PRIME_GROUPS, 0)
    );
}

#[test]
fn fourteen_task_geometry_is_unchanged_for_the_existing_path() {
    let width = 100;
    assert_eq!(group_count(width, TASKS), TASKS);
    let mut start = 0;
    for index in 0..TASKS {
        let length = width / TASKS + usize::from(index < width % TASKS);
        assert_eq!(group_range(width, TASKS, index), start..start + length);
        start += length;
    }
    assert_eq!(start, width);
}

#[test]
fn coordinates_default_to_u16_then_u32_and_never_a_raw_usize() {
    assert_eq!(coord_width(0), Some(CoordWidth::U16));
    assert_eq!(coord_width(usize::from(u16::MAX)), Some(CoordWidth::U16));
    assert_eq!(
        coord_width(usize::from(u16::MAX) + 1),
        Some(CoordWidth::U32)
    );
    assert_eq!(coord_width(u32::MAX as usize), Some(CoordWidth::U32));
    assert_eq!(coord_width(u32::MAX as usize + 1), None);
    assert_eq!(coord_bytes(CoordWidth::U16), 4);
    assert_eq!(coord_bytes(CoordWidth::U32), 8);
    assert!(coord_bytes(CoordWidth::U16) < size_of::<(usize, usize)>());
}

#[test]
fn integers_default_to_i16_when_the_range_fits() {
    assert_eq!(int_width(0, 0), IntWidth::I16);
    assert_eq!(
        int_width(i32::from(i16::MIN), i32::from(i16::MAX)),
        IntWidth::I16
    );
    assert_eq!(int_width(i32::from(i16::MIN) - 1, 0), IntWidth::I32);
    assert_eq!(int_width(0, i32::from(i16::MAX) + 1), IntWidth::I32);
    assert_eq!(int_bytes(IntWidth::I16), 2);
    assert_eq!(int_bytes(IntWidth::I32), 4);
}

#[test]
fn a_tight_budget_declines_to_serial_instead_of_erroring() {
    let per_row = coord_bytes(CoordWidth::U16);
    assert!(reservation_fits(10, per_row, 1000));
    // Half of 20 bytes is 10; two 4-byte rows would need 8, which fits; three
    // need 12, which does not.
    assert!(reservation_fits(2, per_row, 20));
    assert!(!reservation_fits(3, per_row, 20));
    assert!(!reservation_fits(1, per_row, 0));
    // Overflowing row counts must not wrap into a false fit.
    assert!(!reservation_fits(usize::MAX, per_row, 8));
}

#[test]
fn reservation_picks_packed_widths_and_declines_when_the_budget_is_tight() {
    let (coords, ints) = admit_reservation(10, 3, 100, 1, 0, 10, 10_000).expect("fits");
    assert_eq!(coords, CoordWidth::U16);
    assert_eq!(ints, IntWidth::I16);
    let (coords, ints) = admit_reservation(10, 3, usize::from(u16::MAX) + 1, 1, 0, 40_000, 10_000)
        .expect("u32/i32 still fit a large budget");
    assert_eq!(coords, CoordWidth::U32);
    assert_eq!(ints, IntWidth::I32);
    assert!(admit_reservation(10, 3, u32::MAX as usize + 1, 0, 0, 0, 10_000).is_none());
    assert!(admit_reservation(1_000_000, 8, 10, 0, 0, 0, 20).is_none());
}

#[test]
fn a_packed_occurrence_is_narrower_than_two_usizes() {
    assert_eq!(super::coord_bytes(CoordWidth::U16), size_of::<u32>());
    assert_eq!(super::coord_bytes(CoordWidth::U32), size_of::<u64>());
    assert!(size_of::<u32>() < size_of::<(usize, usize)>());
    assert!(size_of::<u64>() < size_of::<(usize, usize)>());
}

#[test]
fn width_for_index_overflows_above_u32() {
    use crate::test_support::location;
    assert!(super::width_for_index(u32::MAX as usize, location()).is_ok());
    assert!(super::width_for_index(u32::MAX as usize + 1, location()).is_err());
}

#[test]
fn a_trailing_residual_skip_keeps_the_body_unsplit() {
    let skip = ResidualSkip::suffix(3).unwrap();
    assert!(!skip.is_empty());
    assert_eq!(skip.first(), Some(3));
    assert!(!skip.contains(2));
    assert!(skip.contains(3));
    assert!(skip.contains(10));
    assert!(ResidualSkip::none().is_empty());
    assert_eq!(ResidualSkip::none().first(), None);
    assert!(!ResidualSkip::none().contains(0));
    assert!(!ResidualSkip::none().contains(3));
}

#[test]
fn a_residual_suffix_index_that_does_not_fit_a_u16_is_declined() {
    assert!(ResidualSkip::suffix(usize::from(u16::MAX)).is_none());
    assert!(ResidualSkip::suffix(0).is_some());
}

#[test]
fn residual_from_raw_round_trips() {
    let skip = ResidualSkip::suffix(12).unwrap();
    assert_eq!(ResidualSkip::from_raw(skip.raw()), skip);
    assert_eq!(
        ResidualSkip::from_raw(ResidualSkip::NONE),
        ResidualSkip::none()
    );
}
