//! Exact history ownership, separately from DPLL traversal and CNF storage.

use std::collections::BTreeSet;

use super::Cursor;
use super::tests::{budget, exclude};
use crate::{AdmissionError, AdmissionLimits, Cnf, Control, Incomplete, Literal, Resource, Solve};

#[test]
fn restarts_preserve_exclusions_without_watching_their_literals() {
    let control = Control::default();
    let mut charged = budget(&control);
    let mut cnf = Cnf::new(5, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::projected(3);
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
                exclude(&mut cursor, &mut cnf, key, &mut charged).unwrap();
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
    let mut cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::projected(2);
    let Solve::Sat(first) = cursor.query(&cnf, &mut charged) else {
        panic!("first leaf")
    };
    exclude(&mut cursor, &mut cnf, &first.0[..2], &mut charged).unwrap();
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
fn logical_history_admission_keeps_inclusive_clause_and_literal_limits() {
    let control = Control::default();
    for (clauses, literals, expected) in [
        (0, 3, Some(Resource::Clauses)),
        (1, 2, Some(Resource::Literals)),
        (1, 3, None),
    ] {
        let mut cnf = Cnf::new(
            3,
            vec![],
            AdmissionLimits {
                max_variables: 3,
                max_clauses: clauses,
                max_literals: literals,
            },
        )
        .unwrap();
        let mut cursor = Cursor::projected(3);
        let result = exclude(
            &mut cursor,
            &mut cnf,
            &[false, true, false],
            &mut budget(&control),
        );
        if let Some(resource) = expected {
            assert_eq!(
                result,
                Err(Incomplete::Admission(AdmissionError::Limit {
                    resource,
                    observed: if resource == Resource::Clauses { 1 } else { 3 },
                    limit: if resource == Resource::Clauses {
                        clauses
                    } else {
                        literals
                    }
                }))
            );
        } else {
            result.unwrap();
            assert!(matches!(
                exclude(
                    &mut cursor,
                    &mut cnf,
                    &[true, false, false],
                    &mut budget(&control)
                ),
                Err(Incomplete::Admission(AdmissionError::Limit {
                    resource: Resource::Clauses,
                    observed: 2,
                    limit: 1
                }))
            ));
        }
        assert!(cnf.clauses().is_empty());
    }
}

#[test]
fn interrupted_insertion_restores_history_and_admission() {
    let control = Control::default();
    let mut measured_cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
    let mut measured = budget(&control);
    exclude(
        &mut Cursor::projected(3),
        &mut measured_cnf,
        &[false; 3],
        &mut measured,
    )
    .unwrap();
    for ceiling in 0..measured.statistics.work {
        let limits = AdmissionLimits {
            max_clauses: 1,
            max_literals: 3,
            ..AdmissionLimits::default()
        };
        let mut cnf = Cnf::new(3, vec![], limits).unwrap();
        let mut cursor = Cursor::projected(3);
        let mut short = budget(&control);
        short.limits.max_work = ceiling;
        assert_eq!(
            exclude(&mut cursor, &mut cnf, &[false; 3], &mut short),
            Err(Incomplete::WorkLimit)
        );
        assert_eq!(short.statistics.work, ceiling);
        // A retry must fit the only logical history slot. An unfinished suffix
        // must not reject any of the remaining, independently enumerated keys.
        let mut complete = budget(&control);
        exclude(&mut cursor, &mut cnf, &[false; 3], &mut complete).unwrap();
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
    let mut cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::projected(2);
    cursor.restart();
    assert_eq!(
        exclude(&mut cursor, &mut cnf, &[false; 3], &mut budget(&control)),
        Err(Incomplete::InvalidWitness)
    );
    assert_eq!(
        exclude(&mut cursor, &mut cnf, &[false; 1], &mut budget(&control)),
        Err(Incomplete::InvalidWitness)
    );
}

#[test]
fn distinct_projection_insertion_has_exact_partitioned_work() {
    let control = Control::default();
    for width in 0..=4 {
        let mut cnf = Cnf::new(width, vec![], AdmissionLimits::default()).unwrap();
        let mut cursor = Cursor::projected(width);
        let mut charged = budget(&control);
        for mask in 0..1 << width {
            let values: Vec<_> = (0..width).map(|bit| mask & (1 << bit) != 0).collect();
            let before = charged.statistics.work;
            exclude(&mut cursor, &mut cnf, &values, &mut charged).unwrap();
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
        let mut cnf = Cnf::new(
            3,
            vec![],
            AdmissionLimits {
                max_clauses: 2,
                max_literals: 6,
                ..AdmissionLimits::default()
            },
        )
        .unwrap();
        let mut cursor = Cursor::projected(3);
        exclude(&mut cursor, &mut cnf, &[false; 3], &mut budget(&control)).unwrap();
        let mut short = budget(&control);
        short.limits.max_work = ceiling;
        assert_eq!(
            exclude(&mut cursor, &mut cnf, &[false, true, true], &mut short),
            Err(Incomplete::WorkLimit)
        );
        // Existing exact membership survives; the incomplete suffix does not
        // become a second key. The same trie is read without a search cursor.
        let index = cursor.projections.as_ref().unwrap();
        for mask in 0..8 {
            let assignment = crate::Assignment((0..3).map(|bit| mask & (1 << bit) != 0).collect());
            assert_eq!(
                index.permits(&assignment, &mut budget(&control)).unwrap(),
                mask != 0
            );
        }
        exclude(
            &mut cursor,
            &mut cnf,
            &[false, true, true],
            &mut budget(&control),
        )
        .unwrap();
    }
}
