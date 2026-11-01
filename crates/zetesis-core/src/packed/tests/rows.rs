use std::collections::BTreeSet;

use proptest::prelude::*;

use crate::packed::{count, disjoint, first, union_assign, visit_set_bits};
use crate::packed::{intersection_count, xor_assign};

fn packed(positions: &BTreeSet<usize>, words: usize) -> Vec<u64> {
    let mut result = vec![0; words];
    for &position in positions {
        result[position / 64] |= 1 << (position % 64);
    }
    result
}

fn positions(words: &[u64]) -> BTreeSet<usize> {
    (0..words.len() * 64)
        .filter(|&position| words[position / 64] & (1 << (position % 64)) != 0)
        .collect()
}

proptest! {
    #[test]
    fn packed_selection_and_reductions_match_position_sets(
        left in proptest::collection::btree_set(0usize..256, 0..=256),
        right in proptest::collection::btree_set(0usize..256, 0..=256),
    ) {
        let left_words = packed(&left, 4);
        let right_words = packed(&right, 4);
        let mut operations = 0;
        let actual_count = count(&left_words, &mut |work| { operations += work; Ok::<_, ()>(()) }).unwrap();
        prop_assert_eq!(actual_count, left.len());
        prop_assert_eq!(operations, 9);
        prop_assert_eq!(first(&left_words, &mut |_| Ok::<_, ()>(())).unwrap(), left.first().copied());
        prop_assert_eq!(disjoint(&left_words, &right_words, &mut |_| Ok::<_, ()>(())).unwrap(), left.is_disjoint(&right));
        let mut actual_union = left_words.clone();
        union_assign(&mut actual_union, &right_words, &mut |_| Ok::<_, ()>(())).unwrap();
        prop_assert_eq!(positions(&actual_union), left.union(&right).copied().collect::<BTreeSet<_>>());
        let mut context = (0, Vec::new());
        visit_set_bits(&left_words, &mut context,
            &mut |(work, _), amount| { *work += amount; Ok::<_, ()>(()) },
            &mut |(_, seen), position| { seen.push(position); Ok::<_, ()>(()) },
        ).unwrap();
        prop_assert_eq!(context.0, 4 + 5 * left.len() as u64);
        prop_assert_eq!(context.1, left.iter().copied().collect::<Vec<_>>());
    }
    #[test]
    fn xor_is_set_symmetric_difference(
        left in proptest::collection::btree_set(0usize..256, 0..=256),
        right in proptest::collection::btree_set(0usize..256, 0..=256),
    ) {
        let mut actual = packed(&left, 4);
        xor_assign(&mut actual, &packed(&right, 4), &mut || Ok::<_, ()>(())).unwrap();
        let expected: BTreeSet<_> = left.symmetric_difference(&right).copied().collect();
        prop_assert_eq!(positions(&actual), expected);
    }

    #[test]
    fn intersection_counts_common_set_positions(
        left in proptest::collection::btree_set(0usize..256, 0..=256),
        right in proptest::collection::btree_set(0usize..256, 0..=256),
    ) {
        let actual = intersection_count(
            &packed(&left, 4), &packed(&right, 4), &mut || Ok::<_, ()>(()),
        ).unwrap();
        prop_assert_eq!(actual, left.intersection(&right).count());
    }
}

#[test]
fn a_shared_bit_absorbs_disjointness_before_the_suffix() {
    let mut remaining = 2;
    let result = disjoint(&[1, u64::MAX, 0], &[1, 0, u64::MAX], &mut |work| {
        if work > remaining {
            return Err("unread suffix");
        }
        remaining -= work;
        Ok(())
    });
    assert_eq!(result, Ok(false));
    assert_eq!(remaining, 0);
}

#[test]
fn first_position_does_not_read_after_its_witness() {
    let mut remaining = 6;
    let actual = first(&[0, 1 << 63, u64::MAX], &mut |work| {
        if work > remaining {
            return Err("unread suffix");
        }
        remaining -= work;
        Ok(())
    });
    assert_eq!(actual, Ok(Some(127)));
    assert_eq!(remaining, 0);
}

#[test]
fn union_refusal_keeps_the_unvisited_suffix() {
    for allowance in 0..3 {
        let mut words = [1, 2, 4];
        let mut left = allowance;
        assert_eq!(
            union_assign(&mut words, &[8, 16, 32], &mut |_| {
                if left == 0 {
                    return Err("stop");
                }
                left -= 1;
                Ok(())
            }),
            Err("stop")
        );
        let mut expected = [1, 2, 4];
        for index in 0..allowance {
            expected[index] |= [8, 16, 32][index];
        }
        assert_eq!(words, expected);
    }
}

