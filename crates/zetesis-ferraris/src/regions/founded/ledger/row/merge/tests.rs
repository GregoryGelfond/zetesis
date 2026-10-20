//! Consolidation preserves signed consequences and publishes no partial union.

use super::*;
use crate::regions::founded::ledger::{Deduction, row::append};
use crate::test_support::SinglePermits;

fn explanation(conditions: &[(usize, bool)], outputs: &[usize]) -> Explanation {
    Explanation {
        conditions: conditions.to_vec(),
        unsupported: outputs.to_vec(),
    }
}

fn copy(source: &Explanation) -> Explanation {
    explanation(&source.conditions, &source.unsupported)
}

fn stored(source: &Explanation, packed: bool) -> Row {
    if !packed {
        return Row::Scalar(copy(source));
    }
    let mut words = Vec::new();
    for &(atom, held) in &source.conditions {
        append(&mut words, 0, atom, held);
    }
    let premise_words = words.len();
    for &atom in &source.unsupported {
        append(&mut words, premise_words, atom, true);
    }
    Row::Packed {
        words,
        premise_words,
        conditions: source.conditions.len(),
        outputs: source.unsupported.len(),
    }
}

fn merge(row: &Row, other: &mut Explanation, maximum: usize) -> Option<bool> {
    let mut remaining = u64::MAX;
    row.union_outputs(
        other,
        maximum,
        &mut remaining,
        &mut Work::new(u64::MAX),
        &mut 0,
    )
    .unwrap()
}

fn deduction(row: &Row, held: &[u64], cut: &[u64]) -> Deduction {
    row.deduction(held, cut, &mut Work::new(u64::MAX)).unwrap()
}

#[derive(Debug, PartialEq, Eq)]
enum Consequence {
    None,
    Require(usize, bool),
    Exclude(Vec<usize>),
    Refuted,
}

/// Read the observable consequence, including the caller's held-output test.
fn consequence(row: &Row, held: &[u64], cut: &[u64]) -> Consequence {
    match deduction(row, held, cut) {
        Deduction::None => Consequence::None,
        Deduction::Require { atom, value } => Consequence::Require(atom, value),
        Deduction::ExcludeOutputs => {
            if row
                .outputs()
                .any(|atom| held[atom / 64] & (1 << (atom % 64)) != 0)
            {
                Consequence::Refuted
            } else {
                Consequence::Exclude(
                    row.outputs()
                        .filter(|&atom| cut[atom / 64] & (1 << (atom % 64)) == 0)
                        .collect(),
                )
            }
        }
    }
}

#[test]
fn identical_conditions_union_distinct_outputs_in_order() {
    let conditions = [(0, true), (63, false), (64, true), (128, false)];
    let source = explanation(&conditions, &[1, 63, 64, 127, 128]);
    for packed in [false, true] {
        let row = stored(&source, packed);
        let before = row.retained_bytes();
        let mut other = explanation(&conditions, &[0, 63, 127, 129]);
        assert_eq!(merge(&row, &mut other, 64), Some(true));
        assert_eq!(other.conditions, conditions);
        assert_eq!(other.unsupported, [0, 1, 63, 64, 127, 128, 129]);
        assert_eq!(row.conditions().collect::<Vec<_>>(), source.conditions);
        assert_eq!(row.outputs().collect::<Vec<_>>(), source.unsupported);
        assert_eq!(row.retained_bytes(), before);
        for merged_packed in [false, true] {
            let merged = stored(&other, merged_packed);
            assert_eq!(merged.outputs().collect::<Vec<_>>(), other.unsupported);
        }
    }
}

#[test]
fn union_refuses_different_signed_conditions_without_mutation() {
    let source = explanation(&[(0, true), (64, false)], &[1, 65]);
    for packed in [false, true] {
        let row = stored(&source, packed);
        for conditions in [
            vec![(0, false), (64, false)],
            vec![(0, true)],
            vec![(0, true), (64, false), (128, true)],
            vec![(0, true), (128, false)],
        ] {
            let mut other = explanation(&conditions, &[2, 129]);
            let before = copy(&other);
            assert_eq!(merge(&row, &mut other, 64), Some(false));
            assert_eq!(other, before);
        }
    }
}

#[test]
fn union_limit_counts_distinct_outputs() {
    let conditions = [(128, false)];
    let source = explanation(&conditions, &(0..64).collect::<Vec<_>>());
    for packed in [false, true] {
        let row = stored(&source, packed);
        let mut same = copy(&source);
        assert_eq!(merge(&row, &mut same, 64), Some(true));
        assert_eq!(same, source);

        let mut overflow = explanation(&conditions, &(1..65).collect::<Vec<_>>());
        let before = copy(&overflow);
        assert_eq!(merge(&row, &mut overflow, 64), Some(false));
        assert_eq!(overflow, before);

        let partial = explanation(&conditions, &(32..64).collect::<Vec<_>>());
        let mut repeated = partial;
        assert_eq!(merge(&row, &mut repeated, 64), Some(true));
        assert_eq!(merge(&row, &mut repeated, 64), Some(true));
        assert_eq!(repeated, source);
    }
}

