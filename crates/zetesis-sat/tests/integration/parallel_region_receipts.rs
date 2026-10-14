//! Failed candidate narrowing keeps the work receipt of its admitted prefix.

use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use zetesis_ferraris::{AdmissionLimits, Node, Theory};
use zetesis_sat::{
    Cancellation, Engagement, Incomplete, Limits, RegionWalkEngagement, SearchLimits, StableModels,
};

/// Both routes: workers started at the first pull, and a walk that stays on
/// the calling thread, where a failing pull is the coordinator's and its
/// receipts are merged when the pull ends.
const ROUTES: [Engagement; 2] = [Engagement::Immediate, Engagement::After(Duration::MAX)];

fn region_workers(theory: &Theory, engagement: Engagement, limits: Limits) -> StableModels {
    StableModels::with_region_workers_engaging(
        theory,
        NonZeroUsize::new(2).unwrap(),
        engagement,
        limits,
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(2)).unwrap(),
    )
    .unwrap()
}

/// The walk ran on the route `engagement` names.
fn assert_route(search: &StableModels, engagement: Engagement) {
    let record = search
        .statistics()
        .regions
        .unwrap()
        .workers
        .unwrap()
        .engagement;
    match engagement {
        Engagement::Immediate => assert!(
            matches!(record, RegionWalkEngagement::Engaged { launched: 2, .. }),
            "{record:?}"
        ),
        _ => assert!(
            matches!(record, RegionWalkEngagement::Unengaged { .. }),
            "{record:?}"
        ),
    }
}

#[test]
fn exhausted_root_narrowing_keeps_its_charged_work() {
    for engagement in ROUTES {
        exhausted_root_narrowing_keeps_its_charged_work_on(engagement);
    }
}

fn exhausted_root_narrowing_keeps_its_charged_work_on(engagement: Engagement) {
    let theory = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::implies(1, 0),
                Node::or_pair([1, 2]),
            ],
            vec![],
        )
        .unwrap(),
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap();
    let initial = region_workers(&theory, engagement, Limits::default()).statistics();
    assert_eq!(initial.regions.unwrap().counts.work, initial.search.work);
    // Room for the original index, charged when the walk starts, and one
    // read of the root's narrowing.
    let limit = initial.search.work
        + u64::try_from(theory.nodes().len() + theory.parts().occurrences()).unwrap()
        + 1;
    let mut search = region_workers(
        &theory,
        engagement,
        Limits {
            search: SearchLimits {
                max_work: limit,
                ..SearchLimits::default()
            },
            ..Limits::default()
        },
    );
    assert!(matches!(search.next(), Some(Err(Incomplete::WorkLimit))));
    assert_route(&search, engagement);
    let statistics = search.statistics();
    assert!(!search.exhausted());
    assert_eq!(statistics.candidates, 0, "{statistics:?}");
    assert_eq!(statistics.search.decisions, 0, "{statistics:?}");
    assert_eq!(statistics.search.work, limit, "{statistics:?}");
    let regions = statistics.regions.unwrap();
    assert_eq!(
        regions.counts.regions, 1,
        "the root's narrowing was entered"
    );
    // With no certificate, decision or leaf, the only charged operations
    // after construction are the index and the root's narrowing.
    // RegionCounts explicitly includes those node/root/producer reads and
    // the indexes.
    assert_eq!(
        statistics.regions.unwrap().counts.work,
        statistics.search.work,
        "the failed narrowing must preserve its admitted work receipt: {statistics:?}",
    );
}

#[test]
fn failed_reduct_narrowing_keeps_its_query_count() {
    for engagement in ROUTES {
        failed_reduct_narrowing_keeps_its_query_count_on(engagement);
    }
}

fn failed_reduct_narrowing_keeps_its_query_count_on(engagement: Engagement) {
    // Two facts force one candidate and at least two frozen reads. There is
    // no concurrent candidate whose completed query can mask a missing count.
    let theory = Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap(),
        vec![0, 1],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut complete = region_workers(&theory, engagement, Limits::default());
    assert_eq!(
        complete
            .next()
            .unwrap()
            .unwrap()
            .atoms()
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert!(complete.next().is_none());
    assert!(complete.exhausted());
    assert_route(&complete, engagement);
    assert_eq!(complete.statistics().countermodel_queries, 1);
    let limit = complete.statistics().search.work - 1;
    let mut search = region_workers(
        &theory,
        engagement,
        Limits {
            search: SearchLimits {
                max_work: limit,
                ..SearchLimits::default()
            },
            ..Limits::default()
        },
    );
    assert!(matches!(search.next(), Some(Err(Incomplete::WorkLimit))));
    assert_route(&search, engagement);
    let statistics = search.statistics();
    assert!(!search.exhausted());
    assert_eq!(statistics.candidates, 1, "limit={limit}: {statistics:?}");
    // The original index was charged when the candidate walk started and is
    // shared.
    // Every unit of reduct-region work therefore comes from frozen narrowing.
    // Traversal counts completed regions, so a failed first narrowing need
    // not increment that count.
    assert!(
        statistics.reduct.regions.work > 0,
        "the failed query must have performed a frozen read: {statistics:?}",
    );
    assert_eq!(
        statistics.countermodel_queries, 1,
        "the started query is counted even when its first narrowing fails; limit={limit}: {statistics:?}",
    );
}
