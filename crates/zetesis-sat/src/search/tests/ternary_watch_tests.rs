//! A ternary replacement inspects exactly the one unwatched occurrence.

use super::State;
use super::budget;
use crate::{AdmissionLimits, Cancellation, Cnf, Incomplete, Literal};

fn ternary(signs: usize) -> Cnf {
    Cnf::new(
        3,
        vec![
            (0..3)
                .map(|variable| Literal::new(variable, signs & (1 << variable) != 0))
                .collect(),
        ],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn ternary_replacement_matches_the_complete_scan() {
    let cancellation = Cancellation::default();
    let mut checked = 0;
    for signs in 0..8 {
        let cnf = ternary(signs);
        let mut charged = budget(&cancellation);
        let mut state = State::new(&cnf, &mut charged).unwrap();
        for first in 0..3 {
            for second in 0..3 {
                if first == second {
                    continue;
                }
                state.positions[0] = [first, second];
                for mut valuation in 0..27 {
                    for value in &mut state.values {
                        *value = [None, Some(false), Some(true)][valuation % 3];
                        valuation /= 3;
                    }
                    // This reference uses explicit occurrence exclusion and
                    // signed literal truth, without computing a complement.
                    let expected = cnf
                        .clause_at(0)
                        .iter()
                        .enumerate()
                        .find(|(position, literal)| {
                            ![first, second].contains(position)
                                && state.values[literal.variable()] != Some(!literal.positive())
                        })
                        .map(|(position, _)| position);
                    for slot in 0..2 {
                        let before = charged.statistics.work;
                        assert_eq!(state.replacement(&cnf, 0, slot, &mut charged), Ok(expected));
                        assert_eq!(charged.statistics.work, before + 1);
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 2_592);
}

#[test]
fn ternary_replacement_requires_one_paid_inspection() {
    let cnf = ternary(7);
    let cancellation = Cancellation::default();
    for remaining in [None, Some(false), Some(true)] {
        let mut charged = budget(&cancellation);
        let mut state = State::new(&cnf, &mut charged).unwrap();
        state.positions[0] = [2, 1];
        state.values = vec![remaining, Some(false), Some(false)];
        let before = charged.statistics;
        charged.limits.max_work = before.work;
        assert_eq!(
            state.replacement(&cnf, 0, 1, &mut charged),
            Err(Incomplete::WorkLimit)
        );
        assert_eq!(charged.statistics, before);
        charged.limits.max_work += 1;
        assert_eq!(
            state.replacement(&cnf, 0, 1, &mut charged),
            Ok((remaining != Some(false)).then_some(0))
        );
        assert_eq!(charged.statistics.work, before.work + 1);
    }
}

#[test]
fn cancellation_precedes_ternary_inspection() {
    let cnf = ternary(7);
    let cancellation = Cancellation::default();
    let mut charged = budget(&cancellation);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    let before = charged.statistics;
    cancellation.cancel();
    assert_eq!(
        state.replacement(&cnf, 0, 0, &mut charged),
        Err(Incomplete::Cancelled)
    );
    assert_eq!(charged.statistics, before);
}

#[test]
fn unpaid_ternary_inspection_cannot_move_a_watch() {
    let cnf = ternary(7);
    let cancellation = Cancellation::default();
    let mut charged = budget(&cancellation);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.assign(Literal::new(0, false)));
    let heads = state.heads.clone();
    let next = state.next.clone();
    // Pay the dequeue and watch visit, but not the remaining literal test.
    charged.limits.max_work = charged.statistics.work + 2;
    assert_eq!(
        state.propagate(&cnf, &mut charged),
        Err(Incomplete::WorkLimit)
    );
    assert_eq!(state.positions, [[0, 1]]);
    assert_eq!(state.heads, heads);
    assert_eq!(state.next, next);
    assert_eq!(state.values, [Some(false), None, None]);
}

#[test]
fn ternary_watch_motion_fits_three_operations() {
    let cnf = ternary(7);
    let cancellation = Cancellation::default();
    let mut charged = budget(&cancellation);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.assign(Literal::new(0, false)));
    let before = charged.statistics.work;
    charged.limits.max_work = before + 3;
    assert_eq!(state.propagate(&cnf, &mut charged), Ok(true));
    assert_eq!(charged.statistics.work, before + 3);
    assert_eq!(state.positions, [[2, 1]]);
    assert_eq!(state.values, [Some(false), None, None]);
}

#[test]
fn longer_clauses_keep_their_complete_replacement_scan() {
    let cnf = Cnf::new(
        4,
        vec![
            (0..4)
                .map(|variable| Literal::new(variable, true))
                .collect(),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let mut charged = budget(&cancellation);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    state.values = vec![Some(false), Some(false), Some(false), None];
    let before = charged.statistics.work;
    assert_eq!(state.replacement(&cnf, 0, 0, &mut charged), Ok(Some(3)));
    assert_eq!(charged.statistics.work, before + 4);
}