#[test]
fn union_preserves_partial_consequences_from_either_row() {
    let conditions = [(0, true), (63, false)];
    let left = explanation(&conditions, &[64]);
    let right = explanation(&conditions, &[127, 128]);
    let atoms = [0, 63, 64, 127, 128];
    for left_packed in [false, true] {
        let old_left = stored(&left, left_packed);
        let mut merged = copy(&right);
        assert_eq!(merge(&old_left, &mut merged, 64), Some(true));
        for right_packed in [false, true] {
            let old_right = stored(&right, right_packed);
            let combined = stored(&merged, right_packed);
            for mut reading in 0..3_usize.pow(atoms.len().try_into().unwrap()) {
                let mut held = [0_u64; 3];
                let mut cut = [0_u64; 3];
                for &atom in &atoms {
                    match reading % 3 {
                        1 => held[atom / 64] |= 1 << (atom % 64),
                        2 => cut[atom / 64] |= 1 << (atom % 64),
                        _ => {}
                    }
                    reading /= 3;
                }
                let actual = consequence(&combined, &held, &cut);
                for old in [&old_left, &old_right] {
                    match consequence(old, &held, &cut) {
                        Consequence::None => {}
                        Consequence::Require(atom, value) => {
                            assert_eq!(actual, Consequence::Require(atom, value));
                        }
                        Consequence::Refuted => assert_eq!(actual, Consequence::Refuted),
                        Consequence::Exclude(outputs) => match &actual {
                            Consequence::Refuted => {}
                            Consequence::Exclude(all) => {
                                assert!(outputs.iter().all(|atom| all.contains(atom)));
                            }
                            _ => panic!("union lost a completed output exclusion"),
                        },
                    }
                }
            }
        }
    }
}

#[test]
fn empty_conditions_still_union_all_outputs() {
    let source = explanation(&[], &[0, 64]);
    for packed in [false, true] {
        let row = stored(&source, packed);
        let mut other = explanation(&[], &[1, 128]);
        assert_eq!(merge(&row, &mut other, 64), Some(true));
        assert_eq!(other.unsupported, [0, 1, 64, 128]);
        assert_eq!(
            consequence(&stored(&other, packed), &[0; 3], &[0; 3]),
            Consequence::Exclude(vec![0, 1, 64, 128])
        );
    }
}

#[test]
fn outputs_aliasing_premises_preserve_partial_forcing() {
    let conditions = [(0, true), (64, false)];
    let source = explanation(&conditions, &[0, 1]);
    for packed in [false, true] {
        let row = stored(&source, packed);
        let mut other = explanation(&conditions, &[64, 128]);
        assert_eq!(merge(&row, &mut other, 64), Some(true));
        let combined = stored(&other, packed);
        assert_eq!(
            consequence(&combined, &[1, 0, 0], &[0; 3]),
            Consequence::Require(64, true)
        );
        assert_eq!(
            consequence(&combined, &[0; 3], &[0, 1, 0]),
            Consequence::None
        );
        assert_eq!(
            consequence(&combined, &[1, 0, 0], &[0, 1, 0]),
            Consequence::Refuted
        );
    }
}

#[test]
fn interrupted_union_preserves_both_inputs() {
    let conditions = [(0, true), (64, false), (128, true)];
    let source = explanation(&conditions, &[1, 63, 64, 127]);
    let proposed = explanation(&conditions, &[2, 63, 128, 129]);
    for packed in [false, true] {
        let row = stored(&source, packed);
        let bytes = row.retained_bytes();
        let mut complete = copy(&proposed);
        let mut measured = Work::new(u64::MAX);
        let mut peak = 0;
        let mut remaining = u64::MAX;
        assert_eq!(
            row.union_outputs(&mut complete, 64, &mut remaining, &mut measured, &mut peak),
            Ok(Some(true))
        );
        assert!(peak > proposed.retained_bytes());
        for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            for prefix in 0..measured.spent {
                let mut other = copy(&proposed);
                let mut permits = SinglePermits::new(prefix, failure);
                let mut work = Work::reserved(&mut permits);
                let mut remaining = u64::MAX;
                assert_eq!(
                    row.union_outputs(&mut other, 64, &mut remaining, &mut work, &mut 0),
                    Err(failure)
                );
                assert_eq!(work.spent, prefix);
                assert_eq!(other, proposed);
                assert_eq!(row.retained_bytes(), bytes);
                assert_eq!(row.outputs().collect::<Vec<_>>(), source.unsupported);
                assert_eq!(merge(&row, &mut other, 64), Some(true));
                assert_eq!(other, complete);
            }
        }
        for prefix in 0..measured.spent {
            let mut other = copy(&proposed);
            let mut remaining = prefix;
            let mut work = Work::new(u64::MAX);
            assert_eq!(
                row.union_outputs(&mut other, 64, &mut remaining, &mut work, &mut 0),
                Ok(None)
            );
            assert_eq!(work.spent, prefix);
            assert_eq!(other, proposed);
            assert_eq!(row.retained_bytes(), bytes);
        }
    }
}

