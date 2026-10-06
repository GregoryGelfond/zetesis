//! Exact subject ownership and build accounting for the shared original index.

use std::num::NonZeroUsize;

use super::*;
use crate::ferraris::fresh_membership;
use crate::ferraris::original_index::{OriginalIndex, hooks};
use crate::prepared_reduct::State;
use crate::search::LocalQuota;
use crate::{
    BatchLimits, BatchVerdict, CertificateLimits, CertificateOrder, Limits, SearchLimits,
    SearchStatistics, StableModels, Statistics,
};
use std::sync::Arc;
use zetesis_ferraris::{AdmissionLimits, Node};

/// `a <- b`, `b <- a`: the classical models are `{}` and `{a, b}`, and only
/// `{}` is stable, so enumeration runs a proper-subset query on `{a, b}`.
fn cycle() -> Theory {
    Theory::new(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Implies(0, 1),
            Node::Implies(1, 0),
        ],
        vec![2, 3],
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// The fact `a` and `b <- a`: a positive program whose least model `{a, b}`
/// is its one answer set, decided by a positive certificate.
fn positive() -> Theory {
    Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1)],
        vec![0, 2],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn budget(cancellation: &Cancellation, work: u64) -> Budget<'_> {
    Budget {
        quota: LocalQuota,
        limits: SearchLimits {
            max_work: work,
            ..SearchLimits::default()
        },
        cancellation,
        statistics: SearchStatistics::default(),
    }
}

fn nodes(theory: &Theory) -> u64 {
    u64::try_from(theory.nodes().len()).unwrap()
}

