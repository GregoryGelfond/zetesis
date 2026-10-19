//! Syntactic dominance preserves each complete conditional-absence consequence.

use super::*;

fn copy(source: &Explanation) -> Explanation {
    explanation(&source.conditions, &source.unsupported)
}

fn verdict(row: &Row, proposed: &Explanation) -> bool {
    let mut remaining = u64::MAX;
    row.subsumes(proposed, &mut remaining, &mut Work::new(u64::MAX))
        .unwrap()
        .unwrap()
}

fn satisfied(row: &Explanation, interpretation: usize) -> bool {
    !row.conditions
        .iter()
        .all(|&(atom, value)| (interpretation & (1 << atom) != 0) == value)
        || row
            .unsupported
            .iter()
            .all(|&atom| interpretation & (1 << atom) == 0)
}

#[test]
fn subsumption_matches_sets_and_preserves_consequences() {
    let mut rows = Vec::new();
    for mut conditions in 0..9 {
        let mut literals = Vec::new();
        for atom in 0..2 {
            match conditions % 3 {
                1 => literals.push((atom, false)),
                2 => literals.push((atom, true)),
                _ => {}
            }
            conditions /= 3;
        }
        for outputs in 0..4 {
            rows.push(explanation(
                &literals,
                &(2..4)
                    .filter(|&atom| outputs & (1 << (atom - 2)) != 0)
                    .collect::<Vec<_>>(),
            ));
        }
    }
    for retained in &rows {
        for proposed in &rows {
            // An independent set definition, not the production ordered merge.
            let wanted = retained
                .conditions
                .iter()
                .all(|value| proposed.conditions.contains(value))
                && proposed
                    .unsupported
                    .iter()
                    .all(|atom| retained.unsupported.contains(atom));
            for row in [Row::Scalar(copy(retained)), packed(retained)] {
                assert_eq!(verdict(&row, proposed), wanted);
            }
            if wanted {
                // Exhaust every interpretation over these two premises and outputs.
                for interpretation in 0..16 {
                    assert!(
                        !satisfied(retained, interpretation) || satisfied(proposed, interpretation)
                    );
                }
            }
        }
    }
}

#[test]
fn stronger_rows_subsume_without_changing_their_storage() {
    let retained = dense();
    let mut conditions = retained.conditions.clone();
    conditions.push((80, false));
    let proposed = explanation(&conditions, &[17, 64]);
    let packed = prepared(copy(&retained));
    assert!(matches!(packed, Row::Packed { .. }));
    for row in [Row::Scalar(copy(&retained)), packed] {
        let before = row.retained_bytes();
        assert!(verdict(&row, &proposed));
        assert_eq!(row.retained_bytes(), before);
        assert_eq!(row.conditions().collect::<Vec<_>>(), retained.conditions);
        assert_eq!(row.outputs().collect::<Vec<_>>(), retained.unsupported);
    }
    assert!(
        !verdict(&prepared(proposed), &retained),
        "dominance is directional"
    );
}

#[test]
fn opposite_premises_and_missing_outputs_do_not_subsume() {
    let retained = explanation(&[(1, true), (64, false)], &[2, 63, 65, 127]);
    for row in [Row::Scalar(copy(&retained)), packed(&retained)] {
        assert!(!verdict(
            &row,
            &explanation(&[(1, false), (64, false)], &[63])
        ));
        assert!(!verdict(
            &row,
            &explanation(&[(1, true), (64, false)], &[63, 128])
        ));
        assert!(!verdict(&row, &explanation(&[(1, true)], &[63])));
        assert!(verdict(
            &row,
            &explanation(&[(0, false), (1, true), (64, false)], &[63, 127])
        ));
    }
}

#[test]
fn every_interrupted_inclusion_prefix_preserves_its_stop() {
    let retained = explanation(&[(1, true), (64, false)], &[2, 63, 65, 127]);
    let proposed = explanation(
        &[(0, false), (1, true), (32, true), (64, false), (100, true)],
        &[63, 127],
    );
    for row in [Row::Scalar(copy(&retained)), packed(&retained)] {
        let mut complete = Work::new(u64::MAX);
        let mut remaining = u64::MAX;
        assert_eq!(
            row.subsumes(&proposed, &mut remaining, &mut complete)
                .unwrap(),
            Some(true)
        );
        // Four proposed premises and four retained outputs are compared; the
        // extra final premise cannot change inclusion and is not traversed.
        assert_eq!(complete.spent, 8);
        for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            for prefix in 0..complete.spent {
                let mut permits = SinglePermits::new(prefix, failure);
                let mut work = Work::reserved(&mut permits);
                let mut remaining = u64::MAX;
                assert_eq!(
                    row.subsumes(&proposed, &mut remaining, &mut work),
                    Err(failure)
                );
                assert_eq!(work.spent, prefix);
                assert_eq!(row.conditions().collect::<Vec<_>>(), retained.conditions);
                assert_eq!(row.outputs().collect::<Vec<_>>(), retained.unsupported);
            }
        }
        for prefix in 0..complete.spent {
            let mut remaining = prefix;
            let mut work = Work::new(u64::MAX);
            assert_eq!(
                row.subsumes(&proposed, &mut remaining, &mut work).unwrap(),
                None
            );
            assert_eq!(work.spent, prefix);
            assert_eq!(remaining, 0);
            assert!(verdict(&row, &proposed));
        }
    }
}
