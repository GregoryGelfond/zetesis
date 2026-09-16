//! Exact search and reservation boundaries are independent of quota ownership.

use crate::search::{Budget, LocalQuota, SharedBudget, query};
use crate::{
    AdmissionLimits, Cnf, Control, Incomplete, Literal, SearchLimits, SearchStatistics, Solve,
};

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Sat(Vec<bool>),
    Unsat,
    Inconclusive(Incomplete),
}

fn outcome(result: Solve) -> Outcome {
    match result {
        Solve::Sat(assignment) => Outcome::Sat(assignment.0),
        Solve::Unsat => Outcome::Unsat,
        Solve::Inconclusive(error) => Outcome::Inconclusive(error),
    }
}

fn compare_query(cnf: &Cnf, limits: SearchLimits, spent: SearchStatistics) -> SearchStatistics {
    let control = Control::default();
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        control: &control,
        statistics: spent,
    };
    let expected = outcome(query(cnf, &mut local));
    let shared = SharedBudget::new(limits, spent);
    let mut worker = Budget {
        quota: shared.lease(&control),
        limits,
        control: &control,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(outcome(query(cnf, &mut worker)), expected);
    assert_eq!(worker.statistics.work, local.statistics.work - spent.work);
    assert_eq!(
        worker.statistics.decisions,
        local.statistics.decisions - spent.decisions
    );
    let mut joined = worker.statistics;
    drop(worker.quota);
    shared.record(&mut joined);
    joined.propagations += spent.propagations;
    joined.conflicts += spent.conflicts;
    assert_eq!(joined, local.statistics);
    if let Outcome::Sat(values) = expected {
        assert_eq!(values.len(), cnf.variables());
        assert!(cnf.clauses().all(|clause| {
            clause
                .iter()
                .any(|literal| values[literal.variable()] == literal.positive())
        }));
    }
    local.statistics
}

fn clauses() -> Vec<Vec<Literal>> {
    (0..9)
        .map(|mut code| {
            (0..2)
                .filter_map(|variable| {
                    let digit = code % 3;
                    code /= 3;
                    (digit != 0).then(|| Literal::new(variable, digit == 2))
                })
                .collect()
        })
        .collect()
}

