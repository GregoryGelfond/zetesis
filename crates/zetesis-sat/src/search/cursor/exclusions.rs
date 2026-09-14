//! Exact history ownership, separately from DPLL traversal and CNF storage.

use std::collections::BTreeSet;

use super::Cursor;
use super::tests::{budget, exclude};
use crate::{AdmissionLimits, Cnf, Control, Incomplete, Literal, ProjectionLimits, Solve};

#[test]
fn restarts_preserve_exclusions_without_watching_their_literals() {
    let control = Control::default();
    let mut charged = budget(&control);
    let mut cnf = Cnf::new(5, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::projected(3, crate::ProjectionLimits::default()).unwrap();
    let mut seen = BTreeSet::new();
    loop {
        match cursor.query(&cnf, &mut charged) {
            Solve::Sat(assignment) => {
                let key = &assignment.0[..3];
                assert!(seen.insert(key.to_vec()));
                assert_eq!(
                    cursor.state.as_ref().unwrap().base_clauses,
                    cnf.clauses().len()
                );
                exclude(&mut cursor, &cnf, key, &mut charged).unwrap();
                // A real restriction changes the auxiliary universe's models,
                // but neither the semantic width nor the retained exclusions.
                if seen.len() == 1 {
                    cnf.append(vec![Literal::new(4, true)]).unwrap();
                }
                assert_eq!(cnf.clauses().len(), 1, "no materialized blocking clauses");
                cursor.restart();
            }
            Solve::Unsat => break,
            Solve::Inconclusive(error) => panic!("finite exact history: {error}"),
        }
    }
    assert_eq!(
        seen,
        (0..8)
            .map(|mask| (0..3).map(|bit| mask & (1 << bit) != 0).collect())
            .collect()
    );
}

#[test]
fn final_membership_rejects_a_revisited_traversal_leaf() {
    let control = Control::default();
    let mut charged = budget(&control);
    let cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::projected(2, crate::ProjectionLimits::default()).unwrap();
    let Solve::Sat(first) = cursor.query(&cnf, &mut charged) else {
        panic!("first leaf")
    };
    exclude(&mut cursor, &cnf, &first.0[..2], &mut charged).unwrap();
    // Disable normal traversal advancement. The independent final exact
    // membership check must still reject the same completed assignment.
    cursor.after_model = false;
    let Solve::Sat(next) = cursor.query(&cnf, &mut charged) else {
        panic!("unseen projection")
    };
    assert_ne!(&first.0[..2], &next.0[..2]);
    assert!(cnf.clauses().is_empty());
}

#[test]
fn exclusion_history_does_not_consume_cnf_admission() {
    let control = Control::default();
    let mut cnf = Cnf::new(
        3,
        vec![],
        AdmissionLimits {
            max_variables: 3,
            max_clauses: 1,
            max_literals: 1,
        },
    )
    .unwrap();
    let mut cursor = Cursor::projected(3, ProjectionLimits::default()).unwrap();
    let mut charged = budget(&control);
    let mut seen = BTreeSet::new();
    loop {
        match cursor.query(&cnf, &mut charged) {
            Solve::Sat(assignment) => {
                assert!(seen.insert(assignment.0.clone()));
                exclude(&mut cursor, &cnf, &assignment.0, &mut charged).unwrap();
            }
            Solve::Unsat => break,
            Solve::Inconclusive(error) => panic!("independent history budget: {error}"),
        }
    }
    assert_eq!(seen.len(), 8);
    assert_eq!(cursor.projection_statistics().entries, 8);
    assert!(cnf.clauses().is_empty());
    // All authored CNF capacity remains available after the eight exclusions.
    cnf.append(vec![Literal::new(0, true)]).unwrap();
    assert_eq!(cnf.clauses().len(), 1);
}

#[test]
fn interrupted_insertion_preserves_the_only_entry_slot() {
    let control = Control::default();
    let measured_cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
    let mut measured = budget(&control);
    exclude(
        &mut Cursor::projected(3, crate::ProjectionLimits::default()).unwrap(),
        &measured_cnf,
        &[false; 3],
        &mut measured,
    )
    .unwrap();
    for ceiling in 0..measured.statistics.work {
        let cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
        let mut cursor = Cursor::projected(
            3,
            ProjectionLimits {
                max_entries: 1,
                ..ProjectionLimits::default()
            },
        )
        .unwrap();
        let mut short = budget(&control);
        short.limits.max_work = ceiling;
        assert_eq!(
            exclude(&mut cursor, &cnf, &[false; 3], &mut short),
            Err(Incomplete::WorkLimit)
        );
        assert_eq!(short.statistics.work, ceiling);
        assert_eq!(cursor.projection_statistics().entries, 0);
        // A retry must fit the only logical history slot. An unfinished suffix
        // must not reject any of the remaining, independently enumerated keys.
        let mut complete = budget(&control);
        exclude(&mut cursor, &cnf, &[false; 3], &mut complete).unwrap();
        let mut seen = BTreeSet::new();
        loop {
            match cursor.query(&cnf, &mut complete) {
                Solve::Sat(assignment) => {
                    assert!(seen.insert(assignment.0));
                }
                Solve::Unsat => break,
                Solve::Inconclusive(error) => panic!("retry: {error}"),
            }
        }
        let expected = (1..8)
            .map(|mask| (0..3).map(|bit| mask & (1 << bit) != 0).collect())
            .collect();
        assert_eq!(seen, expected);
    }
}

#[test]
fn projection_width_is_fixed_across_restart() {
    let control = Control::default();
    let cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::projected(2, crate::ProjectionLimits::default()).unwrap();
    cursor.restart();
    assert_eq!(
        exclude(&mut cursor, &cnf, &[false; 3], &mut budget(&control)),
        Err(Incomplete::InvalidWitness)
    );
    assert_eq!(
        exclude(&mut cursor, &cnf, &[false; 1], &mut budget(&control)),
        Err(Incomplete::InvalidWitness)
    );
}

#[test]
fn distinct_projection_insertion_has_exact_partitioned_work() {
    let control = Control::default();
    for width in 0..=4 {
        let cnf = Cnf::new(width, vec![], AdmissionLimits::default()).unwrap();
        let mut cursor = Cursor::projected(width, crate::ProjectionLimits::default()).unwrap();
        let mut charged = budget(&control);
        for mask in 0..1 << width {
            let values: Vec<_> = (0..width).map(|bit| mask & (1 << bit) != 0).collect();
            let before = charged.statistics.work;
            exclude(&mut cursor, &cnf, &values, &mut charged).unwrap();
            assert_eq!(
                charged.statistics.work - before,
                if width == 0 {
                    2
                } else {
                    u64::try_from(width).unwrap() + 3
                }
            );
        }
        assert!(matches!(cursor.query(&cnf, &mut charged), Solve::Unsat));
    }
}

#[test]
fn interrupted_suffix_never_changes_an_existing_key() {
    let control = Control::default();
    for ceiling in 0..6 {
        let cnf = Cnf::new(
            3,
            vec![],
            AdmissionLimits {
                max_clauses: 2,
                max_literals: 6,
                ..AdmissionLimits::default()
            },
        )
        .unwrap();
        let mut cursor = Cursor::projected(3, crate::ProjectionLimits::default()).unwrap();
        exclude(&mut cursor, &cnf, &[false; 3], &mut budget(&control)).unwrap();
        let mut short = budget(&control);
        short.limits.max_work = ceiling;
        assert_eq!(
            exclude(&mut cursor, &cnf, &[false, true, true], &mut short),
            Err(Incomplete::WorkLimit)
        );
        // Existing exact membership survives; the incomplete suffix does not
        // become a second key. The same trie is read without a search cursor.
        let index = cursor.projections.as_mut().unwrap();
        for mask in 0..8 {
            let assignment = crate::Assignment((0..3).map(|bit| mask & (1 << bit) != 0).collect());
            assert_eq!(
                index.permits(&assignment, &mut budget(&control)).unwrap(),
                mask != 0
            );
        }
        exclude(
            &mut cursor,
            &cnf,
            &[false, true, true],
            &mut budget(&control),
        )
        .unwrap();
    }
}
