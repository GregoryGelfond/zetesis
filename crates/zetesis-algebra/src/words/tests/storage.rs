use std::mem::{align_of, size_of};

use crate::words::Words;

#[test]
fn storage_forms_keep_the_backing_layout() {
    macro_rules! layout {
        ($lane:ty, $storage:ty) => {
            assert_eq!(size_of::<Words<$lane, $storage>>(), size_of::<$storage>());
            assert_eq!(align_of::<Words<$lane, $storage>>(), align_of::<$storage>());
        };
    }
    layout!(u64, Vec<u64>);
    layout!(u64, Box<[u64]>);
    layout!(u64, &[u64]);
    layout!(u64, &mut [u64]);
    layout!(u64, [u64; 4]);
    layout!(u32, Vec<u32>);
    layout!(u32, Box<[u32]>);
    layout!(u32, &[u32]);
    layout!(u32, &mut [u32]);
    layout!(u32, [u32; 4]);
}

#[test]
fn wrapping_preserves_capacity_pointer_and_tail() {
    let mut source = Vec::with_capacity(7);
    source.extend([0_u64, u64::MAX]);
    let pointer = source.as_ptr();
    let capacity = source.capacity();
    let mut words = Words::<u64>::new(source);
    assert_eq!(words.as_ptr(), pointer);
    assert_eq!(words.capacity(), capacity);
    {
        let mut view = Words::<u64, _>::new(&mut words[..]);
        view.insert(1);
        assert_eq!(view.as_ptr(), pointer);
        assert_eq!(view[1], u64::MAX, "wrapping does not normalize a tail");
    }
    let source = words.into_storage();
    assert_eq!(source, [2, u64::MAX]);
    assert_eq!(source.as_ptr(), pointer);
    assert_eq!(source.capacity(), capacity);
    assert_eq!(
        format!("{:?}", Words::<u64, _>::new(&source[..])),
        format!("{source:?}")
    );
}

#[test]
fn clone_from_keeps_each_backings_reuse_policy() {
    let mut vector = Words::<u64>::new(Vec::with_capacity(9));
    let vector_pointer = vector.as_ptr();
    let vector_capacity = vector.capacity();
    vector.clone_from(&Words::new(vec![5, 9]));
    assert_eq!(vector.as_ptr(), vector_pointer);
    assert_eq!(vector.capacity(), vector_capacity);
    assert_eq!(&*vector, [5, 9]);

    let mut boxed = Words::<u64, _>::new(vec![1, 2].into_boxed_slice());
    let boxed_pointer = boxed.as_ptr();
    boxed.clone_from(&Words::new(vec![7, 11].into_boxed_slice()));
    assert_eq!(boxed.as_ptr(), boxed_pointer);
    assert_eq!(&*boxed, [7, 11]);
    let cloned = boxed.clone();
    assert_ne!(boxed.as_ptr(), cloned.as_ptr());
    boxed[0] = 0;
    assert_eq!(cloned[0], 7);
}

#[test]
fn lane_widths_represent_the_same_positions() {
    let mut wide = Words::<u64, _>::new([0; 3]);
    let mut narrow = Words::<u32, _>::new([0; 6]);
    let expected = [0, 1, 31, 32, 63, 64, 65, 95, 127, 128, 191];
    for bit in expected {
        wide.insert(bit);
        narrow.insert(bit);
    }
    assert_eq!(wide.positions().collect::<Vec<_>>(), expected);
    assert_eq!(narrow.positions().collect::<Vec<_>>(), expected);
    for bit in 0..192 {
        assert_eq!(wide.contains(bit), expected.contains(&bit));
        assert_eq!(narrow.contains(bit), expected.contains(&bit));
    }
    assert_eq!((0..6).map(|i| wide.half32(i)).collect::<Vec<_>>(), &*narrow);
}

#[test]
fn intersection_stops_at_the_first_shared_pair() {
    let left = [1_u32, 2, 4, 8];
    let right = [0, 2, 4, 8];
    for (right, expected, calls) in [
        (&right[..], true, 2),
        (&right[..1], false, 1),
        (&right[..0], false, 0),
    ] {
        let mut observed = 0;
        assert_eq!(
            Words::<u32, _>::new(&left[..]).intersects(Words::new(right), &mut || {
                observed += 1;
                Ok::<_, ()>(())
            }),
            Ok(expected)
        );
        assert_eq!(observed, calls);
    }
}

#[test]
fn intersection_preserves_the_typed_refusal_prefix() {
    let left = Words::<u32, _>::new([1, 2, 4]);
    let right = [0, 0, 4];
    for allowed in 0..3 {
        let mut observed = 0;
        let result = left.intersects(Words::new(&right[..]), &mut || {
            if observed == allowed {
                return Err(("pair", observed));
            }
            observed += 1;
            Ok(())
        });
        assert_eq!(result, Err(("pair", allowed)));
        assert_eq!(observed, allowed);
    }
}