#[test]
fn local_and_single_job_shared_queries_agree_at_every_small_work_and_decision_ceiling() {
    let spent = SearchStatistics {
        work: 7,
        decisions: 3,
        propagations: 5,
        conflicts: 11,
    };
    let possible = clauses();
    let mut checked = 0;
    for first in &possible {
        for second in &possible {
            let cnf = Cnf::new(
                2,
                vec![first.clone(), second.clone()],
                AdmissionLimits::default(),
            )
            .unwrap();
            let complete = compare_query(&cnf, SearchLimits::default(), spent);
            for max_work in spent.work..=complete.work + 1 {
                for max_decisions in spent.decisions..=complete.decisions + 1 {
                    let stats = compare_query(
                        &cnf,
                        SearchLimits {
                            max_work,
                            max_decisions,
                        },
                        spent,
                    );
                    assert!(stats.work <= max_work);
                    assert!(stats.decisions <= max_decisions);
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 81);
}

#[test]
fn failed_decision_reserves_work_first_and_cancellation_precedes_either_quota() {
    let control = Control::default();
    let limits = SearchLimits {
        max_work: 2,
        max_decisions: 0,
    };
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        control: &control,
        statistics: SearchStatistics::default(),
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut worker = Budget {
        quota: shared.lease(&control),
        limits,
        control: &control,
        statistics: SearchStatistics::default(),
    };
    for remaining in [false, true] {
        assert_eq!(local.decide(), Err(Incomplete::DecisionLimit));
        assert_eq!(worker.decide(), Err(Incomplete::DecisionLimit));
        assert_eq!(local.statistics, worker.statistics);
        assert_eq!(local.statistics.work, u64::from(remaining) + 1);
        assert_eq!(local.statistics.decisions, 0);
    }
    assert_eq!(local.decide(), Err(Incomplete::WorkLimit));
    assert_eq!(worker.decide(), Err(Incomplete::WorkLimit));
    control.cancel();
    assert_eq!(local.tick(), Err(Incomplete::Cancelled));
    assert_eq!(worker.tick(), Err(Incomplete::Cancelled));
    assert_eq!(local.decide(), Err(Incomplete::Cancelled));
    assert_eq!(worker.decide(), Err(Incomplete::Cancelled));
    let mut joined = SearchStatistics::default();
    drop(worker.quota);
    shared.record(&mut joined);
    assert_eq!(joined, local.statistics);
    assert_eq!(joined, worker.statistics);
    assert_eq!(joined.work, 2);
    assert_eq!(joined.decisions, 0);
}

#[test]
fn cancellation_precedes_an_expired_deadline_and_exhausted_quotas() {
    let control = Control::with_deadline(std::time::Instant::now()).unwrap();
    let limits = SearchLimits {
        max_work: 0,
        max_decisions: 0,
    };
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        control: &control,
        statistics: SearchStatistics::default(),
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut worker = Budget {
        quota: shared.lease(&control),
        limits,
        control: &control,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(local.tick(), Err(Incomplete::Deadline));
    assert_eq!(worker.tick(), Err(Incomplete::Deadline));
    control.clone().cancel();
    assert_eq!(local.decide(), Err(Incomplete::Cancelled));
    assert_eq!(worker.decide(), Err(Incomplete::Cancelled));
    assert_eq!(local.statistics, SearchStatistics::default());
    assert_eq!(worker.statistics, local.statistics);
    drop(worker.quota);
    let mut joined = SearchStatistics::default();
    shared.record(&mut joined);
    assert_eq!(joined, SearchStatistics::default());
}

#[test]
fn the_last_representable_local_or_shared_charge_never_wraps() {
    let control = Control::default();
    let limits = SearchLimits {
        max_work: u64::MAX,
        max_decisions: u64::MAX,
    };
    let spent = SearchStatistics {
        work: u64::MAX - 1,
        decisions: u64::MAX - 1,
        ..SearchStatistics::default()
    };
    let shared = SharedBudget::new(limits, spent);
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        control: &control,
        statistics: spent,
    };
    let mut worker = Budget {
        quota: shared.lease(&control),
        limits,
        control: &control,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(local.decide(), Ok(()));
    assert_eq!(worker.decide(), Ok(()));
    assert_eq!(worker.statistics.work, 1);
    assert_eq!(worker.statistics.decisions, 1);
    assert_eq!(local.decide(), Err(Incomplete::WorkLimit));
    assert_eq!(worker.decide(), Err(Incomplete::WorkLimit));
    let mut joined = worker.statistics;
    drop(worker.quota);
    shared.record(&mut joined);
    assert_eq!(joined, local.statistics);
    assert_eq!(joined.work, u64::MAX);
    assert_eq!(joined.decisions, u64::MAX);
}

#[test]
fn frozen_encoding_limits_and_clauses_match_before_search() {
    use zetesis_ferraris::{Interpretation, Node, Theory};

    let theory = Theory::new(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::False,
            Node::Implies(0, 2),
            Node::Or(0, 3),
            Node::Implies(1, 0),
            Node::And(4, 5),
        ],
        vec![6],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let control = Control::default();
    for bits in 0..4 {
        let candidate =
            Interpretation::new(&theory, (0..2).filter(|atom| bits & (1 << atom) != 0)).unwrap();
        for frozen in [None, Some(&candidate)] {
            for max_work in 0..=128 {
                let limits = SearchLimits {
                    max_work,
                    max_decisions: 0,
                };
                let mut local = Budget {
                    quota: LocalQuota,
                    limits,
                    control: &control,
                    statistics: SearchStatistics::default(),
                };
                let shared = SharedBudget::new(limits, SearchStatistics::default());
                let mut worker = Budget {
                    quota: shared.lease(&control),
                    limits,
                    control: &control,
                    statistics: SearchStatistics::default(),
                };
                let expected = crate::encoding::encode(
                    &theory,
                    frozen,
                    AdmissionLimits::default(),
                    &mut local,
                );
                let actual = crate::encoding::encode(
                    &theory,
                    frozen,
                    AdmissionLimits::default(),
                    &mut worker,
                );
                match (expected, actual) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(a.variables(), b.variables());
                        assert!(a.clauses().eq(b.clauses()));
                    }
                    (Err(a), Err(b)) => assert_eq!(a, b),
                    pair => panic!("different quota result at {bits}/{max_work}: {pair:?}"),
                }
                assert_eq!(local.statistics, worker.statistics);
                let mut joined = SearchStatistics::default();
                drop(worker.quota);
                shared.record(&mut joined);
                assert_eq!(joined, local.statistics);
            }
        }
    }
}
