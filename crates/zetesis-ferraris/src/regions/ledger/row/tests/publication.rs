//! Publication preserves ordered cuts and the first held contradiction.

use super::*;

#[derive(Debug, PartialEq, Eq)]
enum Effect {
    Cut(usize),
    Hold(usize),
    Contradiction(usize),
}

fn expected_effects(source: &Literals, held: &[u64], cut: &[u64]) -> Vec<Effect> {
    let absent: Vec<usize> = source.outputs.iter().map(|&(atom, _)| atom).collect();
    let conflict = absent.iter().position(|&atom| bit(held, atom));
    let end = conflict.unwrap_or(absent.len());
    absent[..end]
        .iter()
        .filter(|&&atom| !bit(cut, atom))
        .map(|&atom| Effect::Cut(atom))
        .chain(conflict.map(|position| Effect::Contradiction(absent[position])))
        .collect()
}

fn publish(row: &Row, held: &[u64], cut: &mut [u64]) -> Vec<Effect> {
    let mut cursor = OutputCursor::default();
    let mut work = Work::new(u64::MAX);
    let mut effects = Vec::new();
    while let Some((atom, present)) = row
        .next_publication_output(&mut cursor, held, cut, &mut work)
        .unwrap()
    {
        assert!(!present, "these fixtures learn absences only");
        if bit(held, atom) {
            effects.push(Effect::Contradiction(atom));
            break;
        }
        if !bit(cut, atom) {
            effects.push(Effect::Cut(atom));
            cut[atom / 64] |= 1 << (atom % 64);
        }
    }
    effects
}

/// Signed outputs in order: the first output whose negation is decided
/// contradicts, and every earlier open output takes its sign.
fn expected_signed_effects(source: &Literals, held: &[u64], cut: &[u64]) -> Vec<Effect> {
    let (mut held, mut cut) = (held.to_vec(), cut.to_vec());
    let mut effects = Vec::new();
    for &(atom, present) in &source.outputs {
        if bit(if present { &cut } else { &held }, atom) {
            effects.push(Effect::Contradiction(atom));
            break;
        }
        let agrees = if present { &mut held } else { &mut cut };
        if !bit(agrees, atom) {
            agrees[atom / 64] |= 1 << (atom % 64);
            effects.push(if present {
                Effect::Hold(atom)
            } else {
                Effect::Cut(atom)
            });
        }
    }
    effects
}

fn publish_signed(row: &Row, held: &mut [u64], cut: &mut [u64]) -> Vec<Effect> {
    let mut cursor = OutputCursor::default();
    let mut work = Work::new(u64::MAX);
    let mut effects = Vec::new();
    while let Some((atom, present)) = row
        .next_publication_output(&mut cursor, held, cut, &mut work)
        .unwrap()
    {
        if bit(if present { &*cut } else { &*held }, atom) {
            effects.push(Effect::Contradiction(atom));
            break;
        }
        let agrees = if present { &mut *held } else { &mut *cut };
        if !bit(agrees, atom) {
            agrees[atom / 64] |= 1 << (atom % 64);
            effects.push(if present {
                Effect::Hold(atom)
            } else {
                Effect::Cut(atom)
            });
        }
    }
    effects
}

#[test]
fn every_signed_output_reading_preserves_ordered_effects() {
    // Both signs of atom 64: its condition admits no member of the class, so
    // an open atom is cut and then contradicts.
    let outputs = [
        (0, false),
        (1, true),
        (63, true),
        (64, false),
        (64, true),
        (65, true),
        (128, false),
    ];
    let source = Literals {
        conditions: Vec::new(),
        outputs: outputs.to_vec(),
    };
    let compact = packed(&source);
    let scalar = Row::Scalar(Literals {
        conditions: Vec::new(),
        outputs: outputs.to_vec(),
    });
    let atoms = [0, 1, 63, 64, 65, 128];
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
        let wanted = expected_signed_effects(&source, &held, &cut);
        for row in [&scalar, &compact] {
            let (mut actual_held, mut actual_cut) = (held, cut);
            assert_eq!(
                publish_signed(row, &mut actual_held, &mut actual_cut),
                wanted
            );
        }
    }
}