#[test]
fn set_bit_visitation_retains_only_completed_visitor_effects() {
    let words = [3, 1 << 63, 1];
    for limit in 0..4 {
        let mut positions = Vec::new();
        let result = visit_set_bits(
            &words,
            &mut positions,
            &mut |_, _| Ok::<_, &str>(()),
            &mut |seen, position| {
                if seen.len() == limit {
                    return Err("visitor stopped");
                }
                seen.push(position);
                Ok(())
            },
        );
        assert_eq!(result, Err("visitor stopped"));
        assert_eq!(positions, [0, 1, 127, 128][..limit]);
    }
}

#[test]
fn count_and_selection_refusals_publish_no_partial_result() {
    for allowance in 0..7 {
        let mut remaining = allowance;
        assert_eq!(
            count(&[1, 2, 4], &mut |work| {
                if work > remaining {
                    return Err("stop");
                }
                remaining -= work;
                Ok(())
            }),
            Err("stop")
        );
    }
    for allowance in 0..5 {
        let mut remaining = allowance;
        assert_eq!(
            first(&[1], &mut |work| {
                if work > remaining {
                    return Err("stop");
                }
                remaining -= work;
                Ok(())
            }),
            Err("stop")
        );
    }
}

#[test]
fn xor_preserves_positions_at_word_boundaries() {
    let left = BTreeSet::from([0, 63, 64, 127, 128, 191, 192, 255]);
    let right = BTreeSet::from([0, 64, 128, 192]);
    let mut actual = packed(&left, 4);
    xor_assign(&mut actual, &packed(&right, 4), &mut || Ok::<_, ()>(())).unwrap();
    assert_eq!(positions(&actual), BTreeSet::from([63, 127, 191, 255]));
}

#[test]
fn intersection_counts_all_bits_in_full_words() {
    for width in 0..=4 {
        let words = vec![u64::MAX; width];
        let count = intersection_count(&words, &words, &mut || Ok::<_, ()>(())).unwrap();
        assert_eq!(count, 64 * width);
    }
}

#[test]
fn empty_xor_needs_no_operation() {
    xor_assign(&mut [], &[], &mut || Err("unexpected operation")).unwrap();
}

#[test]
fn empty_intersection_needs_no_operation() {
    assert_eq!(
        intersection_count(&[], &[], &mut || Err("unexpected operation")),
        Ok(0)
    );
}

#[test]
fn zero_xor_words_each_charge_one_operation() {
    let mut operations = 0;
    xor_assign(&mut [0; 3], &[0; 3], &mut || {
        operations += 1;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(operations, 3);
}

#[test]
fn zero_intersection_words_each_charge_two_operations() {
    let mut operations = 0;
    let count = intersection_count(&[0; 3], &[0; 3], &mut || {
        operations += 1;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!((count, operations), (0, 6));
}

#[derive(Debug, PartialEq, Eq)]
struct Refused(usize);

#[test]
fn xor_refusal_preserves_the_unvisited_suffix() {
    let left = BTreeSet::from([0, 63, 64, 65, 128, 191]);
    let right = BTreeSet::from([1, 63, 64, 127, 190, 191]);
    for allowed in 0..=3 {
        let mut actual = packed(&left, 3);
        let mut attempts = 0;
        let result = xor_assign(&mut actual, &packed(&right, 3), &mut || {
            attempts += 1;
            if attempts > allowed {
                Err(Refused(attempts))
            } else {
                Ok(())
            }
        });
        if allowed == 3 {
            assert_eq!(result, Ok(()));
            assert_eq!(attempts, 3);
        } else {
            assert_eq!(result, Err(Refused(allowed + 1)));
            assert_eq!(attempts, allowed + 1);
        }
        let mut expected = left.clone();
        // Only the admitted word prefix may toggle membership in the set.
        for &position in right.range(..allowed * 64) {
            if !expected.remove(&position) {
                expected.insert(position);
            }
        }
        assert_eq!(positions(&actual), expected);
    }
}

#[test]
fn intersection_stops_at_each_primitive_refusal() {
    let left = packed(&BTreeSet::from([0, 63, 64, 65, 128, 191]), 3);
    let right = packed(&BTreeSet::from([1, 63, 64, 127, 190, 191]), 3);
    for allowed in 0..=6 {
        let mut attempts = 0;
        let result = intersection_count(&left, &right, &mut || {
            attempts += 1;
            if attempts > allowed {
                Err(Refused(attempts))
            } else {
                Ok(())
            }
        });
        if allowed == 6 {
            assert_eq!(result, Ok(3));
            assert_eq!(attempts, 6);
        } else {
            assert_eq!(result, Err(Refused(allowed + 1)));
            assert_eq!(attempts, allowed + 1);
        }
    }
}

#[test]
#[should_panic(expected = "packed rows have equal lengths")]
fn xor_rejects_unequal_rows_before_any_operation() {
    let _ = xor_assign(&mut [1], &[], &mut || -> Result<(), ()> {
        panic!("unexpected operation")
    });
}

#[test]
#[should_panic(expected = "packed rows have equal lengths")]
fn intersection_rejects_unequal_rows_before_work() {
    let _ = intersection_count(&[], &[1], &mut || -> Result<(), ()> {
        panic!("unexpected operation")
    });
}