#[test]
fn scan_from_respects_offsets_and_the_physical_tail() {
    let wide = Words::<u64, _>::new([1, 0, 1 | (1 << 63)]);
    let narrow = Words::<u32, _>::new([1, 0, 0, 0, 1, 1 << 31]);
    for from in 0..=192 {
        let expected = [0, 128, 191].into_iter().find(|&bit| bit >= from);
        assert_eq!(wide.next_from(from), expected);
        assert_eq!(narrow.next_from(from), expected);
    }
    assert_eq!(wide.next_from(usize::MAX), None);
    assert_eq!(narrow.next_from(usize::MAX), None);
}

#[test]
fn scan_callbacks_stop_at_a_match_or_typed_refusal() {
    let words = Words::<u32, _>::new([1, 0, 4, u32::MAX]);
    for (from, position, calls) in [
        (0, Some(0), 1),
        (1, Some(66), 3),
        (67, Some(96), 2),
        (128, None, 0),
    ] {
        let mut visited = 0;
        let result = words.next_from_with(from, || {
            visited += 1;
            Ok::<_, ()>(())
        });
        assert_eq!(result, Ok(position));
        assert_eq!(visited, calls);
    }
    for refused in 0..3 {
        let mut visited = 0;
        let result = words.next_from_with(1, || {
            if visited == refused {
                return Err(("lane", refused));
            }
            visited += 1;
            Ok(())
        });
        assert_eq!(result, Err(("lane", refused)));
        assert_eq!(visited, refused);
    }
}

#[test]
fn removal_preserves_other_positions_in_both_lane_widths() {
    let mut wide = Words::<u64, _>::new([u64::MAX; 2]);
    let mut narrow = Words::<u32, _>::new([u32::MAX; 4]);
    let removed = [0, 31, 32, 63, 64, 127];
    for bit in removed {
        wide.remove(bit);
        narrow.remove(bit);
    }
    let expected: Vec<_> = (0..128).filter(|bit| !removed.contains(bit)).collect();
    assert_eq!(wide.positions().collect::<Vec<_>>(), expected);
    assert_eq!(narrow.positions().collect::<Vec<_>>(), expected);
}

#[test]
fn backing_access_preserves_existing_vector_capacity() {
    let mut words = Words::<u64>::new(Vec::with_capacity(8));
    let pointer = words.as_ptr();
    let capacity = words.capacity();
    words.storage_mut().extend([3, 5]);
    assert_eq!(words.as_ptr(), pointer);
    assert_eq!(words.capacity(), capacity);
    assert_eq!(&*words, [3, 5]);
    assert!(Words::<u64>::default().is_empty());
    assert_eq!(Words::<u64>::default().capacity(), 0);
}

#[test]
fn refused_bit_advance_retains_the_exact_cursor() {
    let mut bits = Words::<u64>::bits((1 << 2) | (1 << 63));
    assert_eq!(
        bits.next_with(|| Err::<(), _>("allowance")),
        Err("allowance")
    );
    assert_eq!(bits.next(), Some(2));
    assert_eq!(bits.next_with(|| Err::<(), _>("deadline")), Err("deadline"));
    assert_eq!(bits.next_with(|| Ok::<_, ()>(())), Ok(Some(63)));
    assert_eq!(
        bits.next_with(|| panic!("empty cursor must not poll")),
        Ok::<_, ()>(None)
    );
    assert_eq!(bits.next(), None);
}

#[test]
fn visitor_failure_keeps_the_selected_bit_pending() {
    let mut bits = Words::<u32>::bits((1 << 3) | (1 << 31));
    let mut observed = Vec::new();
    assert_eq!(
        bits.visit_next(|bit| {
            observed.push(bit);
            Err::<(), _>("storage")
        }),
        Err("storage")
    );
    assert_eq!(observed, [3]);
    assert_eq!(bits.visit_next(|bit| Ok::<_, ()>(bit * 2)), Ok(Some(6)));
    assert_eq!(bits.next(), Some(31));
    assert_eq!(
        bits.visit_next(|_| panic!("exhausted cursor must not visit")),
        Ok::<Option<usize>, ()>(None)
    );
}

#[test]
fn checked_membership_distinguishes_absent_lanes() {
    let wide = Words::<u64, _>::new([1, 1 << 63]);
    let narrow = Words::<u32, _>::new([1, 0, 0, 1 << 31]);
    for index in 0..128 {
        assert_eq!(wide.get_bit(index), Some(index == 0 || index == 127));
        assert_eq!(narrow.get_bit(index), wide.get_bit(index));
    }
    for index in [128, usize::MAX] {
        assert_eq!(wide.get_bit(index), None);
        assert_eq!(narrow.get_bit(index), None);
    }
    assert_eq!(Words::<u64, _>::new([]).get_bit(0), None);
}

