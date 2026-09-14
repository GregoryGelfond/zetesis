//! Retained topology is independent of assignments, including failed queries.

use super::*;
use crate::search::LocalQuota;
use crate::{AdmissionLimits, Control, SearchLimits, SearchStatistics};

pub(super) fn budget(control: &Control, max_work: u64) -> Budget<'_, LocalQuota> {
    Budget {
        quota: LocalQuota,
        control,
        limits: SearchLimits {
            max_work,
            ..Default::default()
        },
        statistics: SearchStatistics::default(),
    }
}

fn disjunction() -> Cnf {
    Cnf::new(
        4,
        vec![(0..4).map(|v| Literal::new(v, true)).collect()],
        AdmissionLimits::default(),
    )
    .unwrap()
}

pub(super) fn verify_registry(workspace: &PreparedWorkspace, cnf: &Cnf) {
    assert!(workspace.indexed);
    let state = &workspace.workspace.0;
    let mut seen = vec![false; state.next.len()];
    for (literal, &head) in state.heads.iter().enumerate() {
        let mut cursor = head;
        while let Some(node) = cursor {
            let index = node.index();
            assert!(!seen[index], "duplicate or cyclic watch node");
            seen[index] = true;
            let clause = index / 2;
            let positions = state.positions[clause];
            assert_ne!(positions[0], positions[1]);
            assert_eq!(
                cnf.clause_at(clause).at(positions[index % 2]).index(),
                literal
            );
            cursor = state.next[index];
        }
    }
    for (clause, values) in cnf.clauses().enumerate() {
        assert_eq!(seen[2 * clause], values.len() >= 2);
        assert_eq!(seen[2 * clause + 1], values.len() >= 2);
    }
    let mut assigned = vec![false; cnf.variables()];
    for literal in &state.trail {
        assert!(!assigned[literal.variable()]);
        assigned[literal.variable()] = true;
        assert_eq!(state.value(*literal), Some(true));
    }
    for (index, value) in state.values.iter().enumerate() {
        assert_eq!(value.is_some(), assigned[index]);
    }
}

pub(super) fn query(
    workspace: &mut PreparedWorkspace,
    cnf: &Cnf,
    assumptions: &[Literal],
    max_work: u64,
) -> (Solve, SearchStatistics) {
    let units = cnf
        .clauses()
        .enumerate()
        .filter_map(|(i, c)| (c.len() == 1).then_some(i))
        .collect::<Vec<_>>();
    let empty = cnf.clauses().any(crate::Clause::is_empty);
    let control = Control::default();
    let mut meter = budget(&control, max_work);
    let result = workspace.query(cnf, &units, empty, assumptions, &mut meter);
    (result, meter.statistics)
}

pub(super) fn assert_sat(result: &Solve, cnf: &Cnf, assumptions: &[Literal]) {
    let Solve::Sat(assignment) = result else {
        panic!("expected SAT: {result:?}")
    };
    for clause in cnf.clauses() {
        assert!(
            clause
                .iter()
                .any(|l| assignment.value(l.variable()) == Some(l.positive()))
        );
    }
    assert!(
        assumptions
            .iter()
            .all(|l| assignment.value(l.variable()) == Some(l.positive()))
    );
}

#[test]
fn warm_reset_preserves_moved_watches() {
    let cnf = disjunction();
    let assumptions = (0..3).map(|v| Literal::new(v, false)).collect::<Vec<_>>();
    let mut workspace = PreparedWorkspace::default();
    assert_sat(
        &query(&mut workspace, &cnf, &assumptions, u64::MAX).0,
        &cnf,
        &assumptions,
    );
    verify_registry(&workspace, &cnf);
    let state = &workspace.workspace.0;
    assert_ne!(
        state.positions[0],
        [0, 1],
        "the prior search must relocate a watch"
    );
    let topology = (
        state.positions.clone(),
        state.heads.clone(),
        state.next.clone(),
    );
    let retained = workspace.retained_bytes();
    let control = Control::default();
    let mut meter = budget(&control, u64::MAX);
    workspace.reset(&cnf, &mut meter).unwrap();
    // One query reset plus four assigned-value undo operations; no index fill.
    assert_eq!(meter.statistics.work, 5);
    let state = &workspace.workspace.0;
    assert_eq!(
        (&state.positions, &state.heads, &state.next),
        (&topology.0, &topology.1, &topology.2)
    );
    assert!(state.values.iter().all(Option::is_none));
    assert!(state.trail.is_empty());
    assert_eq!(workspace.retained_bytes(), retained);
    verify_registry(&workspace, &cnf);
}

