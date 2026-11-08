//! Ordered applications, partial late progress and attempted-receipt precedence.

use std::cell::RefCell;

use super::*;
use crate::budget::LocalQuota;
use crate::{Cancellation, SearchLimits, SearchStatistics};
use zetesis_algebra::AnswerSetsSought;

#[derive(Clone, Copy)]
enum Action {
    Unchanged,
    Hold { atom: usize, after: Option<usize> },
    Refuted,
    Fail,
}

struct Script<'a> {
    ordinal: usize,
    action: Action,
    trace: &'a RefCell<Vec<(usize, Pass)>>,
}

impl Subject<AnswerSetsSought> for Script<'_> {
    type Knowledge = ();

    fn narrow<Q: Quota>(
        &mut self,
        region: &mut Region,
        _knowledge: &mut (),
        _scratch: &mut NarrowingScratch,
        pass: Pass,
        context: &mut SubjectContext<'_, '_, Q>,
    ) -> SubjectAttempt {
        self.trace.borrow_mut().push((self.ordinal, pass));
        if let Err(error) = context.budget.tick() {
            return SubjectAttempt::failed(error);
        }
        let mut counts = RegionCounts {
            work: 1,
            ..RegionCounts::default()
        };
        let result = match self.action {
            Action::Unchanged => Ok(SubjectStep::Unchanged),
            Action::Hold { atom, after } => {
                if after.is_none_or(|other| region.is_held(other)) && region.is_open(atom) {
                    assert!(region.hold(atom));
                    counts.held = 1;
                    counts.propagations = 1;
                    Ok(SubjectStep::Changed)
                } else {
                    Ok(SubjectStep::Unchanged)
                }
            }
            Action::Refuted => Ok(SubjectStep::Refuted),
            Action::Fail => Err(Incomplete::InvalidWitness),
        };
        SubjectAttempt { result, counts }
    }
}

struct Observation {
    result: Result<Narrowing, Incomplete>,
    region: Region,
    counts: RegionCounts,
    trace: Vec<(usize, Pass)>,
    search: SearchStatistics,
}

fn run(eager_actions: &[Action], late_actions: &[Action], counts: RegionCounts) -> Observation {
    run_optional(
        eager_actions,
        &late_actions.iter().copied().map(Some).collect::<Vec<_>>(),
        counts,
    )
}

fn run_optional(
    eager_actions: &[Action],
    late_actions: &[Option<Action>],
    mut counts: RegionCounts,
) -> Observation {
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let trace = RefCell::new(Vec::new());
    let eager = || {
        eager_actions
            .iter()
            .enumerate()
            .map(|(ordinal, &action)| Script {
                ordinal,
                action,
                trace: &trace,
            })
    };
    let mut late: Vec<_> = late_actions
        .iter()
        .enumerate()
        .map(|(ordinal, action)| {
            action.map(|action| Script {
                ordinal: eager_actions.len() + ordinal,
                action,
                trace: &trace,
            })
        })
        .collect();
    let mut region = Region::all_open(3);
    let result = joint(
        eager,
        &mut late,
        &mut region,
        &mut (),
        &mut NarrowingScratch::default(),
        &mut budget,
        &mut counts,
    );
    drop(late);
    Observation {
        result,
        region,
        counts,
        trace: trace.into_inner(),
        search: budget.statistics,
    }
}

#[test]
fn a_late_change_reenters_eager_before_the_next_late_subject() {
    let observed = run(
        &[Action::Hold {
            atom: 0,
            after: Some(1),
        }],
        &[
            Action::Hold {
                atom: 1,
                after: None,
            },
            Action::Unchanged,
        ],
        RegionCounts::default(),
    );
    assert_eq!(observed.result, Ok(Narrowing::Fixed { changed: true }));
    assert_eq!(
        observed.trace,
        [
            (0, Pass::First),
            (1, Pass::First),
            (0, Pass::Continue),
            (1, Pass::Continue),
            (2, Pass::Continue),
        ]
    );
    assert!(observed.region.is_held(0) && observed.region.is_held(1));
    assert!(observed.region.is_open(2));
    assert_eq!(observed.counts.work, 5);
    assert_eq!(observed.counts.held, 2);
    assert_eq!(observed.counts.propagations, 2);
    assert_eq!(observed.search.work, 5);
    assert_eq!(observed.search.decisions, 0);
}

#[test]
fn a_later_eager_change_repeats_the_declared_order() {
    let observed = run(
        &[
            Action::Hold {
                atom: 0,
                after: Some(1),
            },
            Action::Hold {
                atom: 1,
                after: None,
            },
        ],
        &[Action::Unchanged],
        RegionCounts::default(),
    );
    assert_eq!(observed.result, Ok(Narrowing::Fixed { changed: true }));
    assert_eq!(
        observed.trace,
        [
            (0, Pass::First),
            (1, Pass::First),
            (0, Pass::First),
            (1, Pass::First),
            (2, Pass::First),
        ]
    );
    assert_eq!(observed.counts.work, 5);
    assert_eq!(observed.counts.held, 2);
    assert_eq!(observed.search.work, 5);
}

#[test]
fn refutation_accounts_its_prefix_and_absorbs_remaining_subjects() {
    for (eager, late) in [
        (vec![Action::Refuted, Action::Fail], vec![Action::Fail]),
        (vec![Action::Unchanged], vec![Action::Refuted, Action::Fail]),
    ] {
        let observed = run(&eager, &late, RegionCounts::default());
        assert_eq!(observed.result, Ok(Narrowing::Refuted));
        let expected = 1 + u64::from(matches!(eager[0], Action::Unchanged));
        assert_eq!(observed.counts.work, expected);
        assert_eq!(observed.search.work, expected);
        assert_eq!(observed.trace.len(), usize::try_from(expected).unwrap());
        assert_eq!(observed.region.open().collect::<Vec<_>>(), [0, 1, 2]);
    }
}

#[test]
fn failed_attempt_receipts_precede_the_typed_error() {
    for initial in [0, u64::MAX] {
        let counts = RegionCounts {
            work: initial,
            ..RegionCounts::default()
        };
        let observed = run(&[Action::Fail, Action::Unchanged], &[], counts);
        assert_eq!(
            observed.result,
            Err(if initial == 0 {
                Incomplete::InvalidWitness
            } else {
                Incomplete::CounterOverflow
            })
        );
        assert_eq!(observed.counts.work, if initial == 0 { 1 } else { initial });
        assert_eq!(observed.search.work, 1);
        assert_eq!(observed.trace, [(0, Pass::First)]);
        assert_eq!(observed.region.open().collect::<Vec<_>>(), [0, 1, 2]);
    }
}

#[test]
fn absent_late_slots_do_no_work() {
    let observed = run_optional(
        &[Action::Unchanged],
        &[None, Some(Action::Unchanged), None],
        RegionCounts::default(),
    );
    assert_eq!(observed.result, Ok(Narrowing::Fixed { changed: false }));
    assert_eq!(observed.trace, [(0, Pass::First), (2, Pass::First)]);
    assert_eq!(observed.counts.work, 2);
    assert_eq!(observed.search.work, 2);
    assert_eq!(observed.region.open().collect::<Vec<_>>(), [0, 1, 2]);
}
