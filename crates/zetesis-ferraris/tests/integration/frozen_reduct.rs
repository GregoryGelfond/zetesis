//! Reusing one subject-bound freeze preserves arbitrary-interpretation semantics.

use std::time::Instant;

use crate::support::worlds::interpretation;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{AdmissionLimits, FrozenReduct, Limits, Node, Theory, models_reduct};

fn theory(nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn work_limit(max_work: u64) -> Limits {
    Limits {
        max_work,
        max_subsets: 0,
    }
}

#[test]
fn frozen_subject_is_the_original_borrow() {
    let theory = theory(vec![Node::atom(0)], vec![0]);
    let candidate = interpretation(&theory, 1);
    let frozen = FrozenReduct::new(&candidate, work_limit(1), &Cancellation::default()).unwrap();
    assert!(std::ptr::eq(frozen.candidate(), &raw const candidate));
    assert!(std::ptr::eq(frozen.theory(), candidate.theory()));
    let same_theory = interpretation(&theory.clone(), 1);
    assert!(
        frozen
            .is_satisfied_by(&same_theory, work_limit(2), &Cancellation::default())
            .unwrap()
    );
}

#[test]
fn freezing_does_not_require_original_satisfaction() {
    let theory = theory(vec![Node::atom(0)], vec![0]);
    let candidate = interpretation(&theory, 0);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(1), &cancellation).unwrap();
    for world in 0..4 {
        assert!(
            !frozen
                .is_satisfied_by(
                    &interpretation(&theory, world),
                    work_limit(2),
                    &cancellation
                )
                .unwrap()
        );
    }
}

#[test]
fn tested_interpretations_need_not_be_subsets() {
    let theory = theory(
        vec![Node::atom(0), Node::atom(1), Node::implies(0, 1)],
        vec![2],
    );
    let candidate = interpretation(&theory, 2);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(5), &cancellation).unwrap();
    // The candidate-false antecedent is falsum even when atom 0 belongs to J.
    for world in [1, 3, 0, 2, 1] {
        let tested = interpretation(&theory, world);
        assert!(
            frozen
                .is_satisfied_by(&tested, work_limit(6), &cancellation)
                .unwrap()
        );
        assert!(
            models_reduct(&theory, &candidate, &tested, work_limit(11), &cancellation).unwrap()
        );
    }
}

#[test]
fn distinct_candidates_keep_distinct_reducts() {
    let theory = theory(
        vec![Node::atom(0), Node::atom(1), Node::implies(0, 1)],
        vec![2],
    );
    let cancellation = Cancellation::default();
    let guarded = interpretation(&theory, 3);
    let vacuous = interpretation(&theory, 2);
    let guarded = FrozenReduct::new(&guarded, work_limit(5), &cancellation).unwrap();
    let vacuous = FrozenReduct::new(&vacuous, work_limit(5), &cancellation).unwrap();
    let tested = interpretation(&theory, 1);
    assert!(
        !guarded
            .is_satisfied_by(&tested, work_limit(6), &cancellation)
            .unwrap()
    );
    assert!(
        vacuous
            .is_satisfied_by(&tested, work_limit(6), &cancellation)
            .unwrap()
    );
    assert!(
        !guarded
            .is_satisfied_by(&tested, work_limit(6), &cancellation)
            .unwrap()
    );
}

#[test]
fn independent_queries_can_share_one_frozen_value() {
    let theory = theory(
        vec![Node::atom(0), Node::atom(1), Node::implies(0, 1)],
        vec![2],
    );
    let candidate = interpretation(&theory, 3);
    let false_tested = interpretation(&theory, 1);
    let true_tested = interpretation(&theory, 2);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(5), &cancellation).unwrap();
    std::thread::scope(|scope| {
        let false_query =
            scope.spawn(|| frozen.is_satisfied_by(&false_tested, work_limit(6), &cancellation));
        let true_query =
            scope.spawn(|| frozen.is_satisfied_by(&true_tested, work_limit(6), &cancellation));
        assert!(!false_query.join().unwrap().unwrap());
        assert!(true_query.join().unwrap().unwrap());
    });
}

#[test]
fn nested_negation_retains_candidate_truth() {
    let theory = theory(
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::implies(2, 1),
            Node::implies(3, 0),
        ],
        vec![4],
    );
    let candidate = interpretation(&theory, 1);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(11), &cancellation).unwrap();
    // In (not not a -> a)^M, double negation is true for every tested J.
    for world in 0..4 {
        assert_eq!(
            frozen
                .is_satisfied_by(
                    &interpretation(&theory, world),
                    work_limit(12),
                    &cancellation
                )
                .unwrap(),
            world & 1 != 0
        );
    }
}

#[test]
fn constraints_keep_their_frozen_meaning() {
    let theory = theory(
        vec![Node::atom(0), Node::falsum(), Node::implies(0, 1)],
        vec![2],
    );
    let cancellation = Cancellation::default();
    for candidate in 0..4 {
        let subject = interpretation(&theory, candidate);
        let frozen = FrozenReduct::new(&subject, work_limit(5), &cancellation).unwrap();
        for tested in 0..4 {
            assert_eq!(
                frozen
                    .is_satisfied_by(
                        &interpretation(&theory, tested),
                        work_limit(6),
                        &cancellation
                    )
                    .unwrap(),
                candidate & 1 == 0
            );
        }
    }
}