#[test]
fn every_output_reading_preserves_ordered_effects() {
    let outputs = [0, 1, 63, 64, 65, 128];
    let source = explanation(&[], &outputs);
    let compact = packed(&source);
    let scalar = Row::Scalar(explanation(&[], &outputs));
    assert!(matches!(compact, Row::Packed { .. }));
    assert!(matches!(scalar, Row::Scalar(_)));
    for mut reading in 0..4_usize.pow(u32::try_from(outputs.len()).unwrap()) {
        let mut held = [0_u64; 3];
        let mut cut = [0_u64; 3];
        for &atom in &outputs {
            let state = reading % 4;
            reading /= 4;
            if state & 1 != 0 {
                held[atom / 64] |= 1 << (atom % 64);
            }
            if state & 2 != 0 {
                cut[atom / 64] |= 1 << (atom % 64);
            }
        }
        let wanted = expected_effects(&source, &held, &cut);
        let mut expected_cut = cut;
        for effect in &wanted {
            if let Effect::Cut(atom) = effect {
                expected_cut[atom / 64] |= 1 << (atom % 64);
            }
        }
        for row in [&scalar, &compact] {
            let mut actual_cut = cut;
            assert_eq!(publish(row, &held, &mut actual_cut), wanted);
            assert_eq!(actual_cut, expected_cut);
        }
    }
}

#[test]
fn a_held_cut_output_retains_its_contradiction() {
    let source = explanation(&[], &[0, 63, 64, 128, 129]);
    let row = packed(&source);
    assert!(matches!(row, Row::Packed { .. }));
    let held = [0, 0, 1];
    let mut cut = [1, 1, 1];
    assert_eq!(
        publish(&row, &held, &mut cut),
        [Effect::Cut(63), Effect::Contradiction(128)]
    );
    assert_eq!(cut, [1 | (1 << 63), 1, 1]);
}

#[test]
fn settled_packed_outputs_charge_only_their_words() {
    let compact = prepared(dense());
    assert!(matches!(compact, Row::Packed { .. }));
    let scalar = Row::Scalar(dense());
    let held = [0; 2];
    let cut = [u64::MAX; 2];
    for (row, expected_visits, expected_reads) in [(&compact, 0, 2), (&scalar, 64, 64)] {
        let mut cursor = OutputCursor::default();
        let mut work = Work::new(u64::MAX);
        let mut visited = 0;
        while row
            .next_publication_output(&mut cursor, &held, &cut, &mut work)
            .unwrap()
            .is_some()
        {
            visited += 1;
        }
        assert_eq!(visited, expected_visits);
        assert_eq!(work.spent, expected_reads);
        assert_eq!(
            row.next_publication_output(&mut cursor, &held, &cut, &mut work),
            Ok(None)
        );
        assert_eq!(work.spent, expected_reads);
    }
}

#[test]
fn open_packed_outputs_charge_words_and_events() {
    let row = prepared(dense());
    assert!(matches!(row, Row::Packed { .. }));
    let held = [0; 2];
    let cut = [0; 2];
    let mut cursor = OutputCursor::default();
    let mut work = Work::new(u64::MAX);
    let mut visited = Vec::new();
    while let Some(atom) = row
        .next_publication_output(&mut cursor, &held, &cut, &mut work)
        .unwrap()
    {
        visited.push(atom);
    }
    assert_eq!(
        visited,
        (16..80).map(|atom| (atom, false)).collect::<Vec<_>>()
    );
    assert_eq!(work.spent, 66);
}

#[test]
fn publication_refusal_keeps_the_unread_output() {
    let source = explanation(&[], &[0, 2, 63, 64, 70, 128]);
    let held = [0; 3];
    let cut = [1 << 2, 1 | (1 << 6), 0];
    for row in [
        Row::Scalar(Literals {
            conditions: Vec::new(),
            outputs: source.outputs.clone(),
        }),
        packed(&source),
    ] {
        let mut complete = Work::new(u64::MAX);
        let mut cursor = OutputCursor::default();
        let mut wanted = Vec::new();
        while let Some(atom) = row
            .next_publication_output(&mut cursor, &held, &cut, &mut complete)
            .unwrap()
        {
            wanted.push(atom);
        }
        for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            for prefix in 0..complete.spent {
                let mut permits = SinglePermits::new(prefix, failure);
                let mut work = Work::reserved(&mut permits);
                let mut cursor = OutputCursor::default();
                let mut visited = Vec::new();
                let result = loop {
                    match row.next_publication_output(&mut cursor, &held, &cut, &mut work) {
                        Ok(Some(atom)) => visited.push(atom),
                        other => break other,
                    }
                };
                assert_eq!(result, Err(failure));
                assert_eq!(work.spent, prefix);
                assert_eq!(visited, wanted[..visited.len()]);
                let mut resumed = Work::new(u64::MAX);
                while let Some(atom) = row
                    .next_publication_output(&mut cursor, &held, &cut, &mut resumed)
                    .unwrap()
                {
                    visited.push(atom);
                }
                assert_eq!(visited, wanted);
                assert_eq!(work.spent + resumed.spent, complete.spent);
            }
        }
    }
}
