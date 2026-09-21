//! Reinitialized search preserves logical outcomes and all charged operations.

use super::*;
use crate::AdmissionLimits;

fn outcome(result: Solve) -> Result<Option<Assignment>, Incomplete> {
    match result {
        Solve::Sat(assignment) => Ok(Some(assignment)),
        Solve::Unsat => Ok(None),
        Solve::Inconclusive(error) => Err(error),
    }
}

#[test]
fn reused_search_matches_fresh_state_after_every_interruption() {
    let cancellation = Cancellation::default();
    let mut workspace = Workspace::default();
    workspace.reserve(4, 4).unwrap();
    let retained = workspace.retained_bytes();
    let p = |variable| Literal::new(variable, true);
    for clauses in [
        vec![vec![p(0), p(1)], vec![p(0).negated(), p(2)]],
        vec![vec![]],
        vec![],
        vec![vec![p(1)], vec![p(1).negated()]],
        vec![vec![p(3).negated(), p(0)], vec![p(0).negated(), p(2), p(3)]],
    ] {
        let cnf = Cnf::new(4, clauses, AdmissionLimits::default()).unwrap();
        for max_work in 0..=160 {
            let limits = SearchLimits {
                max_work,
                ..Default::default()
            };
            let mut fresh = Budget {
                quota: LocalQuota,
                limits,
                cancellation: &cancellation,
                statistics: SearchStatistics::default(),
            };
            let mut reused = Budget {
                quota: LocalQuota,
                limits,
                cancellation: &cancellation,
                statistics: SearchStatistics::default(),
            };
            assert_eq!(
                outcome(workspace.query(&cnf, &mut reused)),
                outcome(query(&cnf, &mut fresh))
            );
            assert_eq!(reused.statistics, fresh.statistics);
            assert_eq!(workspace.retained_bytes(), retained);
        }
    }
}

#[test]
fn search_keeps_reserved_arrays_across_smaller_queries() {
    let cancellation = Cancellation::default();
    let mut workspace = Workspace::default();
    workspace.reserve(5, 4).unwrap();
    let pointers = (
        workspace.0.values.as_ptr(),
        workspace.0.trail.as_ptr(),
        workspace.0.decisions.as_ptr(),
        workspace.0.positions.as_ptr(),
        workspace.0.heads.as_ptr(),
        workspace.0.next.as_ptr(),
    );
    for variables in [5, 0, 2, 1, 4, 5] {
        let cnf = Cnf::new(variables, vec![], AdmissionLimits::default()).unwrap();
        let mut budget = Budget {
            quota: LocalQuota,
            limits: SearchLimits::default(),
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let answer = outcome(workspace.query(&cnf, &mut budget))
            .unwrap()
            .unwrap();
        assert_eq!(answer.variables(), variables);
        assert!((0..variables).all(|variable| answer.value(variable) == Some(false)));
        assert_eq!(
            (
                workspace.0.values.as_ptr(),
                workspace.0.trail.as_ptr(),
                workspace.0.decisions.as_ptr(),
                workspace.0.positions.as_ptr(),
                workspace.0.heads.as_ptr(),
                workspace.0.next.as_ptr()
            ),
            pointers
        );
    }
}

#[test]
fn changing_level_zero_parameters_matches_explicit_unit_queries() {
    let cancellation = Cancellation::default();
    let mut workspace = Workspace::default();
    let p = |variable| Literal::new(variable, true);
    let base = vec![
        vec![p(0), p(1)],
        vec![p(1).negated(), p(2)],
        vec![p(0).negated(), p(2).negated()],
    ];
    let cnf = Cnf::new(3, base.clone(), AdmissionLimits::default()).unwrap();
    for assumptions in [
        vec![p(2)],
        vec![p(2).negated()],
        vec![p(0), p(0).negated()],
        vec![p(0).negated()],
        vec![],
    ] {
        let mut clauses = base.clone();
        clauses.extend(assumptions.iter().map(|&literal| vec![literal]));
        let reference = Cnf::new(3, clauses, AdmissionLimits::default()).unwrap();
        let expected = solve(&reference, SearchLimits::default(), &cancellation);
        let mut budget = Budget {
            quota: LocalQuota,
            limits: SearchLimits::default(),
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let actual = workspace.query_assuming(&cnf, &assumptions, &mut budget);
        match (actual, expected) {
            (Solve::Sat(assignment), Solve::Sat(_)) => {
                assert!(assumptions.iter().all(
                    |literal| assignment.value(literal.variable()) == Some(literal.positive())
                ));
                assert!(base.iter().all(|clause| clause.iter().any(|literal| {
                    assignment.value(literal.variable()) == Some(literal.positive())
                })));
            }
            (Solve::Unsat, Solve::Unsat) => (),
            mismatch => panic!("parameter/unit mismatch: {mismatch:?}"),
        }
    }
}
