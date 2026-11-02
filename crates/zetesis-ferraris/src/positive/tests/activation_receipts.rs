//! Exact legacy propagation boundaries, usable before the kernel factoring.

use super::*;
use crate::positive::compile::{Edge, Graph, State};

struct Snapshot {
    result: Result<(), PositiveError>,
    statistics: PositivePlanStatistics,
    graph: Graph,
    word: u64,
}

fn run(owner: &Theory, mut graph: Graph, maximum: u64) -> Snapshot {
    let cancellation = Cancellation::default();
    let limits = PositivePlanLimits {
        max_work: maximum,
        ..PositivePlanLimits::default()
    };
    let mut statistics = PositivePlanStatistics::default();
    let mut words = [0];
    let metered = cancellation.metered(maximum, |meter| {
        propagate::complete(
            owner,
            &mut graph,
            &mut words,
            &mut Budget {
                limits,
                cancellation: &cancellation,
                meter,
                statistics: &mut statistics,
                current_bytes: 0,
            },
        )
    });
    statistics.work = metered.charged;
    Snapshot {
        result: metered.result,
        statistics,
        graph,
        word: words[0],
    }
}

fn repeated_graph() -> Graph {
    Graph {
        offsets: vec![0, 2, 2, 2, 2, 2, 2],
        edges: vec![
            Edge {
                target: 1,
                weight: 1,
            },
            Edge {
                target: 1,
                weight: 1,
            },
        ],
        states: vec![
            State::Awaiting(1),
            State::Awaiting(2),
            State::Inactive,
            State::Inactive,
            State::Inactive,
            State::Inactive,
        ],
        queue: Vec::with_capacity(6),
    }
}

fn repeated_owner(roots: Vec<usize>) -> Theory {
    theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(2, 2),
        ],
        roots,
    )
}

#[test]
fn legacy_ticks_preserve_every_repeated_incidence_prefix() {
    let owner = repeated_owner(vec![0]);
    // (activated formula vertices, derived atoms, traversed edges, atom word).
    // Seed scans take six ticks before the canonical truth's first activation;
    // root lookup and activation then take two. The queue ticks at 10, 11, 15;
    // its two repeated incidences tick at 12 and 13, and atom 1 activates at 14.
    let expected = [
        (0, 0, 0, 0),
        (0, 0, 0, 0),
        (0, 0, 0, 0),
        (0, 0, 0, 0),
        (0, 0, 0, 0),
        (0, 0, 0, 0),
        (0, 0, 0, 0),
        (1, 0, 0, 0),
        (1, 0, 0, 0),
        (1, 1, 0, 0),
        (1, 1, 0, 0),
        (1, 1, 0, 1),
        (1, 1, 1, 1),
        (1, 1, 2, 1),
        (1, 2, 2, 1),
        (1, 2, 2, 3),
    ];
    for (maximum, &(nodes, atoms, edges, word)) in expected.iter().enumerate() {
        let maximum = u64::try_from(maximum).unwrap();
        let snapshot = run(&owner, repeated_graph(), maximum);
        if maximum == 15 {
            assert_eq!(snapshot.result, Ok(()));
        } else {
            assert_eq!(
                snapshot.result,
                Err(PositiveError::Limit {
                    resource: PositiveResource::Work,
                    observed: u128::from(maximum) + 1,
                    limit: u128::from(maximum),
                })
            );
        }
        assert_eq!(
            snapshot.statistics,
            PositivePlanStatistics {
                work: maximum,
                derived_atoms: atoms,
                activated_nodes: nodes,
                propagated_dependencies: edges,
                ..PositivePlanStatistics::default()
            },
            "prefix {maximum}"
        );
        assert_eq!(snapshot.word, word, "prefix {maximum}");
        assert_eq!(snapshot.graph.queue, [5, 0, 1][..nodes + atoms]);
        let remaining = if edges == 0 { 2 } else { 1 };
        let expected_state = if atoms == 2 {
            State::True
        } else {
            State::Awaiting(remaining)
        };
        assert!(snapshot.graph.states[1] == expected_state);
    }
}

#[test]
fn duplicate_seed_roots_add_no_activation_tick() {
    let snapshot = run(&repeated_owner(vec![0, 0]), repeated_graph(), u64::MAX);
    assert_eq!(snapshot.result, Ok(()));
    assert_eq!(snapshot.statistics.work, 16);
    assert_eq!(snapshot.statistics.derived_atoms, 2);
    assert_eq!(snapshot.statistics.activated_nodes, 1);
    assert_eq!(snapshot.statistics.propagated_dependencies, 2);
    assert_eq!(snapshot.graph.queue, [5, 0, 1]);
    assert_eq!(snapshot.word, 3);
}

#[test]
fn a_weighted_activation_refusal_keeps_its_incidence_receipt() {
    let owner = theory(2, vec![Node::atom(0), Node::atom(1)], vec![0]);
    for (remaining, weights) in [
        (3_i128, [1, 2]),
        (i128::from(u64::MAX) + 1, [u64::MAX, u64::MAX]),
    ] {
        let graph = || Graph {
            offsets: vec![0, 2, 2, 2, 2],
            edges: weights.map(|weight| Edge { target: 1, weight }).to_vec(),
            states: vec![
                State::Awaiting(1),
                State::Weighted(remaining),
                State::Inactive,
                State::Inactive,
            ],
            queue: Vec::with_capacity(4),
        };
        let refused = run(&owner, graph(), 7);
        assert_eq!(
            refused.result,
            Err(PositiveError::Limit {
                resource: PositiveResource::Work,
                observed: 8,
                limit: 7,
            })
        );
        assert_eq!(refused.statistics.work, 7);
        assert_eq!(refused.statistics.propagated_dependencies, 2);
        assert_eq!(refused.statistics.derived_atoms, 1);
        assert_eq!(refused.graph.queue, [0]);
        assert_eq!(refused.word, 1);
        assert!(refused.graph.states[1] == State::Weighted(remaining - i128::from(weights[0])));
        let complete = run(&owner, graph(), 9);
        assert_eq!(complete.result, Ok(()));
        assert_eq!(complete.statistics.work, 9);
        assert_eq!(complete.statistics.propagated_dependencies, 2);
        assert_eq!(complete.statistics.derived_atoms, 2);
        assert_eq!(complete.graph.queue, [0, 1]);
        assert_eq!(complete.word, 3);
    }
}
