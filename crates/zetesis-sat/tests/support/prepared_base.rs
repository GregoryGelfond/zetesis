//! Reusable propagation contains consequences of the immutable CNF alone.

use super::tests::{assert_sat, budget, query, verify_registry};
use super::*;
use crate::{AdmissionLimits, Control};

fn p(variable: usize) -> Literal {
    Literal::new(variable, true)
}

fn chain() -> Cnf {
    Cnf::new(
        5,
        vec![
            vec![p(0)],
            vec![p(0).negated(), p(1)],
            vec![p(1).negated(), p(2)],
            vec![p(3).negated(), p(4)],
        ],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn closed_prefix_is_not_undone_or_repropagated() {
    let cnf = chain();
    let mut workspace = PreparedWorkspace::default();
    assert_sat(
        &query(&mut workspace, &cnf, &[p(3)], u64::MAX).0,
        &cnf,
        &[p(3)],
    );
    assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 3 });
    let control = Control::default();
    let mut reset = budget(&control, u64::MAX);
    workspace.reset(&cnf, &mut reset).unwrap();
    // One reset plus two candidate-suffix values; the three derived base values
    // remain assigned and their already-processed events are not replayed.
    assert_eq!(reset.statistics.work, 3);
    let state = &workspace.workspace.0;
    assert_eq!(state.trail, [p(0), p(1), p(2)]);
    assert_eq!(state.propagation_head, 3);
    verify_registry(&workspace, &cnf);
    let assumptions = [p(3).negated(), p(4).negated()];
    let (answer, receipt) = query(&mut workspace, &cnf, &assumptions, u64::MAX);
    assert_sat(&answer, &cnf, &assumptions);
    assert_eq!(receipt.propagations, 0);
    assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 3 });
}

#[test]
fn parameter_consequences_never_become_base_truth() {
    let cnf = chain();
    let mut workspace = PreparedWorkspace::default();
    assert_sat(
        &query(&mut workspace, &cnf, &[p(3)], u64::MAX).0,
        &cnf,
        &[p(3)],
    );
    assert_eq!(workspace.workspace.0.value(p(4)), Some(true));
    let reversed = [p(3).negated(), p(4).negated()];
    assert_sat(
        &query(&mut workspace, &cnf, &reversed, u64::MAX).0,
        &cnf,
        &reversed,
    );
    assert_eq!(workspace.workspace.0.value(p(4)), Some(false));
    assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 3 });
}

#[test]
fn parameter_conflict_does_not_refute_the_base() {
    let cnf = chain();
    let mut workspace = PreparedWorkspace::default();
    assert!(matches!(
        query(&mut workspace, &cnf, &[p(0).negated()], u64::MAX).0,
        Solve::Unsat
    ));
    assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 3 });
    assert_sat(
        &query(&mut workspace, &cnf, &[p(3)], u64::MAX).0,
        &cnf,
        &[p(3)],
    );
    verify_registry(&workspace, &cnf);
}

