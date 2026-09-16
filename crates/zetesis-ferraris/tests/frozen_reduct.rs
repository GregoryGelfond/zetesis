//! Reusing one subject-bound freeze preserves arbitrary-interpretation semantics.

use std::time::Instant;

use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{
    AdmissionLimits, FrozenReduct, Interpretation, Limits, Node, Theory, models_reduct,
};

fn theory(nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(2, nodes, roots, AdmissionLimits::default()).unwrap()
}

fn interpretation(theory: &Theory, world: u8) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| world & (1 << atom) != 0)).unwrap()
}

fn work_limit(max_work: u64) -> Limits {
    Limits {
        max_work,
        max_subsets: 0,
    }
}

#[test]
fn frozen_subject_is_the_original_borrow() {
    let theory = theory(vec![Node::Atom(0)], vec![0]);
    let candidate = interpretation(&theory, 1);
    let frozen = FrozenReduct::new(&candidate, work_limit(1), &Control::default()).unwrap();
    assert!(std::ptr::eq(frozen.candidate(), &raw const candidate));
    assert!(std::ptr::eq(frozen.theory(), candidate.theory()));
    let same_theory = interpretation(&theory.clone(), 1);
    assert!(
        frozen
            .is_satisfied_by(&same_theory, work_limit(2), &Control::default())
            .unwrap()
    );
}

#[test]
fn freezing_does_not_require_original_satisfaction() {
    let theory = theory(vec![Node::Atom(0)], vec![0]);
    let candidate = interpretation(&theory, 0);
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(1), &control).unwrap();
    for world in 0..4 {
        assert!(
            !frozen
                .is_satisfied_by(&interpretation(&theory, world), work_limit(2), &control)
                .unwrap()
        );
    }
}

#[test]
fn tested_interpretations_need_not_be_subsets() {
    let theory = theory(
        vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1)],
        vec![2],
    );
    let candidate = interpretation(&theory, 2);
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(3), &control).unwrap();
    // The candidate-false antecedent is falsum even when atom 0 belongs to J.
    for world in [1, 3, 0, 2, 1] {
        let tested = interpretation(&theory, world);
        assert!(
            frozen
                .is_satisfied_by(&tested, work_limit(4), &control)
                .unwrap()
        );
        assert!(models_reduct(&theory, &candidate, &tested, work_limit(7), &control).unwrap());
    }
}

#[test]
fn distinct_candidates_keep_distinct_reducts() {
    let theory = theory(
        vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1)],
        vec![2],
    );
    let control = Control::default();
    let guarded = interpretation(&theory, 3);
    let vacuous = interpretation(&theory, 2);
    let guarded = FrozenReduct::new(&guarded, work_limit(3), &control).unwrap();
    let vacuous = FrozenReduct::new(&vacuous, work_limit(3), &control).unwrap();
    let tested = interpretation(&theory, 1);
    assert!(
        !guarded
            .is_satisfied_by(&tested, work_limit(4), &control)
            .unwrap()
    );
    assert!(
        vacuous
            .is_satisfied_by(&tested, work_limit(4), &control)
            .unwrap()
    );
    assert!(
        !guarded
            .is_satisfied_by(&tested, work_limit(4), &control)
            .unwrap()
    );
}

#[test]
fn independent_queries_can_share_one_frozen_value() {
    let theory = theory(
        vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1)],
        vec![2],
    );
    let candidate = interpretation(&theory, 3);
    let false_tested = interpretation(&theory, 1);
    let true_tested = interpretation(&theory, 2);
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(3), &control).unwrap();
    std::thread::scope(|scope| {
        let false_query =
            scope.spawn(|| frozen.is_satisfied_by(&false_tested, work_limit(4), &control));
        let true_query =
            scope.spawn(|| frozen.is_satisfied_by(&true_tested, work_limit(4), &control));
        assert!(!false_query.join().unwrap().unwrap());
        assert!(true_query.join().unwrap().unwrap());
    });
}

#[test]
fn nested_negation_retains_candidate_truth() {
    let theory = theory(
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Implies(2, 1),
            Node::Implies(3, 0),
        ],
        vec![4],
    );
    let candidate = interpretation(&theory, 1);
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(5), &control).unwrap();
    // In (not not a -> a)^M, double negation is true for every tested J.
    for world in 0..4 {
        assert_eq!(
            frozen
                .is_satisfied_by(&interpretation(&theory, world), work_limit(6), &control)
                .unwrap(),
            world & 1 != 0
        );
    }
}