#[test]
fn lane_selection_defines_the_zero_boundary() {
    assert_eq!(Words::<u32>::first_bit(0), 32);
    assert_eq!(Words::<u64>::first_bit(0), 64);
    assert_eq!(Words::<u32>::last_bit(0), 32);
    assert_eq!(Words::<u64>::last_bit(0), 64);
    assert_eq!(Words::<u32>::lowest_bit(0), 0);
    assert_eq!(Words::<u64>::lowest_bit(0), 0);
    for bit in 0..64 {
        let word = u64::MAX << bit;
        assert_eq!(Words::<u64>::first_bit(word), bit);
        assert_eq!(Words::<u64>::lowest_bit(word), 1 << bit);
        assert_eq!(Words::<u64>::last_bit(word), 63);
        assert_eq!(Words::<u64>::last_bit(u64::MAX >> (63 - bit)), bit);
    }
    for bit in 0..32 {
        let word = u32::MAX << bit;
        assert_eq!(Words::<u32>::first_bit(word), bit);
        assert_eq!(Words::<u32>::lowest_bit(word), 1 << bit);
        assert_eq!(Words::<u32>::last_bit(word), 31);
        assert_eq!(Words::<u32>::last_bit(u32::MAX >> (31 - bit)), bit);
    }
}

#[test]
fn combined_lane_mutation_preserves_each_refused_prefix() {
    macro_rules! check {
        ($lane:ty, $method:ident, $op:tt) => {{
            let original: [$lane; 4] = [0b1100, 0b1010, 0b1001, 0b1111];
            let right: [$lane; 3] = [0b0011, 0b0110, 0b0101];
            for length in 0..=right.len() {
                for admitted in 0..=length {
                    let mut values = Words::<$lane, _>::new(original);
                    let mut calls = 0;
                    let result = values.$method(Words::new(&right[..length]), || {
                        if calls == admitted {
                            return Err(("refused", calls));
                        }
                        calls += 1;
                        Ok(())
                    });
                    let expected_result = if admitted < length {
                        Err(("refused", admitted))
                    } else {
                        Ok(())
                    };
                    assert_eq!(result, expected_result);
                    assert_eq!(calls, admitted);
                    for index in 0..original.len() {
                        let expected = if index < admitted {
                            original[index] $op right[index]
                        } else {
                            original[index]
                        };
                        assert_eq!(values[index], expected);
                    }
                }
            }
        }};
    }
    check!(u32, union_assign, |);
    check!(u64, union_assign, |);
    check!(u32, intersect_assign, &);
    check!(u64, intersect_assign, &);
    check!(u32, xor_assign, ^);
    check!(u64, xor_assign, ^);
}

#[test]
fn fill_refusal_preserves_the_unvisited_suffix() {
    macro_rules! check {
        ($lane:ty) => {{
            for admitted in 0..=3 {
                let mut values = Words::<$lane, _>::new([1, 2, 4]);
                let mut calls = 0;
                let result = values.fill_with(7, || {
                    if calls == admitted {
                        return Err(calls);
                    }
                    calls += 1;
                    Ok(())
                });
                assert_eq!(result, if admitted < 3 { Err(admitted) } else { Ok(()) });
                assert_eq!(calls, admitted);
                for (index, old) in [1, 2, 4].into_iter().enumerate() {
                    assert_eq!(values[index], if index < admitted { 7 } else { old });
                }
            }
            let mut empty = Words::<$lane, _>::new([]);
            assert_eq!(empty.fill_with(0, || Err("must not poll")), Ok(()));
        }};
    }
    check!(u32);
    check!(u64);
}

#[test]
fn combined_lanes_do_not_visit_excess_source() {
    macro_rules! check {
        ($lane:ty, $method:ident, $op:tt) => {{
            let right: [$lane; 3] = [6, 5, 3];
            for length in 0..=2 {
                let mut backing: [$lane; 2] = [3, 7];
                let mut values = Words::<$lane, _>::new(&mut backing[..length]);
                let mut calls = 0;
                assert_eq!(values.$method(Words::new(&right[..]), || {
                    calls += 1;
                    if calls > length { Err("excess source") } else { Ok(()) }
                }), Ok(()));
                assert_eq!(calls, length);
                for (index, original) in [3, 7].into_iter().enumerate() {
                    assert_eq!(backing[index], if index < length { original $op right[index] } else { original });
                }
            }
        }};
    }
    check!(u32, union_assign, |);
    check!(u64, union_assign, |);
    check!(u32, intersect_assign, &);
    check!(u64, intersect_assign, &);
    check!(u32, xor_assign, ^);
    check!(u64, xor_assign, ^);
}

