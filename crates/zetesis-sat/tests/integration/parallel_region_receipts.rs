//! Failed candidate narrowing keeps the work receipt of its admitted prefix.

use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use zetesis_ferraris::{AdmissionLimits, Node, Theory};
use zetesis_sat::{Cancellation, Incomplete, Limits, SearchLimits, StableModels};

#[test]
fn exhausted_root_narrowing_keeps_its_charged_work() {
    let theory = Theory::new(
        1,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Implies(1, 0),
            Node::Or(1, 2),
        ],
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap();
    let workers = NonZeroUsize::new(2).unwrap();
    let initial = StableModels::with_region_workers(
        &theory,
        workers,
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap()
    .statistics();
    assert_eq!(initial.regions.unwrap().counts.work, initial.search.work);
    let limit = initial.search.work + 1;
    let mut search = StableModels::with_region_workers(
        &theory,
        workers,
        Limits {
            search: SearchLimits {
                max_work: limit,
                ..SearchLimits::default()
            },
            ..Limits::default()
        },
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(2)).unwrap(),
    )
    .unwrap();
    assert!(matches!(search.next(), Some(Err(Incomplete::WorkLimit))));
    let statistics = search.statistics();
    assert!(!search.exhausted());
    assert_eq!(statistics.candidates, 0, "{statistics:?}");
    assert_eq!(statistics.search.decisions, 0, "{statistics:?}");
    assert_eq!(statistics.search.work, limit, "{statistics:?}");
    // With no certificate, decision or leaf, the only post-construction
    // charged operation is the root's narrowing. RegionCounts explicitly
    // includes those node/root/producer reads and the construction indexes.
    assert_eq!(
        statistics.regions.unwrap().counts.work,
        statistics.search.work,
        "the failed narrowing must preserve its admitted work receipt: {statistics:?}",
    );
}

#[test]
fn failed_reduct_narrowing_keeps_its_query_count() {
    // Two facts force one candidate and at least two frozen reads. There is
    // no concurrent candidate whose completed query can mask a missing count.
    let theory = Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1)],
        vec![0, 1],
        AdmissionLimits::default(),
    )
    .unwrap();
    let workers = NonZeroUsize::new(2).unwrap();
    let mut complete = StableModels::with_region_workers(
        &theory,
        workers,
        Limits::default(),
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(2)).unwrap(),
    )
    .unwrap();
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
    assert_eq!(complete.statistics().countermodel_queries, 1);
    let limit = complete.statistics().search.work - 1;
    let mut search = StableModels::with_region_workers(
        &theory,
        workers,
        Limits {
            search: SearchLimits {
                max_work: limit,
                ..SearchLimits::default()
            },
            ..Limits::default()
        },
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(2)).unwrap(),
    )
    .unwrap();
    assert!(matches!(search.next(), Some(Err(Incomplete::WorkLimit))));
    let statistics = search.statistics();
    assert!(!search.exhausted());
    assert_eq!(statistics.candidates, 1, "limit={limit}: {statistics:?}");
    // The original index was charged by candidate preparation and is shared.
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