fn extraction(theory: &Theory) -> u64 {
    zetesis_ferraris::producers(theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .work
}

fn built(theory: &Theory) -> Arc<IndexedTheory> {
    Arc::clone(OriginalIndex::new(theory).ensure(|_| Ok(())).unwrap())
}

/// Every region route, by name: scalar, parallel workers and producers.
const ROUTES: [(&str, usize, bool); 5] = [
    ("scalar", 1, false),
    ("parallel", 2, false),
    ("parallel", 4, false),
    ("producers", 2, true),
    ("producers", 4, true),
];

fn enumeration(theory: &Theory, workers: usize, producers: bool, limits: Limits) -> StableModels {
    let workers = NonZeroUsize::new(workers).unwrap();
    if producers {
        StableModels::with_region_producers(theory, workers, limits, Cancellation::default())
    } else {
        StableModels::with_region_workers(theory, workers, limits, Cancellation::default())
    }
    .unwrap()
}

#[test]
fn an_enumeration_builds_one_original_index() {
    // The successor of the index-sharing test: the candidate walk and every
    // proper-subset query read one index, built once per enumeration. Each
    // run admits its own theory, so the per-instance count is this run's.
    for (route, workers, producers) in ROUTES {
        let theory = cycle();
        let mut search = enumeration(&theory, workers, producers, Limits::default());
        assert_eq!(
            hooks::builds(&theory),
            0,
            "{route}: construction builds none"
        );
        let models: Vec<_> = search.by_ref().map(Result::unwrap).collect();
        assert_eq!(models.len(), 1);
        assert!(search.exhausted());
        let statistics = search.statistics();
        assert!(statistics.countermodel_queries > 0, "{route}");
        assert_eq!(hooks::builds(&theory), 1, "{route} with {workers} workers");
        // The walk's receipts carry the index once; the queries share it.
        assert_eq!(
            statistics.regions.unwrap().counts.work,
            PARENT_CYCLE_REGIONS_WORK,
            "{route}"
        );
        assert_eq!(
            statistics.reduct.regions.work, PARENT_CYCLE_REDUCT_WORK,
            "{route}"
        );
        assert_eq!(
            statistics.search.work, PARENT_CYCLE_SEARCH_WORK,
            "{route} with {workers} workers"
        );
    }
}

/// The parent head's receipts for a complete enumeration of `cycle()`, on
/// every region route: building the index when the walk starts changes no
/// walking run's totals.
const PARENT_CYCLE_REGIONS_WORK: u64 = 32;
const PARENT_CYCLE_REDUCT_WORK: u64 = 18;
const PARENT_CYCLE_SEARCH_WORK: u64 = 54;

#[test]
fn a_positive_certificate_route_builds_no_index() {
    for (route, workers, producers) in ROUTES {
        let theory = positive();
        let mut search = enumeration(&theory, workers, producers, Limits::default());
        assert!(
            search
                .enable_class_checking(
                    CertificateLimits::default(),
                    CertificateOrder::PositiveFirst
                )
                .unwrap()
        );
        let models: Vec<_> = search.by_ref().map(Result::unwrap).collect();
        assert_eq!(models.len(), 1);
        assert!(search.exhausted());
        assert_eq!(hooks::builds(&theory), 0, "{route} with {workers} workers");
        assert_eq!(
            search.statistics().regions.unwrap().counts.work,
            extraction(&theory),
            "{route}: only the producer extraction"
        );
    }
}

#[test]
fn a_batched_positive_certificate_route_builds_no_index() {
    for (route, workers, producers) in ROUTES {
        let theory = positive();
        let mut search = enumeration(&theory, workers, producers, Limits::default());
        assert!(
            search
                .enable_class_checking(
                    CertificateLimits::default(),
                    CertificateOrder::PositiveFirst
                )
                .unwrap()
        );
        let limits = BatchLimits {
            max_candidates: NonZeroUsize::new(2).unwrap(),
            max_pending_bytes: 4096,
        };
        let models = search
            .next_batch(limits, |_, candidates| {
                Ok::<_, std::convert::Infallible>(vec![BatchVerdict::Residual; candidates.len()])
            })
            .unwrap();
        assert_eq!(models.len(), 1, "{route}");
        assert_eq!(hooks::builds(&theory), 0, "{route} with {workers} workers");
        assert_eq!(
            search.statistics().regions.unwrap().counts.work,
            extraction(&theory),
            "{route}"
        );
    }
}

#[test]
fn a_refused_index_charge_builds_nothing_and_stops_incomplete() {
    for (route, workers, producers) in ROUTES {
        let theory = cycle();
        let setup = extraction(&theory);
        // Room for the extraction, one unit short of the index.
        let limits = Limits {
            search: SearchLimits {
                max_work: setup + nodes(&theory) - 1,
                ..SearchLimits::default()
            },
            ..Limits::default()
        };
        let mut search = enumeration(&theory, workers, producers, limits);
        assert!(matches!(search.next(), Some(Err(Incomplete::WorkLimit))));
        assert!(search.next().is_none());
        assert!(!search.exhausted(), "{route}");
        assert!(search.index.get().is_none(), "{route}");
        assert_eq!(hooks::builds(&theory), 0, "{route} with {workers} workers");
        let statistics = search.statistics();
        assert_eq!(
            statistics.search.work, setup,
            "a refused charge spends nothing"
        );
        assert_eq!(statistics.regions.unwrap().counts.work, setup, "{route}");
    }
}

#[test]
fn a_build_failing_after_its_charge_keeps_the_charge_in_both_receipts() {
    // The admitted charge stays recorded, as a failed narrowing keeps its
    // admitted prefix: search work and the walk's `regions.work` agree.
    for (route, workers, producers) in ROUTES {
        let theory = cycle();
        let mut search = enumeration(&theory, workers, producers, Limits::default());
        hooks::fail_next_build();
        assert!(matches!(search.next(), Some(Err(Incomplete::Allocation))));
        assert!(!search.exhausted(), "{route}");
        assert!(search.index.get().is_none(), "{route}");
        assert_eq!(hooks::builds(&theory), 1, "{route} with {workers} workers");
        let statistics = search.statistics();
        let charged = extraction(&theory) + nodes(&theory);
        assert_eq!(statistics.search.work, charged, "{route}");
        assert_eq!(statistics.regions.unwrap().counts.work, charged, "{route}");
    }
}

#[test]
fn a_refused_charge_leaves_the_owner_unbuilt() {
    let theory = cycle();
    let mut owner = OriginalIndex::new(&theory);
    let mut asked = None;
    assert_eq!(
        owner
            .ensure(|work| {
                asked = Some(work);
                Err(Incomplete::WorkLimit)
            })
            .map(|_| ()),
        Err(Incomplete::WorkLimit)
    );
    // The charge is the index's documented cost, known before building.
    assert_eq!(asked, Some(nodes(&theory)));
    assert!(owner.get().is_none());
    assert_eq!(hooks::builds(&theory), 0);
}

#[test]
fn a_lent_index_rejects_an_independent_equal_theory() {
    let theory = cycle();
    let equal = cycle();
    let candidate = Interpretation::new(&equal, []).unwrap();
    let index = built(&theory);
    let mut state = State::new(SearchMethod::Regions);
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, 100);
    let mut statistics = Statistics::default();
    assert!(matches!(
        state.check(
            &equal,
            Some(&index),
            &candidate,
            Limits::default(),
            &mut budget,
            &mut statistics
        ),
        Err(Incomplete::WrongTheory)
    ));
    // An empty truth buffer would be invalid for either DAG. Identity refusal
    // must precede every indexed access, independently of equal dimensions.
    for subject in [&equal, &theory] {
        assert!(matches!(
            ReductQuery::new(&index).check(
                zetesis_ferraris::FrozenSubject::new(subject, &[]),
                &candidate,
                Limits::default(),
                &mut budget,
                &mut statistics,
                &mut zetesis_ferraris::NarrowingScratch::default(),
            ),
            Err(Incomplete::WrongTheory)
        ));
    }
    assert_eq!(budget.statistics, SearchStatistics::default());
    assert_eq!(statistics, Statistics::default());
}

