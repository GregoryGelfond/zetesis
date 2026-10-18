//! Frozen-reduct membership and complete work-reservation boundaries.

mod reservations;

use super::*;
use crate::Cancellation;
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node};

fn search(nodes: Vec<Node>, roots: Vec<usize>) -> crate::StableModels {
    let theory = Theory::new(
        2,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap();
    crate::StableModels::with_method(
        &theory,
        SearchMethod::Regions,
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap()
}

#[test]
fn cyclic_candidate_rejection_uses_complete_least_closure() {
    let theory = Theory::new(
        2,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::implies(0, 1),
                Node::implies(1, 0),
            ],
            vec![],
        )
        .unwrap(),
        vec![2, 3],
        AdmissionLimits::default(),
    )
    .unwrap();
    // Supply both classical candidates directly: original possible-founded
    // pruning may otherwise remove the nonminimal leaf before this check.
    let index = IndexedTheory::for_membership_test(&theory);
    let mut state = State::new(SearchMethod::Regions);
    let limits = Limits::default();
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: crate::SearchStatistics::default(),
    };
    let mut statistics = Statistics::default();
    let empty = Interpretation::new(&theory, []).unwrap();
    assert!(matches!(
        state
            .check(
                &theory,
                Some(&index),
                &empty,
                limits,
                &mut budget,
                &mut statistics
            )
            .unwrap(),
        Check::Stable
    ));
    let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
    let result = state
        .check(
            &theory,
            Some(&index),
            &candidate,
            limits,
            &mut budget,
            &mut statistics,
        )
        .unwrap();
    let Check::NonMinimal(witness) = result else {
        panic!("expected the empty proper-subset witness: {result:?}")
    };
    assert_eq!(witness.atoms().count(), 0);
    assert!(witness.theory().same_instance(&theory));
    assert_eq!(statistics.countermodel_queries, 2);
    assert_eq!(statistics.countermodels, 1);
    assert_eq!(statistics.reduct.frozen_positive_attempts, 2);
    assert_eq!(statistics.reduct.frozen_positive_completed, 2);
    assert_eq!(statistics.reduct.regions.regions, 0);
    assert!(statistics.reduct.frozen_positive_preparation_work > 0);
    assert!(
        statistics.reduct.frozen_positive_work > statistics.reduct.frozen_positive_preparation_work
    );
}

#[test]
fn competing_live_heads_keep_general_subset_checking() {
    let mut models = search(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::or_pair([0, 1]),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![2, 3, 4],
    );
    assert_eq!(
        models.next().unwrap().unwrap().atoms().collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(models.next().is_none());
    let statistics = models.statistics();
    assert_eq!(statistics.reduct.frozen_positive_attempts, 1);
    assert_eq!(statistics.reduct.frozen_positive_completed, 0);
    assert!(statistics.reduct.regions.regions > 0);
}