#[test]
fn union_peak_includes_old_and_replacement_outputs() {
    let source = explanation(&[(0, true)], &[1, 2, 3, 4]);
    let row = stored(&source, true);
    let mut other = explanation(&[(0, true)], &[4, 5, 6, 7]);
    let retained = other.retained_bytes();
    let mut peak = 0;
    let mut remaining = u64::MAX;
    assert_eq!(
        row.union_outputs(
            &mut other,
            64,
            &mut remaining,
            &mut Work::new(u64::MAX),
            &mut peak
        ),
        Ok(Some(true))
    );
    assert_eq!(
        peak,
        size_of::<Explanation>() as u128
            + size_of::<Vec<usize>>() as u128
            + retained
            + other.unsupported.capacity() as u128 * size_of::<usize>() as u128
    );
}

#[test]
fn reverse_subsumption_preserves_units_or_refutes_earlier() {
    let old = explanation(&[(0, true), (63, true)], &[64]);
    let stronger = explanation(&[(0, true)], &[64, 128]);
    for packed in [false, true] {
        let row = stored(&old, packed);
        let mut remaining = u64::MAX;
        assert_eq!(
            row.subsumed_by(&stronger, &mut remaining, &mut Work::new(u64::MAX)),
            Ok(Some(true))
        );
        for new_packed in [false, true] {
            let new = stored(&stronger, new_packed);
            let mut held = [0_u64; 3];
            held[0] = 1 << 63;
            held[1] = 1;
            assert_eq!(
                consequence(&row, &held, &[0; 3]),
                Consequence::Require(0, false)
            );
            assert_eq!(
                consequence(&new, &held, &[0; 3]),
                Consequence::Require(0, false)
            );
            held[0] = 1;
            assert_eq!(
                consequence(&row, &held, &[0; 3]),
                Consequence::Require(63, false)
            );
            assert_eq!(consequence(&new, &held, &[0; 3]), Consequence::Refuted);
        }
    }
}

#[test]
fn reverse_subsumption_rejects_wrong_signs_or_missing_outputs() {
    let source = explanation(&[(1, true), (64, false)], &[2, 63, 65, 127]);
    for packed in [false, true] {
        let row = stored(&source, packed);
        for invalid in [
            explanation(&[(1, false)], &[2, 63, 65, 127]),
            explanation(&[(1, true)], &[2, 63, 65]),
            explanation(&[(1, true), (64, false), (128, true)], &[2, 63, 65, 127]),
        ] {
            let mut remaining = u64::MAX;
            assert_eq!(
                row.subsumed_by(&invalid, &mut remaining, &mut Work::new(u64::MAX)),
                Ok(Some(false))
            );
        }
    }
}

#[test]
fn interrupted_reverse_subsumption_retains_the_old_row() {
    let source = explanation(&[(0, true), (64, false), (128, true)], &[1, 63, 129]);
    let proposed = explanation(&[(0, true), (128, true)], &[1, 63, 64, 127, 129]);
    for packed in [false, true] {
        let row = stored(&source, packed);
        let bytes = row.retained_bytes();
        let mut measured = Work::new(u64::MAX);
        let mut remaining = u64::MAX;
        assert_eq!(
            row.subsumed_by(&proposed, &mut remaining, &mut measured),
            Ok(Some(true))
        );
        for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            for prefix in 0..measured.spent {
                let mut permits = SinglePermits::new(prefix, failure);
                let mut work = Work::reserved(&mut permits);
                let mut remaining = u64::MAX;
                assert_eq!(
                    row.subsumed_by(&proposed, &mut remaining, &mut work),
                    Err(failure)
                );
                assert_eq!(work.spent, prefix);
                assert_eq!(row.outputs().collect::<Vec<_>>(), source.unsupported);
                assert_eq!(row.conditions().collect::<Vec<_>>(), source.conditions);
                assert_eq!(row.retained_bytes(), bytes);
            }
        }
        for prefix in 0..measured.spent {
            let mut remaining = prefix;
            let mut work = Work::new(u64::MAX);
            assert_eq!(
                row.subsumed_by(&proposed, &mut remaining, &mut work),
                Ok(None)
            );
            assert_eq!(work.spent, prefix);
            assert_eq!(row.retained_bytes(), bytes);
        }
    }
}
