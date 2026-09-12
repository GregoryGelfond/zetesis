//! Root assignments are omitted without losing any undecided variable or tie order.

use crate::search::Budget;
use crate::{Control, Incomplete, Literal, SearchLimits, SearchStatistics};

#[test]
fn partial_assignments_match_independent_unassigned_frequency_order() {
    let control = Control::default();
    for code in 0..3_usize.pow(4) {
        let mut digits = code;
        let values: Vec<_> = (0..4)
            .map(|_| {
                let value = match digits % 3 {
                    0 => None,
                    1 => Some(false),
                    _ => Some(true),
                };
                digits /= 3;
                value
            })
            .collect();
        let clauses = vec![
            vec![Literal::new(3, true), Literal::new(1, false)],
            vec![Literal::new(2, true), Literal::new(1, true)],
            vec![Literal::new(3, false)],
        ];
        let mut expected: Vec<_> = (0..4).filter(|&index| values[index].is_none()).collect();
        expected.sort_by_key(|&variable| {
            let occurrences = clauses
                .iter()
                .filter(|clause| {
                    !clause
                        .iter()
                        .any(|literal| values[literal.variable()] == Some(literal.positive()))
                })
                .flatten()
                .filter(|literal| literal.variable() == variable)
                .count();
            (std::cmp::Reverse(occurrences), variable)
        });
        let cnf = crate::Cnf::new(4, clauses, crate::AdmissionLimits::default()).unwrap();
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: SearchLimits::default(),
            control: &control,
            statistics: SearchStatistics::default(),
        };
        assert_eq!(
            super::variables(cnf.clauses(), &values, &mut budget).unwrap(),
            expected
        );
        let work = budget.statistics.work;
        for ceiling in [0, work - 1, work] {
            let mut budget = Budget {
                quota: crate::search::LocalQuota,
                limits: SearchLimits {
                    max_work: ceiling,
                    ..Default::default()
                },
                control: &control,
                statistics: SearchStatistics::default(),
            };
            let result = super::variables(cnf.clauses(), &values, &mut budget);
            if ceiling == work {
                assert_eq!(result.unwrap(), expected);
            } else {
                assert_eq!(result, Err(Incomplete::WorkLimit));
            }
            assert_eq!(budget.statistics.work, ceiling);
        }
    }
}

#[test]
fn all_root_assigned_variables_need_no_merge_work_and_still_poll_control() {
    let control = Control::default();
    for count in [0, 1, 2, 128] {
        let values = vec![Some(false); count];
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: SearchLimits::default(),
            control: &control,
            statistics: SearchStatistics::default(),
        };
        assert!(
            super::variables(std::iter::empty(), &values, &mut budget)
                .unwrap()
                .is_empty()
        );
        assert_eq!(budget.statistics.work, u64::try_from(count * 2).unwrap());
    }
    let cancelled = Control::default();
    cancelled.cancel();
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: SearchLimits::default(),
        control: &cancelled,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(
        super::variables(std::iter::empty(), &[Some(true)], &mut budget),
        Err(Incomplete::Cancelled)
    );
    assert_eq!(budget.statistics.work, 0);
}
