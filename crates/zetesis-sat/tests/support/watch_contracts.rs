//! Watch moves preserve the registry independently of its optional-link layout.

use super::{Budget, Decision, LocalQuota, State, WatchNode, scratch_bytes, storage};
use crate::{AdmissionLimits, Cnf, Control, Incomplete, Literal, SearchLimits, SearchStatistics};

fn budget(control: &Control) -> Budget<'_> {
    Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        control,
        statistics: SearchStatistics::default(),
    }
}

fn registry(state: &State, cnf: &Cnf) -> Vec<Vec<usize>> {
    let mut seen = vec![false; state.next.len()];
    let mut lists = Vec::new();
    for (literal, &head) in state.heads.iter().enumerate() {
        let mut list = Vec::new();
        let mut cursor = head;
        while let Some(node) = cursor {
            let index = node.index();
            assert!(
                !seen[index],
                "a node has exactly one owner; cycles are invalid"
            );
            seen[index] = true;
            let clause = index / 2;
            let position = state.positions[clause][index % 2];
            assert_eq!(cnf.clauses()[clause][position].index(), literal);
            list.push(index);
            cursor = state.next[index];
        }
        lists.push(list);
    }
    for (index, present) in seen.into_iter().enumerate() {
        assert_eq!(present, cnf.clauses()[index / 2].len() >= 2);
    }
    lists
}

#[test]
fn relocating_a_watch_preserves_every_other_link() {
    let p = |variable| Literal::new(variable, true);
    let cnf = Cnf::new(
        7,
        (0..3)
            .map(|clause| vec![p(0), p(1 + 2 * clause), p(2 + 2 * clause)])
            .collect(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let control = Control::default();
    // Every position in a three-node list moves once: head, interior and tail.
    for moved_clause in 0..3 {
        let mut charged = budget(&control);
        let mut state = State::new(&cnf, &mut charged).unwrap();
        assert!(state.initialize(&cnf, &mut charged).unwrap());
        let mut expected = registry(&state, &cnf);
        assert_eq!(expected[p(0).index()], [4, 2, 0]);
        let capacities = (state.heads.capacity(), state.next.capacity());
        for clause in 0..3 {
            if clause != moved_clause {
                assert!(state.assign(p(1 + 2 * clause)));
            }
        }
        assert!(state.assign(p(0).negated()));
        assert!(state.propagate(&cnf, &mut charged).unwrap());
        expected[p(0).index()].retain(|&node| node != 2 * moved_clause);
        expected[p(2 + 2 * moved_clause).index()] = vec![2 * moved_clause];
        assert_eq!(registry(&state, &cnf), expected);
        assert_eq!((state.heads.capacity(), state.next.capacity()), capacities);
    }
}

#[test]
fn trial_undo_retains_valid_relocated_watches() {
    let p = |variable| Literal::new(variable, true);
    let cnf = Cnf::new(
        5,
        vec![vec![p(0), p(1), p(2)], vec![p(0).negated(), p(3), p(4)]],
        AdmissionLimits::default(),
    )
    .unwrap();
    let control = Control::default();
    let mut charged = budget(&control);
    let mut state = State::new(&cnf, &mut charged).unwrap();
    assert!(state.initialize(&cnf, &mut charged).unwrap());
    assert!(state.probe(&cnf, 1, &mut charged).unwrap());
    assert!(state.trail.is_empty());
    assert!(state.values.iter().all(Option::is_none));
    assert_eq!(state.propagation_head, 0);
    assert_eq!(state.positions, [[2, 1], [2, 1]]);
    let lists = registry(&state, &cnf);
    assert!(lists[p(0).index()].is_empty());
    assert!(lists[p(0).negated().index()].is_empty());
    assert_eq!(lists[p(2).index()], [0]);
    assert_eq!(lists[p(4).index()], [2]);
}

#[test]
fn optional_link_reservation_failure_is_inconclusive() {
    assert!(matches!(
        storage::<Option<WatchNode>>(usize::MAX),
        Err(Incomplete::Allocation)
    ));
}

#[test]
fn scratch_reservation_counts_the_stored_link_type() {
    let variables = 7;
    let clauses = 3;
    let control = Control::default();
    let cnf = Cnf::new(
        variables,
        vec![vec![Literal::new(0, true)]; clauses],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut charged = budget(&control);
    let state = State::new(&cnf, &mut charged).unwrap();
    let links = std::mem::size_of_val(state.heads.as_slice())
        + std::mem::size_of_val(state.next.as_slice());
    assert_eq!(links, 2 * (variables + clauses) * size_of::<usize>());
    // This independently sums the other requested typed storage populations:
    // values, trail, decisions, positions, ranks/order/sort scratch and witness.
    let other = size_of::<State>()
        + variables
            * (size_of::<Option<bool>>()
                + size_of::<Literal>()
                + size_of::<Decision>()
                + 4 * size_of::<usize>()
                + size_of::<u64>()
                + size_of::<bool>())
        + clauses * size_of::<[usize; 2]>();
    assert_eq!(
        scratch_bytes(variables as u128, clauses as u128),
        (other + links) as u128
    );
}