#[test]
fn empty_theories_need_no_charged_work() {
    let theory = theory(vec![], vec![]);
    let candidate = interpretation(&theory, 3);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(0), &cancellation).unwrap();
    for world in 0..4 {
        assert!(
            frozen
                .is_satisfied_by(
                    &interpretation(&theory, world),
                    work_limit(0),
                    &cancellation
                )
                .unwrap()
        );
    }
}

#[test]
fn freeze_work_ceiling_is_inclusive() {
    let theory = theory(
        vec![Node::atom(0), Node::atom(1), Node::and_pair([0, 1])],
        vec![2],
    );
    let candidate = interpretation(&theory, 3);
    let cancellation = Cancellation::default();
    for limit in 0..5 {
        assert_eq!(
            FrozenReduct::new(&candidate, work_limit(limit), &cancellation).unwrap_err(),
            Stop::WorkLimit
        );
    }
    let frozen = FrozenReduct::new(&candidate, work_limit(5), &cancellation).unwrap();
    assert!(
        frozen
            .is_satisfied_by(&candidate, work_limit(6), &cancellation)
            .unwrap()
    );
}

#[test]
fn satisfaction_does_not_recharge_the_freeze() {
    let theory = theory(
        vec![Node::atom(0), Node::atom(1), Node::and_pair([0, 1])],
        vec![2],
    );
    let candidate = interpretation(&theory, 3);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(5), &cancellation).unwrap();
    for limit in 0..6 {
        assert_eq!(
            frozen
                .is_satisfied_by(&candidate, work_limit(limit), &cancellation)
                .unwrap_err(),
            Stop::WorkLimit
        );
    }
    for _ in 0..3 {
        assert!(
            frozen
                .is_satisfied_by(&candidate, work_limit(6), &cancellation)
                .unwrap()
        );
    }
    assert_eq!(
        models_reduct(
            &theory,
            &candidate,
            &candidate,
            work_limit(10),
            &cancellation
        )
        .unwrap_err(),
        Stop::WorkLimit
    );
    assert!(
        models_reduct(
            &theory,
            &candidate,
            &candidate,
            work_limit(11),
            &cancellation
        )
        .unwrap()
    );
}

#[test]
fn roots_are_charged_until_the_first_failure() {
    let theory = theory(
        vec![Node::atom(0), Node::falsum(), Node::implies(1, 1)],
        vec![2, 1, 0],
    );
    let candidate = interpretation(&theory, 1);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(5), &cancellation).unwrap();
    assert_eq!(
        frozen
            .is_satisfied_by(&candidate, work_limit(6), &cancellation)
            .unwrap_err(),
        Stop::WorkLimit
    );
    assert!(
        !frozen
            .is_satisfied_by(&candidate, work_limit(7), &cancellation)
            .unwrap()
    );
}

#[test]
fn foreign_tested_identity_precedes_control() {
    let own = theory(vec![Node::atom(0)], vec![0]);
    let foreign = theory(vec![Node::atom(0)], vec![0]);
    let candidate = interpretation(&own, 1);
    let tested = interpretation(&foreign, 1);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(1), &cancellation).unwrap();
    cancellation.cancel();
    assert_eq!(
        frozen
            .is_satisfied_by(&tested, work_limit(0), &cancellation)
            .unwrap_err(),
        Stop::WrongProgram
    );
}

#[test]
fn wrapper_identity_precedes_control() {
    let own = theory(vec![], vec![]);
    let foreign = theory(vec![], vec![]);
    let candidate = interpretation(&own, 0);
    let other = interpretation(&foreign, 0);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for (candidate, tested) in [(&other, &candidate), (&candidate, &other)] {
        assert_eq!(
            models_reduct(&own, candidate, tested, work_limit(0), &cancellation).unwrap_err(),
            Stop::WrongProgram
        );
    }
}

#[test]
fn stopped_queries_leave_the_reduct_reusable() {
    let theory = theory(vec![Node::atom(0)], vec![0]);
    let candidate = interpretation(&theory, 1);
    let cancellation = Cancellation::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(1), &cancellation).unwrap();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (stopped, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        assert_eq!(
            frozen
                .is_satisfied_by(&candidate, work_limit(0), &stopped)
                .unwrap_err(),
            expected
        );
        assert!(
            frozen
                .is_satisfied_by(&candidate, work_limit(2), &cancellation)
                .unwrap()
        );
    }
}

#[test]
fn empty_freezes_still_poll_control() {
    let theory = theory(vec![], vec![]);
    let candidate = interpretation(&theory, 0);
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (cancellation, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        assert_eq!(
            FrozenReduct::new(&candidate, work_limit(0), &cancellation).unwrap_err(),
            expected
        );
    }
}

#[test]
fn empty_satisfaction_still_polls_control() {
    let theory = theory(vec![], vec![]);
    let candidate = interpretation(&theory, 0);
    let frozen = FrozenReduct::new(&candidate, work_limit(0), &Cancellation::default()).unwrap();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (cancellation, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        assert_eq!(
            frozen
                .is_satisfied_by(&candidate, work_limit(0), &cancellation)
                .unwrap_err(),
            expected
        );
    }
}
