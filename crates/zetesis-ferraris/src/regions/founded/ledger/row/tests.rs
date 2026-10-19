//! Packed matching obeys the scalar signed-condition law under every reading.

use super::*;
use crate::test_support::SinglePermits;

fn explanation(conditions: &[(usize, bool)], outputs: &[usize]) -> Explanation {
    Explanation {
        conditions: conditions.to_vec(),
        unsupported: outputs.to_vec(),
    }
}

/// The reference classifies signed propositions before considering outputs;
/// it does not use word grouping or the production scalar short-circuit loop.
fn expected(row: &Explanation, held: &[u64], cut: &[u64]) -> Deduction {
    let undecided: Vec<_> = row
        .conditions
        .iter()
        .copied()
        .filter(|&(atom, value)| !bit(if value { held } else { cut }, atom))
        .collect();
    if undecided
        .iter()
        .any(|&(atom, value)| bit(if value { cut } else { held }, atom))
    {
        return Deduction::None;
    }
    match undecided.as_slice() {
        [] => Deduction::ExcludeOutputs,
        &[(atom, value)] if row.unsupported.iter().any(|&output| bit(held, output)) => {
            Deduction::Require {
                atom,
                value: !value,
            }
        }
        _ => Deduction::None,
    }
}

/// Force the representation for semantic tests even when a sparse fixture
/// would correctly select the smaller scalar form in production.
fn packed(row: &Explanation) -> Row {
    let mut words = Vec::new();
    for &(atom, held) in &row.conditions {
        append(&mut words, 0, atom, held);
    }
    let premise_words = words.len();
    for &atom in &row.unsupported {
        append(&mut words, premise_words, atom, true);
    }
    Row::Packed {
        words,
        premise_words,
        conditions: row.conditions.len(),
        outputs: row.unsupported.len(),
    }
}

#[test]
fn all_partial_readings_match_the_signed_condition_law() {
    let premises = [0, 63, 64, 127];
    for count in 0..=premises.len() {
        for signs in 0..1 << count {
            let conditions: Vec<_> = premises[..count]
                .iter()
                .enumerate()
                .map(|(position, &atom)| (atom, signs & (1 << position) != 0))
                .collect();
            let source = explanation(&conditions, &[1, 128]);
            let row = packed(&source);
            assert!(matches!(row, Row::Packed { .. }));
            let scalar = Row::Scalar(explanation(&conditions, &[1, 128]));
            let atoms: Vec<_> = premises[..count].iter().copied().chain([1, 128]).collect();
            for mut reading in 0..4_usize.pow(u32::try_from(atoms.len()).unwrap()) {
                let mut held = [0_u64; 3];
                let mut cut = [0_u64; 3];
                for &atom in &atoms {
                    let state = reading % 4;
                    reading /= 4;
                    if state & 1 != 0 {
                        held[atom / 64] |= 1 << (atom % 64);
                    }
                    if state & 2 != 0 {
                        cut[atom / 64] |= 1 << (atom % 64);
                    }
                }
                let wanted = expected(&source, &held, &cut);
                assert_eq!(
                    row.deduction(&held, &cut, &mut Work::new(u64::MAX))
                        .unwrap(),
                    wanted
                );
                assert_eq!(
                    scalar
                        .deduction(&held, &cut, &mut Work::new(u64::MAX))
                        .unwrap(),
                    wanted
                );
            }
        }
    }
}

fn dense() -> Explanation {
    Explanation {
        conditions: (0..16).map(|atom| (atom, atom % 2 == 0)).collect(),
        unsupported: (16..80).collect(),
    }
}

fn prepared(source: Explanation) -> Row {
    let mut allowance = u64::MAX;
    Row::prepare(source, &mut allowance, &mut Work::new(u64::MAX), &mut 0)
        .unwrap()
        .unwrap()
}

#[test]
fn packing_retains_less_storage_for_dense_rows() {
    let source = dense();
    let scalar_bytes = size_of::<Row>() as u128 + source.retained_bytes();
    let row = prepared(source);
    assert!(matches!(row, Row::Packed { .. }));
    assert!(size_of::<Row>() as u128 + row.retained_bytes() < scalar_bytes);
    assert_eq!(row.conditions().collect::<Vec<_>>(), dense().conditions);
    assert_eq!(row.outputs().collect::<Vec<_>>(), dense().unsupported);
}

#[test]
fn dispersed_rows_keep_the_scalar_representation() {
    let source = Explanation {
        conditions: (0..16).map(|atom| (atom * 64, atom % 2 == 0)).collect(),
        unsupported: (16..80).map(|atom| atom * 64).collect(),
    };
    let expected_bytes = source.retained_bytes();
    let row = prepared(source);
    assert!(matches!(row, Row::Scalar(_)));
    assert_eq!(row.retained_bytes(), expected_bytes);
    assert_eq!(
        row.outputs().collect::<Vec<_>>(),
        (16..80).map(|atom| atom * 64).collect::<Vec<_>>()
    );
}

#[test]
fn peak_storage_includes_both_conversion_buffers() {
    let source = dense();
    let scalar = source.retained_bytes();
    let mut peak = 0;
    let mut allowance = u64::MAX;
    let row = Row::prepare(source, &mut allowance, &mut Work::new(u64::MAX), &mut peak)
        .unwrap()
        .unwrap();
    assert!(matches!(row, Row::Packed { .. }));
    assert_eq!(
        peak,
        size_of::<Row>() as u128 + size_of::<Explanation>() as u128 + scalar + row.retained_bytes()
    );
}

