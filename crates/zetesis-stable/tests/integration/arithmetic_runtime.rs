//! Root arithmetic and complete answer families on the default candidate schedules.

use std::collections::BTreeSet;

use crate::support::choice_theories::exclusive_choices;
use crate::support::interpretations::stable_models;
use crate::support::routes::{ROUTES, Route};
use zetesis_ferraris::{
    AdmissionLimits, FormulaNodes, FormulaParts, Node, PreparationStatus, Theory,
};
use zetesis_stable::{Cancellation, RegionSearchStatistics, RegionWalkEngagement, StableModels};

fn assert_no_regional_inspections(regions: &RegionSearchStatistics, route: Route) {
    assert_eq!(
        (
            regions.parity.region_passes,
            regions.parity.forced,
            regions.parity.refuted
        ),
        (0, 0, 0),
        "{route:?}"
    );
    assert_eq!(
        (
            regions.cardinality.region_passes,
            regions.cardinality.refuted
        ),
        (0, 0),
        "{route:?}"
    );
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
fn candidate_schedules_skip_outer_arithmetic_inspections() {
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
        assert_no_regional_inspections(&initial, route);

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
        assert_no_regional_inspections(&regions, route);
        assert!(!regions.parity.root_refuted && !regions.cardinality.root_refuted);
        assert_eq!(search.statistics(), statistics);
    }
}

#[test]
fn a_prepared_empty_basis_and_unpaired_bounds_skip_only_runtime_batches() {
    for native_lower in [false, true] {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(
                vec![Node::atom(0), Node::atom(1), Node::or_pair([0, 1])],
                vec![],
            )
            .unwrap(),
        );
        let root = if native_lower {
            let mut edit = nodes.transaction();
            let lower = edit
                .push_threshold_family(&[0, 1], &[1, 1], &[1], usize::MAX, usize::MAX)
                .unwrap()[0];
            edit.commit();
            lower
        } else {
            2
        };
        let theory = Theory::new(
            2,
            nodes.into_parts(),
            vec![root],
            AdmissionLimits::default(),
        )
        .unwrap();
        let expected = BTreeSet::from([vec![0], vec![1]]);
        assert_eq!(stable_models(&theory), expected);
        for route in ROUTES {
            let mut search = route.search(&theory, Cancellation::default());
            assert_eq!(route.collect(&mut search), expected);
            let stats = search.statistics().regions.unwrap();
            assert!(stats.counts.regions > 0);
            assert_eq!(stats.parity.status, PreparationStatus::Prepared);
            assert_eq!(stats.cardinality.status, PreparationStatus::Prepared);
            assert_eq!(stats.cardinality.rows, usize::from(native_lower));
            assert_eq!((stats.parity.rank, stats.cardinality.families), (0, 0));
            assert_no_regional_inspections(&stats, route);
            assert!(stats.parity.preparation_work > 0 && stats.cardinality.preparation_work > 0);
        }
    }
}

#[test]
fn a_false_constant_equation_refutes_before_any_runtime_subject() {
    // (false -> false) -> false forbids the sole assignment on empty support.
    let theory = Theory::new(
        0,
        FormulaParts::new(
            vec![Node::falsum(), Node::implies(0, 0), Node::implies(1, 0)],
            vec![],
        )
        .unwrap(),
        vec![2],
        AdmissionLimits::default(),
    )
    .unwrap();
    assert!(stable_models(&theory).is_empty());
    for route in ROUTES {
        let mut search = route.search(&theory, Cancellation::default());
        assert!(search.root_refuted());
        assert!(route.collect(&mut search).is_empty());
        let stats = search.statistics().regions.unwrap();
        assert_eq!(stats.parity.rank, 0);
        assert!(stats.parity.root_refuted);
        assert_eq!(stats.cardinality.status, PreparationStatus::NotPrepared);
        assert_eq!(stats.counts.regions, 0);
        assert_no_regional_inspections(&stats, route);
    }
}

#[test]
fn root_preparation_seeds_a_checked_parity_unit() {
    let theory = Theory::new(
        1,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::falsum(),
                Node::implies(0, 1),
                Node::implies(2, 1),
                Node::or_pair([0, 2]),
            ],
            vec![],
        )
        .unwrap(),
        vec![3, 4],
        AdmissionLimits::default(),
    )
    .unwrap();
    let expected = BTreeSet::from([vec![0]]);
    assert_eq!(stable_models(&theory), expected);
    for route in ROUTES {
        let mut search = route.search(&theory, Cancellation::default());
        assert_eq!(route.collect(&mut search), expected);
        let stats = search.statistics().regions.unwrap();
        assert_eq!((stats.parity.rank, stats.parity.units), (1, 1));
        assert!(stats.parity.phases.root_seed > 0);
        assert!(stats.counts.regions > 0);
        assert_no_regional_inspections(&stats, route);
    }
}

#[test]
fn root_preparation_retains_the_guarded_counting_pair() {
    use zetesis_ferraris::NodeView;
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![Node::atom(0), Node::atom(1), Node::atom(2), Node::falsum()],
            vec![],
        )
        .unwrap(),
    );
    let mut edit = nodes.transaction();
    let bounds = edit
        .push_threshold_family(&[0, 1], &[1, 1], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    let missing = edit
        .push(NodeView::Implies(bounds[0], 3), usize::MAX, usize::MAX)
        .unwrap();
    let mut roots = vec![];
    for prohibited in [missing, bounds[1]] {
        let guarded = edit
            .push(NodeView::And(&[2, prohibited]), usize::MAX, usize::MAX)
            .unwrap();
        roots.push(
            edit.push(NodeView::Implies(guarded, 3), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    edit.commit();
    let theory = Theory::new(3, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
    let expected = BTreeSet::from([vec![]]);
    assert_eq!(stable_models(&theory), expected);
    for route in ROUTES {
        let mut search = route.search(&theory, Cancellation::default());
        assert_eq!(route.collect(&mut search), expected);
        let stats = search.statistics().regions.unwrap();
        assert_eq!((stats.cardinality.rows, stats.cardinality.guarded), (2, 2));
        assert!(stats.cardinality.families > 0);
        assert!(stats.counts.regions > 0);
        assert_no_regional_inspections(&stats, route);
    }
}
