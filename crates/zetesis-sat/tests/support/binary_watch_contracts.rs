//! Binary propagation has no unwatched positions; limits still guard every
//! dequeued occurrence and every visited clause before publishing deductions.

use super::{Budget, LocalQuota, State};
use crate::{AdmissionLimits, Cnf, Control, Incomplete, Literal, SearchLimits, SearchStatistics};

fn budget(control: &Control) -> Budget<'_> {
    Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        control,
        statistics: SearchStatistics::default(),
    }
}

fn binary() -> Cnf {
    Cnf::new(
        2,
        vec![vec![Literal::new(0, true), Literal::new(1, true)]],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn binary_propagation_fits_its_exact_work_ceiling() {
    let cnf = binary();
    let control = Control::default();
    let mut charged = budget(&control);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.assign(Literal::new(0, false)));
    let before = charged.statistics.work;
    // Two dequeued occurrences and one watch visit; no replacement position.
    charged.limits.max_work = before + 3;
    assert_eq!(state.propagate(&cnf, &mut charged), Ok(true));
    assert_eq!(state.values, [Some(false), Some(true)]);
    assert_eq!(state.propagation_head, 2);
    assert_eq!(charged.statistics.work, before + 3);
    assert_eq!(charged.statistics.propagations, 1);
}

#[test]
fn binary_propagation_stops_before_an_unpaid_dequeue() {
    let cnf = binary();
    let control = Control::default();
    let mut charged = budget(&control);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.assign(Literal::new(0, false)));
    let before = charged.statistics.work;
    charged.limits.max_work = before + 2;
    assert_eq!(
        state.propagate(&cnf, &mut charged),
        Err(Incomplete::WorkLimit)
    );
    // The paid watch visit can assign the second variable, but its queue entry
    // remains pending. A search must report incompleteness, not accept a leaf.
    assert_eq!(state.values, [Some(false), Some(true)]);
    assert_eq!(state.propagation_head, 1);
    assert_eq!(charged.statistics.work, before + 2);
}

#[test]
fn an_unpaid_binary_watch_cannot_assign_its_other_literal() {
    let cnf = binary();
    let control = Control::default();
    let mut charged = budget(&control);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.assign(Literal::new(0, false)));
    charged.limits.max_work = charged.statistics.work + 1;
    assert_eq!(
        state.propagate(&cnf, &mut charged),
        Err(Incomplete::WorkLimit)
    );
    assert_eq!(state.values, [Some(false), None]);
    assert_eq!(charged.statistics.propagations, 0);
}

#[test]
fn cancellation_precedes_the_binary_fast_path() {
    let cnf = binary();
    let control = Control::default();
    let mut charged = budget(&control);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.assign(Literal::new(0, false)));
    let before = charged.statistics;
    control.cancel();
    assert_eq!(
        state.propagate(&cnf, &mut charged),
        Err(Incomplete::Cancelled)
    );
    assert_eq!(charged.statistics, before);
    assert_eq!(state.values, [Some(false), None]);
    assert_eq!(state.propagation_head, 0);
}

#[test]
fn a_false_binary_clause_conflicts_at_its_watch_visit() {
    let cnf = binary();
    let control = Control::default();
    let mut charged = budget(&control);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.assign(Literal::new(0, false)));
    assert!(state.assign(Literal::new(1, false)));
    let before = charged.statistics.work;
    charged.limits.max_work = before + 2;
    assert_eq!(state.propagate(&cnf, &mut charged), Ok(false));
    assert_eq!(charged.statistics.work, before + 2);
    assert_eq!(state.values, [Some(false), Some(false)]);
}

#[test]
fn a_third_literal_remains_a_replacement_candidate() {
    let cnf = Cnf::new(
        3,
        vec![(0..3).map(|atom| Literal::new(atom, true)).collect()],
        AdmissionLimits::default(),
    )
    .unwrap();
    let control = Control::default();
    let mut charged = budget(&control);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.assign(Literal::new(0, false)));
    assert!(state.assign(Literal::new(1, false)));
    assert_eq!(state.propagate(&cnf, &mut charged), Ok(true));
    assert_eq!(state.values, [Some(false), Some(false), Some(true)]);
    assert_eq!(state.positions, [[2, 1]]);
}