#[test]
fn interrupted_indexing_is_never_reused() {
    let cnf = disjunction();
    let control = Control::default();
    let mut full = budget(&control, u64::MAX);
    PreparedWorkspace::default().reset(&cnf, &mut full).unwrap();
    for limit in 0..full.statistics.work {
        let mut workspace = PreparedWorkspace::default();
        let mut stopped = budget(&control, limit);
        assert_eq!(
            workspace.reset(&cnf, &mut stopped),
            Err(Incomplete::WorkLimit)
        );
        assert_eq!(stopped.statistics.work, limit);
        assert!(!workspace.indexed);
        let mut retried = budget(&control, full.statistics.work);
        workspace.reset(&cnf, &mut retried).unwrap();
        assert_eq!(retried.statistics.work, full.statistics.work);
        verify_registry(&workspace, &cnf);
    }
}

fn moved_workspace(cnf: &Cnf) -> PreparedWorkspace {
    let mut workspace = PreparedWorkspace::default();
    let assumptions = (0..3).map(|v| Literal::new(v, false)).collect::<Vec<_>>();
    assert_sat(
        &query(&mut workspace, cnf, &assumptions, u64::MAX).0,
        cnf,
        &assumptions,
    );
    workspace
}

#[test]
fn every_warm_stop_preserves_a_recoverable_registry() {
    let cnf = disjunction();
    let assumptions = (1..4).map(|v| Literal::new(v, false)).collect::<Vec<_>>();
    let (_, full) = query(&mut moved_workspace(&cnf), &cnf, &assumptions, u64::MAX);
    for limit in 0..full.work {
        // Recreate the same moved-watch, fully assigned prestate for every
        // inclusive boundary. A different prior query has a different undo cost.
        let mut workspace = moved_workspace(&cnf);
        let (result, stopped) = query(&mut workspace, &cnf, &assumptions, limit);
        assert!(
            matches!(result, Solve::Inconclusive(Incomplete::WorkLimit)),
            "{limit}: {result:?}"
        );
        assert_eq!(stopped.work, limit);
        verify_registry(&workspace, &cnf);
        assert_sat(
            &query(&mut workspace, &cnf, &assumptions, u64::MAX).0,
            &cnf,
            &assumptions,
        );
        verify_registry(&workspace, &cnf);
    }
    let mut workspace = moved_workspace(&cnf);
    let (result, exact) = query(&mut workspace, &cnf, &assumptions, full.work);
    assert_sat(&result, &cnf, &assumptions);
    assert_eq!(exact.work, full.work);
}

#[test]
fn prepared_base_clauses_hold_in_every_query() {
    let p = |v| Literal::new(v, true);
    for clauses in [
        vec![vec![p(0)], vec![p(0).negated(), p(1)]],
        vec![vec![p(0)], vec![p(0).negated()]],
        vec![vec![]],
        vec![],
    ] {
        let cnf = Cnf::new(2, clauses.clone(), AdmissionLimits::default()).unwrap();
        let mut workspace = PreparedWorkspace::default();
        for assumptions in [
            vec![p(1).negated()],
            vec![p(0), p(0).negated()],
            vec![],
            vec![p(1)],
        ] {
            let mut explicit = clauses.clone();
            explicit.extend(assumptions.iter().map(|&l| vec![l]));
            let expected = crate::solve(
                &Cnf::new(2, explicit, AdmissionLimits::default()).unwrap(),
                SearchLimits::default(),
                &Control::default(),
            );
            let (actual, _) = query(&mut workspace, &cnf, &assumptions, u64::MAX);
            match expected {
                Solve::Sat(_) => assert_sat(&actual, &cnf, &assumptions),
                Solve::Unsat => assert!(matches!(actual, Solve::Unsat)),
                Solve::Inconclusive(error) => panic!("finite reference: {error}"),
            }
            verify_registry(&workspace, &cnf);
        }
    }
}