#[test]
fn logical_deduplication_ignores_the_physical_form() {
    let original = dense();
    for row in [Row::Scalar(dense()), packed(&original)] {
        let mut allowance = u64::MAX;
        assert_eq!(
            row.equivalent(&original, &mut allowance, &mut Work::new(u64::MAX))
                .unwrap(),
            Some(true)
        );
        let mut opposite = dense();
        opposite.conditions[7].1 = !opposite.conditions[7].1;
        let mut allowance = u64::MAX;
        assert_eq!(
            row.equivalent(&opposite, &mut allowance, &mut Work::new(u64::MAX))
                .unwrap(),
            Some(false)
        );
        let mut different = dense();
        *different.unsupported.last_mut().unwrap() = 80;
        let mut allowance = u64::MAX;
        assert_eq!(
            row.equivalent(&different, &mut allowance, &mut Work::new(u64::MAX))
                .unwrap(),
            Some(false)
        );
    }
}

#[test]
fn output_cursors_preserve_ascending_atom_events() {
    for row in [Row::Scalar(dense()), prepared(dense())] {
        let mut cursor = OutputCursor::default();
        for expected in 16..80 {
            assert_eq!(row.next_output(&mut cursor), Some(expected));
        }
        assert_eq!(row.next_output(&mut cursor), None);
        assert_eq!(row.next_output(&mut cursor), None);
        assert_eq!(row.output_count(), 64);
    }
}

#[test]
fn compact_matching_charges_word_reads_instead_of_atom_reads() {
    let source = dense();
    let row = prepared(dense());
    let scalar = Row::Scalar(source);
    // Exactly one open positive premise, with an output held in the second
    // output word. Every earlier premise and output must actually be read.
    let mut held = [0_u64; 2];
    let mut cut = [0_u64; 2];
    for atom in 1..16 {
        if atom % 2 == 0 {
            held[0] |= 1 << atom;
        } else {
            cut[0] |= 1 << atom;
        }
    }
    held[1] = 1 << 15;
    let mut packed_work = Work::new(u64::MAX);
    let mut scalar_work = Work::new(u64::MAX);
    let wanted = Deduction::Require {
        atom: 0,
        value: false,
    };
    assert_eq!(
        row.deduction(&held, &cut, &mut packed_work).unwrap(),
        wanted
    );
    assert_eq!(
        scalar.deduction(&held, &cut, &mut scalar_work).unwrap(),
        wanted
    );
    assert_eq!(packed_work.spent, 4);
    assert_eq!(scalar_work.spent, 81);
}

#[test]
fn every_interrupted_conversion_prefix_returns_no_row() {
    let mut complete = Work::new(u64::MAX);
    let mut allowance = u64::MAX;
    let row = Row::prepare(dense(), &mut allowance, &mut complete, &mut 0)
        .unwrap()
        .unwrap();
    assert!(matches!(row, Row::Packed { .. }));
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..complete.spent {
            let mut permits = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut permits);
            let mut allowance = u64::MAX;
            assert_eq!(
                Row::prepare(dense(), &mut allowance, &mut work, &mut 0),
                Err(failure)
            );
            assert_eq!(work.spent, prefix);
        }
    }
}

#[test]
fn optional_conversion_exhaustion_returns_no_row() {
    let mut complete = Work::new(u64::MAX);
    let mut allowance = u64::MAX;
    Row::prepare(dense(), &mut allowance, &mut complete, &mut 0)
        .unwrap()
        .unwrap();
    for allowance in 0..complete.spent {
        let mut remaining = allowance;
        let mut work = Work::new(u64::MAX);
        assert_eq!(
            Row::prepare(dense(), &mut remaining, &mut work, &mut 0).unwrap(),
            None
        );
        assert_eq!(work.spent, allowance);
        assert_eq!(remaining, 0);
    }
}

#[test]
fn every_interrupted_word_match_preserves_its_stop() {
    let source = explanation(&[(0, true), (64, false), (128, true)], &[1, 65, 129]);
    let row = packed(&source);
    let held = [0, 0, 3];
    let cut = [0, 1, 0];
    let mut measured = Work::new(u64::MAX);
    let wanted = row.deduction(&held, &cut, &mut measured).unwrap();
    assert_eq!(
        wanted,
        Deduction::Require {
            atom: 0,
            value: false
        }
    );
    assert_eq!(measured.spent, 7);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent {
            let mut permits = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut permits);
            assert_eq!(row.deduction(&held, &cut, &mut work), Err(failure));
            assert_eq!(work.spent, prefix);
            assert_eq!(
                row.deduction(&held, &cut, &mut Work::new(u64::MAX))
                    .unwrap(),
                wanted
            );
        }
    }
}

#[test]
fn a_duplicate_comparison_cannot_hide_a_stop() {
    let original = dense();
    let row = packed(&original);
    let mut measured = Work::new(u64::MAX);
    let mut allowance = u64::MAX;
    assert_eq!(
        row.equivalent(&original, &mut allowance, &mut measured)
            .unwrap(),
        Some(true)
    );
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent {
            let mut permits = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut permits);
            let mut allowance = u64::MAX;
            assert_eq!(
                row.equivalent(&original, &mut allowance, &mut work),
                Err(failure)
            );
            assert_eq!(work.spent, prefix);
        }
    }
    for prefix in 0..measured.spent {
        let mut allowance = prefix;
        assert_eq!(
            row.equivalent(&original, &mut allowance, &mut Work::new(u64::MAX))
                .unwrap(),
            None
        );
    }
}