#[test]
fn a_built_index_needs_no_second_grant() {
    let theory = cycle();
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let mut owner = OriginalIndex::new(&theory);
    owner.ensure(|work| budget.charge(work)).unwrap();
    let paid = budget.statistics;
    assert_eq!(paid.work, nodes(&theory));
    budget.limits.max_work = paid.work;
    let index = Arc::clone(
        owner
            .ensure(|_| panic!("a built index is never charged again"))
            .unwrap(),
    );
    assert_eq!(hooks::builds(&theory), 1);
    let mut state = State::new(SearchMethod::Regions);
    let mut statistics = Statistics::default();
    let candidate = Interpretation::new(&theory, []).unwrap();
    assert!(matches!(
        state.check(
            &theory,
            Some(&index),
            &candidate,
            Limits::default(),
            &mut budget,
            &mut statistics
        ),
        Err(Incomplete::WorkLimit)
    ));
    assert_eq!(budget.statistics, paid);
    assert_eq!(statistics.countermodel_queries, 1);
    assert_eq!(statistics.reduct.regions.work, 0);
}

#[test]
fn standalone_membership_charges_its_one_index() {
    let theory = cycle();
    let candidate = Interpretation::new(&theory, []).unwrap();
    let required = nodes(&theory);
    let cancellation = Cancellation::default();
    for (ceiling, builds) in [(required - 1, 0), (required, 1)] {
        let mut budget = budget(&cancellation, ceiling);
        let mut statistics = Statistics::default();
        // One unit short of the index refuses before building; exactly the
        // index builds it and refuses the query's first read.
        assert!(matches!(
            fresh_membership(
                &theory,
                &candidate,
                SearchMethod::Regions,
                Limits::default(),
                &mut budget,
                &mut statistics,
                &mut crate::ferraris::reduct_query::Workspace::default(),
            ),
            Err(Incomplete::WorkLimit)
        ));
        assert_eq!(hooks::builds(&theory), builds);
        assert_eq!(budget.statistics.work, required * builds as u64);
        assert_eq!(statistics.reduct.regions.work, required * builds as u64);
    }
}

#[test]
fn cancelled_lent_membership_performs_no_query_work() {
    let theory = cycle();
    let candidate = Interpretation::new(&theory, []).unwrap();
    let index = built(&theory);
    let mut state = State::new(SearchMethod::Regions);
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, 100);
    let mut statistics = Statistics::default();
    cancellation.cancel();
    assert!(matches!(
        state.check(
            &theory,
            Some(&index),
            &candidate,
            Limits::default(),
            &mut budget,
            &mut statistics
        ),
        Err(Incomplete::Cancelled)
    ));
    assert_eq!(budget.statistics, SearchStatistics::default());
    assert_eq!(statistics, Statistics::default());
}

#[test]
fn a_failed_region_query_records_growth_without_replacing_its_error() {
    let theory = cycle();
    let candidate = Interpretation::new(&theory, []).unwrap();
    let index = built(&theory);
    let mut state = State::new(SearchMethod::Regions);
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, 0);
    let mut statistics = Statistics::default();
    // Truth fits exactly. Scratch preparation grows the pending mask before
    // the query's first charged read is refused by the work ceiling.
    let limit = u64::try_from(state.workspace.retained_bytes()).unwrap() + nodes(&theory);
    assert!(matches!(
        state.check(
            &theory,
            Some(&index),
            &candidate,
            Limits {
                max_reduct_bytes: limit,
                ..Limits::default()
            },
            &mut budget,
            &mut statistics,
        ),
        Err(Incomplete::WorkLimit)
    ));
    let retained = state.workspace.retained_bytes();
    assert!(retained > u128::from(limit));
    assert_eq!(statistics.reduct.peak_workspace_bytes, retained);
}

mod batch_control;