#[test]
fn constraints_keep_their_frozen_meaning() {
    let theory = theory(
        vec![Node::Atom(0), Node::False, Node::Implies(0, 1)],
        vec![2],
    );
    let control = Control::default();
    for candidate in 0..4 {
        let subject = interpretation(&theory, candidate);
        let frozen = FrozenReduct::new(&subject, work_limit(3), &control).unwrap();
        for tested in 0..4 {
            assert_eq!(
                frozen
                    .is_satisfied_by(&interpretation(&theory, tested), work_limit(4), &control)
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
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(0), &control).unwrap();
    for world in 0..4 {
        assert!(
            frozen
                .is_satisfied_by(&interpretation(&theory, world), work_limit(0), &control)
                .unwrap()
        );
    }
}

#[test]
fn freeze_work_ceiling_is_inclusive() {
    let theory = theory(vec![Node::Atom(0), Node::Atom(1), Node::And(0, 1)], vec![2]);
    let candidate = interpretation(&theory, 3);
    let control = Control::default();
    for limit in 0..3 {
        assert_eq!(
            FrozenReduct::new(&candidate, work_limit(limit), &control).unwrap_err(),
            Stop::WorkLimit
        );
    }
    let frozen = FrozenReduct::new(&candidate, work_limit(3), &control).unwrap();
    assert!(
        frozen
            .is_satisfied_by(&candidate, work_limit(4), &control)
            .unwrap()
    );
}

#[test]
fn satisfaction_does_not_recharge_the_freeze() {
    let theory = theory(vec![Node::Atom(0), Node::Atom(1), Node::And(0, 1)], vec![2]);
    let candidate = interpretation(&theory, 3);
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(3), &control).unwrap();
    for limit in 0..4 {
        assert_eq!(
            frozen
                .is_satisfied_by(&candidate, work_limit(limit), &control)
                .unwrap_err(),
            Stop::WorkLimit
        );
    }
    for _ in 0..3 {
        assert!(
            frozen
                .is_satisfied_by(&candidate, work_limit(4), &control)
                .unwrap()
        );
    }
    assert_eq!(
        models_reduct(&theory, &candidate, &candidate, work_limit(6), &control).unwrap_err(),
        Stop::WorkLimit
    );
    assert!(models_reduct(&theory, &candidate, &candidate, work_limit(7), &control).unwrap());
}

#[test]
fn roots_are_charged_until_the_first_failure() {
    let theory = theory(
        vec![Node::Atom(0), Node::False, Node::Implies(1, 1)],
        vec![2, 1, 0],
    );
    let candidate = interpretation(&theory, 1);
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(3), &control).unwrap();
    assert_eq!(
        frozen
            .is_satisfied_by(&candidate, work_limit(4), &control)
            .unwrap_err(),
        Stop::WorkLimit
    );
    assert!(
        !frozen
            .is_satisfied_by(&candidate, work_limit(5), &control)
            .unwrap()
    );
}

#[test]
fn foreign_tested_identity_precedes_control() {
    let own = theory(vec![Node::Atom(0)], vec![0]);
    let foreign = theory(vec![Node::Atom(0)], vec![0]);
    let candidate = interpretation(&own, 1);
    let tested = interpretation(&foreign, 1);
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(1), &control).unwrap();
    control.cancel();
    assert_eq!(
        frozen
            .is_satisfied_by(&tested, work_limit(0), &control)
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
    let control = Control::default();
    control.cancel();
    for (candidate, tested) in [(&other, &candidate), (&candidate, &other)] {
        assert_eq!(
            models_reduct(&own, candidate, tested, work_limit(0), &control).unwrap_err(),
            Stop::WrongProgram
        );
    }
}

#[test]
fn stopped_queries_leave_the_reduct_reusable() {
    let theory = theory(vec![Node::Atom(0)], vec![0]);
    let candidate = interpretation(&theory, 1);
    let control = Control::default();
    let frozen = FrozenReduct::new(&candidate, work_limit(1), &control).unwrap();
    let cancelled = Control::default();
    cancelled.cancel();
    for (stopped, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Control::with_deadline(Instant::now()).unwrap(),
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
                .is_satisfied_by(&candidate, work_limit(2), &control)
                .unwrap()
        );
    }
}

#[test]
fn empty_freezes_still_poll_control() {
    let theory = theory(vec![], vec![]);
    let candidate = interpretation(&theory, 0);
    let cancelled = Control::default();
    cancelled.cancel();
    for (control, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Control::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        assert_eq!(
            FrozenReduct::new(&candidate, work_limit(0), &control).unwrap_err(),
            expected
        );
    }
}

#[test]
fn empty_satisfaction_still_polls_control() {
    let theory = theory(vec![], vec![]);
    let candidate = interpretation(&theory, 0);
    let frozen = FrozenReduct::new(&candidate, work_limit(0), &Control::default()).unwrap();
    let cancelled = Control::default();
    cancelled.cancel();
    for (control, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Control::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        assert_eq!(
            frozen
                .is_satisfied_by(&candidate, work_limit(0), &control)
                .unwrap_err(),
            expected
        );
    }
}