#[test]
fn lane_population_counts_every_stored_bit() {
    for width in 0..=64 {
        let word = if width == 64 {
            u64::MAX
        } else {
            (1_u64 << width) - 1
        };
        assert_eq!(Words::<u64>::popcount(word), width);
    }
    for width in 0..=32 {
        let word = if width == 32 {
            u32::MAX
        } else {
            (1_u32 << width) - 1
        };
        assert_eq!(Words::<u32>::popcount(word), width);
    }
}

#[test]
fn subset_checks_and_scans_one_backing_view() {
    struct ChangingView(std::cell::Cell<bool>);

    impl AsRef<[u64]> for ChangingView {
        fn as_ref(&self) -> &[u64] {
            if self.0.replace(true) { &[0, 1] } else { &[1] }
        }
    }

    let left = Words::<u64, _>::new(ChangingView(std::cell::Cell::new(false)));
    let mut calls = 0;
    let result = left.is_subset_of(Words::new(&[0]), &mut || {
        calls += 1;
        Ok::<_, ()>(())
    });
    assert_eq!(result, Ok(false));
    assert_eq!(calls, 3);
}

#[test]
fn subset_refusal_preserves_the_deciding_prefix() {
    macro_rules! check {
        ($lane:ty) => {{
            let left: [$lane; 3] = [1, 2, 4];
            let right: [$lane; 3] = [1, 0, 4];
            // The second pair is the first witness. Even a callback willing
            // to continue must not be called for the third pair.
            for admitted in 0..=6 {
                let mut calls = 0;
                let result = Words::<$lane, _>::new(&left[..]).is_subset_of(
                    Words::new(&right[..]),
                    &mut || {
                        if calls == admitted {
                            return Err(("primitive", calls));
                        }
                        calls += 1;
                        Ok(())
                    },
                );
                assert_eq!(
                    result,
                    if admitted == 6 {
                        Ok(false)
                    } else {
                        Err(("primitive", admitted))
                    }
                );
                assert_eq!(calls, admitted);
                assert_eq!(left, [1, 2, 4]);
                assert_eq!(right, [1, 0, 4]);
            }
        }};
    }
    check!(u32);
    check!(u64);
}

#[test]
fn subset_includes_the_final_physical_bit() {
    macro_rules! check {
        ($lane:ty) => {{
            let left: [$lane; 3] = [0, 0, 1 << (<$lane>::BITS - 1)];
            for (right, expected) in [([0; 3], false), (left, true)] {
                let mut calls = 0;
                assert_eq!(
                    Words::<$lane, _>::new(&left[..]).is_subset_of(
                        Words::new(&right[..]),
                        &mut || {
                            calls += 1;
                            Ok::<_, ()>(())
                        }
                    ),
                    Ok(expected)
                );
                assert_eq!(calls, 9);
            }
            assert_eq!(
                Words::<$lane, _>::new([])
                    .is_subset_of(Words::new(&[]), &mut || Err("empty storage must not poll")),
                Ok(true)
            );
        }};
    }
    check!(u32);
    check!(u64);
}

#[test]
#[should_panic(expected = "packed subset shapes agree")]
fn subset_shape_is_checked_before_callbacks() {
    let _ = Words::<u64, _>::new([1]).is_subset_of(Words::new(&[]), &mut || -> Result<(), ()> {
        panic!("callback preceded shape validation")
    });
}

#[test]
fn cursor_checks_and_scans_one_backing_view() {
    struct ChangingView<'a> {
        first: &'a [u64],
        later: &'a [u64],
        borrows: std::cell::Cell<usize>,
    }

    impl AsRef<[u64]> for ChangingView<'_> {
        fn as_ref(&self) -> &[u64] {
            let borrows = self.borrows.get();
            self.borrows.set(borrows + 1);
            if borrows == 0 { self.first } else { self.later }
        }
    }

    let original: &[u64] = &[1, 2];
    let longer: &[u64] = &[1, 2, 4];
    for (first, later, from, allowance, expected, visits) in [
        (original, &[][..], 1, 2, Ok(Some(65)), 2),
        (original, longer, 66, 2, Ok(None), 1),
        (original, longer, 128, 0, Ok(None), 0),
        (&[][..], longer, 0, 0, Ok(None), 0),
        (original, &[][..], 1, 1, Err(1), 2),
    ] {
        let view = Words::<u64, _>::new(ChangingView {
            first,
            later,
            borrows: std::cell::Cell::new(0),
        });
        let mut calls = 0;
        let result = view.next_from_with(from, || {
            let previous = calls;
            calls += 1;
            if previous == allowance {
                Err(previous)
            } else {
                Ok(())
            }
        });
        assert_eq!(result, expected);
        assert_eq!(calls, visits);
        assert_eq!(view.storage().borrows.get(), 1);
    }
}
