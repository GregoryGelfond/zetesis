//! Complete families on the actual candidate schedules, with runtime activity.

use std::collections::BTreeSet;

use crate::support::interpretations::stable_models;
use crate::support::routes::{ROUTES, Route};
use zetesis_ferraris::{
    AdmissionLimits, FormulaNodes, FormulaParts, Node, NodeView, PreparationStatus, Theory,
};
use zetesis_stable::{Cancellation, RegionWalkEngagement, StableModels};

/// {a}. {b}. :- a, b. :- not a, not b.
/// The clauses give the nonunit parity row. An equivalent native threshold
/// interval 1 <= count{a,b} <= 1 gives explicit recognized C1 lower/upper rows.
fn exclusive_choices() -> Theory {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),          // a
                Node::atom(1),          // b
                Node::falsum(),         // 2
                Node::implies(0, 2),    // not a
                Node::implies(1, 2),    // not b
                Node::or_pair([0, 3]),  // a | not a
                Node::or_pair([1, 4]),  // b | not b
                Node::and_pair([0, 1]), // a & b
                Node::and_pair([3, 4]), // not a & not b
                Node::implies(7, 2),    // :- a, b
                Node::implies(8, 2),    // :- not a, not b
            ],
            vec![],
        )
        .unwrap(),
    );
    let mut transaction = nodes.transaction();
    let bounds = transaction
        .push_threshold_family(&[0, 1], &[1, 1], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    let at_most_one = transaction
        .push(NodeView::Implies(bounds[1], 2), usize::MAX, usize::MAX)
        .unwrap();
    transaction.commit();
    Theory::new(
        2,
        nodes.into_parts(),
        vec![5, 6, 9, 10, bounds[0], at_most_one],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn assert_route(route: Route, search: &StableModels) {
    let regions = search.statistics().regions.unwrap();
    assert!(regions.counts.regions > 0, "{route:?}");
    match route {
        Route::Scalar => {
            assert!(regions.workers.is_none());
            assert!(regions.frontier.is_none());
            assert!(regions.frontier_order.is_none());
        }
        Route::Producers => {
            assert!(regions.workers.is_none());
            let frontier = regions.frontier.unwrap();
            assert!(regions.frontier_order.is_some());
            assert!(frontier.peak_regions > 0);
            assert_eq!(frontier.regions, 0);
            assert!(search.batch_statistics().checker_calls > 0);
        }
        Route::Native => {
            assert!(regions.frontier.is_none());
            assert!(matches!(
                regions.workers.unwrap().engagement,
                RegionWalkEngagement::Engaged { launched: 4, .. }
            ));
        }
        Route::Coordinator => {
            assert!(regions.frontier.is_none());
            assert!(matches!(
                regions.workers.unwrap().engagement,
                RegionWalkEngagement::Unengaged { .. }
            ));
        }
    }
}

#[test]
fn every_candidate_schedule_checks_arithmetic_and_returns_the_exact_family() {
    let theory = exclusive_choices();
    let expected = BTreeSet::from([vec![0], vec![1]]);
    // Independently enumerate every interpretation through the maintained
    // reduct checker, so this expected family is not another candidate walk.
    assert_eq!(stable_models(&theory), expected);
    for route in ROUTES {
        let mut search = route.search(&theory, Cancellation::default());
        assert!(!search.root_refuted(), "{route:?}");
        let initial = search.statistics().regions.unwrap();
        assert_eq!(initial.parity.status, PreparationStatus::Prepared);
        assert!(initial.parity.rank > 0);
        assert_eq!(initial.parity.units, 0);
        assert_eq!(initial.cardinality.status, PreparationStatus::Prepared);
        assert!(initial.cardinality.families > 0);
        assert_eq!(initial.parity.region_passes, 0);
        assert_eq!(initial.cardinality.region_passes, 0);

        // The existing route helper rejects duplicates and requires exhausted
        // coverage; its producer path uses residual batched membership.
        assert_eq!(route.collect(&mut search), expected, "{route:?}");
        search.stop().unwrap();
        assert!(search.exhausted());
        assert_route(route, &search);
        let statistics = search.statistics();
        let regions = statistics.regions.unwrap();
        assert_eq!(statistics.stable_models, 2);
        assert!(statistics.certified.is_none());
        assert!(regions.parity.region_passes > 0, "{route:?}");
        assert!(regions.cardinality.region_passes > 0, "{route:?}");
        assert!(!regions.parity.root_refuted && !regions.cardinality.root_refuted);
        assert!(regions.parity.refuted <= regions.parity.region_passes);
        assert!(regions.cardinality.refuted <= regions.cardinality.region_passes);
        assert_eq!(search.statistics(), statistics);
    }
}