fn contradiction() -> Cnf {
    Cnf::new(
        3,
        vec![vec![p(0)], vec![p(0).negated(), p(1)], vec![p(1).negated()]],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn unconditional_conflict_is_reused_for_new_parameters() {
    let cnf = contradiction();
    let mut workspace = PreparedWorkspace::default();
    let (first, cold) = query(&mut workspace, &cnf, &[p(2)], u64::MAX);
    assert!(matches!(first, Solve::Unsat));
    assert_eq!(workspace.base, BaseClosure::Refuted);
    assert!(cold.propagations > 0);
    for parameter in [p(2).negated(), p(2)] {
        let (result, reused) = query(&mut workspace, &cnf, &[parameter], 1);
        assert!(matches!(result, Solve::Unsat));
        assert_eq!(reused.work, 1, "the query boundary remains charged");
        assert_eq!(reused.propagations, 0);
        assert_eq!(reused.conflicts, 1);
        verify_registry(&workspace, &cnf);
    }
}

fn indexed(cnf: &Cnf) -> PreparedWorkspace {
    let mut workspace = PreparedWorkspace::default();
    workspace
        .reset(cnf, &mut budget(&Control::default(), u64::MAX))
        .unwrap();
    assert!(workspace.indexed);
    assert_eq!(workspace.base, BaseClosure::Unprepared);
    workspace
}

#[test]
fn interrupted_base_propagation_never_publishes_a_result() {
    for (cnf, units, expected) in [
        (chain(), vec![0], true),
        (contradiction(), vec![0, 2], false),
    ] {
        let control = Control::default();
        let mut completed = budget(&control, u64::MAX);
        assert_eq!(
            indexed(&cnf)
                .prepare_base(&cnf, &units, false, &mut completed)
                .unwrap(),
            expected
        );
        assert!(
            completed.statistics.work > units.len() as u64,
            "actual propagation must follow unit seeding"
        );
        for limit in 0..completed.statistics.work {
            let mut workspace = indexed(&cnf);
            let mut stopped = budget(&control, limit);
            assert_eq!(
                workspace.prepare_base(&cnf, &units, false, &mut stopped),
                Err(Incomplete::WorkLimit)
            );
            assert_eq!(stopped.statistics.work, limit);
            assert_eq!(workspace.base, BaseClosure::Unprepared);
            verify_registry(&workspace, &cnf);
            let (retried, _) = query(&mut workspace, &cnf, &[], u64::MAX);
            if expected {
                assert_sat(&retried, &cnf, &[]);
            } else {
                assert!(matches!(retried, Solve::Unsat));
            }
            verify_registry(&workspace, &cnf);
        }
        let mut workspace = indexed(&cnf);
        let mut exact = budget(&control, completed.statistics.work);
        assert_eq!(
            workspace
                .prepare_base(&cnf, &units, false, &mut exact)
                .unwrap(),
            expected
        );
        assert_eq!(exact.statistics.work, completed.statistics.work);
        assert_ne!(workspace.base, BaseClosure::Unprepared);
    }
}

#[test]
fn branch_backtracking_preserves_the_closed_prefix() {
    let cnf = Cnf::new(
        3,
        vec![
            vec![p(0)],
            vec![p(1), p(2)],
            vec![p(1).negated(), p(2)],
            vec![p(1), p(2).negated()],
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut workspace = PreparedWorkspace::default();
    let (result, receipt) = query(&mut workspace, &cnf, &[], u64::MAX);
    assert_sat(&result, &cnf, &[]);
    assert!(
        receipt.decisions > 0 && receipt.conflicts > 0,
        "the query must actually branch and backtrack"
    );
    assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 1 });
    assert_eq!(workspace.workspace.0.trail[0], p(0));
    assert!(
        workspace
            .workspace
            .0
            .decisions
            .iter()
            .all(|d| d.trail_start >= 1)
    );
    verify_registry(&workspace, &cnf);
}

#[test]
fn quiescent_base_is_not_a_satisfiability_decision() {
    let cnf = Cnf::new(
        2,
        vec![
            vec![p(0), p(1)],
            vec![p(0).negated(), p(1)],
            vec![p(0), p(1).negated()],
            vec![p(0).negated(), p(1).negated()],
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut workspace = PreparedWorkspace::default();
    let (result, receipt) = query(&mut workspace, &cnf, &[], u64::MAX);
    assert!(matches!(result, Solve::Unsat));
    assert!(receipt.decisions > 0);
    assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 0 });
    verify_registry(&workspace, &cnf);
}

fn moved_nonempty_base(cnf: &Cnf) -> PreparedWorkspace {
    let mut workspace = indexed(cnf);
    assert!(
        workspace
            .prepare_base(cnf, &[0], false, &mut budget(&Control::default(), u64::MAX))
            .unwrap()
    );
    let base_positions = workspace.workspace.0.positions[2];
    assert_ne!(
        base_positions,
        [0, 1],
        "the base must relocate its false watch"
    );
    let assumptions = [p(2).negated(), p(3).negated()];
    assert_sat(
        &query(&mut workspace, cnf, &assumptions, u64::MAX).0,
        cnf,
        &assumptions,
    );
    assert_ne!(
        workspace.workspace.0.positions[2], base_positions,
        "the candidate must relocate another watch after the base is closed"
    );
    assert_eq!(workspace.workspace.0.value(p(4)), Some(true));
    assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 2 });
    workspace
}

#[test]
fn interrupted_relocation_preserves_the_closed_prefix() {
    let cnf = Cnf::new(
        5,
        vec![
            vec![p(0)],
            vec![p(0).negated(), p(1)],
            vec![p(1).negated(), p(2), p(3), p(4)],
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let assumptions = [p(2), p(3).negated(), p(4).negated()];
    let mut measured = moved_nonempty_base(&cnf);
    let before = measured.workspace.0.positions[2];
    let (result, full) = query(&mut measured, &cnf, &assumptions, u64::MAX);
    assert_sat(&result, &cnf, &assumptions);
    assert_ne!(measured.workspace.0.positions[2], before);
    for limit in 0..full.work {
        // Recreate the same closed prefix, moved watches and candidate suffix.
        // The sweep includes every suffix undo and later relocation boundary.
        let mut workspace = moved_nonempty_base(&cnf);
        let (result, stopped) = query(&mut workspace, &cnf, &assumptions, limit);
        assert!(
            matches!(result, Solve::Inconclusive(Incomplete::WorkLimit)),
            "{limit}: {result:?}"
        );
        assert_eq!(stopped.work, limit);
        assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 2 });
        assert!(workspace.workspace.0.trail.starts_with(&[p(0), p(1)]));
        verify_registry(&workspace, &cnf);
        assert_sat(
            &query(&mut workspace, &cnf, &assumptions, u64::MAX).0,
            &cnf,
            &assumptions,
        );
        assert_eq!(workspace.base, BaseClosure::Closed { trail_len: 2 });
        assert!(workspace.workspace.0.trail.starts_with(&[p(0), p(1)]));
        assert_eq!(workspace.workspace.0.value(p(4)), Some(false));
        verify_registry(&workspace, &cnf);
    }
    let mut exact = moved_nonempty_base(&cnf);
    let (result, receipt) = query(&mut exact, &cnf, &assumptions, full.work);
    assert_sat(&result, &cnf, &assumptions);
    assert_eq!(receipt.work, full.work);
    verify_registry(&exact, &cnf);
}
