//! Exact subject ownership and build accounting for the shared original index.

use std::num::NonZeroUsize;
use std::time::Duration;

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
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node};

thread_local! {
    // Counts actual subject invocations, including closures charging no work.
    static NARROWING_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn record_narrowing() {
    NARROWING_ATTEMPTS.set(NARROWING_ATTEMPTS.get() + 1);
}

/// `a <- b`, `b <- a`: the classical models are `{}` and `{a, b}`, and only
/// `{}` is stable; frozen-positive closure rejects the self-supported `{a, b}`.
fn cycle() -> Theory {
    Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(
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
    .unwrap()
}

/// The fact `a` and `b <- a`: a positive program whose least model `{a, b}`
/// is its one answer set, decided by a positive certificate.
fn positive() -> Theory {
    Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(
            vec![Node::atom(0), Node::atom(1), Node::implies(0, 1)],
            vec![],
        )
        .unwrap(),
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

#[test]
fn support_attribution_overflow_leaves_counts_unchanged() {
    for (support_work, founded_work) in [(u64::MAX, 0), (0, u64::MAX)] {
        let mut counts = RegionCounts {
            support_work,
            founded_work,
            ..RegionCounts::default()
        };
        let before = counts;
        assert_eq!(
            counts.add(RegionCounts {
                support_work: 1,
                founded_work: 1,
                ..RegionCounts::default()
            }),
            Err(Incomplete::CounterOverflow)
        );
        assert_eq!(counts, before);
    }
}

fn indexing_work(theory: &Theory) -> u64 {
    u64::try_from(theory.nodes().len() + theory.parts().occurrences()).unwrap()
}

fn extraction(theory: &Theory) -> u64 {
    zetesis_ferraris::producers(theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .work
}

// These test-only methods keep membership and handoff regressions on the
// classical-candidate route even when optional producer pruning can decide the
// fixture earlier. The original theory, index identity and choice marks remain.
impl OriginalParts {
    pub(in crate::ferraris) fn omit_producers_for_membership_test(&mut self) {
        self.producers = None;
    }
}

impl RegionSearch {
    pub(in crate::ferraris) fn omit_producers_for_membership_test(&mut self) {
        assert_eq!(
            self.statistics.counts.regions, 0,
            "before the first narrowing"
        );
        self.parts.omit_producers_for_membership_test();
        self.statistics.producers = false;
    }
}

impl IndexedTheory {
    pub(crate) fn for_membership_test(theory: &Theory) -> Arc<Self> {
        built(theory)
    }
}

fn built(theory: &Theory) -> Arc<IndexedTheory> {
    Arc::clone(
        OriginalIndex::new(theory, u64::MAX)
            .ensure(&crate::Cancellation::default(), |_| Ok(()))
            .unwrap(),
    )
}

/// Every region route, by name: scalar, parallel workers started at the
/// first pull, a parallel walk that stays on the calling thread, and
/// producers.
const ROUTES: [(&str, usize, bool); 7] = [
    ("scalar", 1, false),
    ("parallel", 2, false),
    ("parallel", 4, false),
    ("coordinator", 2, false),
    ("coordinator", 4, false),
    ("producers", 2, true),
    ("producers", 4, true),
];

/// The enumeration a route of `ROUTES` names: the coordinator route never
/// engages its workers, and the parallel route starts them at the first pull.
fn enumeration(
    theory: &Theory,
    route: &str,
    workers: usize,
    producers: bool,
    limits: Limits,
) -> StableModels {
    let workers = NonZeroUsize::new(workers).unwrap();
    if producers {
        StableModels::with_region_producers(theory, workers, limits, Cancellation::default())
    } else {
        let engagement = if route == "coordinator" {
            super::super::Engagement::After(Duration::MAX)
        } else {
            super::super::Engagement::Immediate
        };
        StableModels::with_region_workers_engaging(
            theory,
            workers,
            engagement,
            limits,
            Cancellation::default(),
        )
    }
    .unwrap()
}

/// The theory over three atoms with the formula nodes `nodes` and the roots
/// `roots`, under the default admission limits.
fn theory(nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        3,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn an_enumeration_builds_one_original_index() {
    // The candidate walk builds one original index. Possible-founded closure
    // cuts the positive cycle before membership, leaving the empty candidate.
    // Its frozen-positive check shares that index and builds no subset regions.
    for (route, workers, producers) in ROUTES {
        let theory = cycle();
        let cancellation = Cancellation::default();
        let extraction =
            zetesis_ferraris::producers(&theory, RegionLimits::default(), &cancellation).unwrap();
        let narrower = Narrower::new(&theory);
        let mut region = Region::all_open(theory.atom_count());
        let (closed, direct) = narrower
            .narrow_known(
                OriginalSubject::new(&theory, extraction.producers.as_ref())
                    .with_choices(&extraction.choice_heads),
                &mut region,
                &mut narrower.knowledge(),
                &mut NarrowingScratch::default(),
                RegionLimits::default(),
                &cancellation,
            )
            .unwrap();
        assert!(matches!(closed, Narrowing::Fixed { .. }));
        assert_eq!(region.open().count(), 0);
        assert_eq!(region.held().count(), 0);
        // The independent core narrowing receipt plus extraction and one N+E
        // index charge is the exact integration receipt on every route.
        let region_work = extraction.work + indexing_work(&theory) + direct.work;
        let mut search = enumeration(&theory, route, workers, producers, Limits::default());
        assert_eq!(
            hooks::builds(&theory),
            0,
            "{route}: construction builds none"
        );
        let models: Vec<_> = search.by_ref().map(Result::unwrap).collect();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].atoms().count(), 0);
        assert!(search.exhausted());
        let statistics = search.statistics();
        assert_eq!(statistics.countermodel_queries, 1, "{route}");
        assert_eq!(hooks::builds(&theory), 1, "{route} with {workers} workers");
        let counts = statistics.regions.unwrap().counts;
        assert_eq!(counts.regions, 1, "{route}");
        assert_eq!(counts.leaves, 1, "{route}");
        assert_eq!(counts.work, region_work, "{route}");
        assert_eq!(statistics.reduct.regions.work, 0, "{route}");
        assert_eq!(statistics.reduct.frozen_positive_attempts, 1, "{route}");
        assert_eq!(statistics.reduct.frozen_positive_completed, 1, "{route}");
        assert_eq!(statistics.countermodels, 0, "{route}");
        assert!(
            statistics.reduct.frozen_positive_preparation_work > 0,
            "{route}"
        );
        // No split or proper-subset witness adds work outside these receipts.
        assert_eq!(statistics.search.decisions, 0, "{route}");
        assert_eq!(
            statistics.search.work,
            region_work + statistics.reduct.frozen_positive_work,
            "{route} with {workers} workers"
        );
    }
}

#[test]
fn a_positive_certificate_route_builds_no_index() {
    for (route, workers, producers) in ROUTES {
        let theory = positive();
        let mut search = enumeration(&theory, route, workers, producers, Limits::default());
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
        let mut search = enumeration(&theory, route, workers, producers, Limits::default());
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
                max_work: setup + indexing_work(&theory) - 1,
                ..SearchLimits::default()
            },
            ..Limits::default()
        };
        let mut search = enumeration(&theory, route, workers, producers, limits);
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
        let mut search = enumeration(&theory, route, workers, producers, Limits::default());
        hooks::fail_next_build();
        assert!(matches!(search.next(), Some(Err(Incomplete::Allocation))));
        assert!(!search.exhausted(), "{route}");
        assert!(search.index.get().is_none(), "{route}");
        assert_eq!(hooks::builds(&theory), 1, "{route} with {workers} workers");
        let statistics = search.statistics();
        let charged = extraction(&theory) + indexing_work(&theory);
        assert_eq!(statistics.search.work, charged, "{route}");
        assert_eq!(statistics.regions.unwrap().counts.work, charged, "{route}");
    }
}

#[test]
fn a_refused_charge_leaves_the_owner_unbuilt() {
    let theory = cycle();
    let mut owner = OriginalIndex::new(&theory, u64::MAX);
    let mut asked = None;
    assert_eq!(
        owner
            .ensure(&crate::Cancellation::default(), |work| {
                asked = Some(work);
                Err(Incomplete::WorkLimit)
            })
            .map(|_| ()),
        Err(Incomplete::WorkLimit)
    );
    // The charge is the index's documented cost, known before building.
    assert_eq!(asked, Some(indexing_work(&theory)));
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
    let mut owner = OriginalIndex::new(&theory, u64::MAX);
    owner
        .ensure(&crate::Cancellation::default(), |work| budget.charge(work))
        .unwrap();
    let paid = budget.statistics;
    assert_eq!(paid.work, indexing_work(&theory));
    budget.limits.max_work = paid.work;
    let index = Arc::clone(
        owner
            .ensure(&crate::Cancellation::default(), |_| {
                panic!("a built index is never charged again")
            })
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
    let required = indexing_work(&theory);
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
    let mut workspace = crate::ReductWorkspace::default();
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, 0);
    let mut statistics = Statistics::default();
    // Exercise the general query directly: an optional closure certificate can
    // now refuse before any narrowing scratch grows. Truth fits exactly; this
    // query grows its pending mask before its first charged read is refused.
    let limit = u64::try_from(workspace.retained_bytes()).unwrap() + indexing_work(&theory);
    let limits = Limits {
        max_reduct_bytes: limit,
        ..Limits::default()
    };
    let result = (|| {
        let evaluated = workspace.evaluate(&candidate, limits, &cancellation, &mut statistics)?;
        ReductQuery::new(&index).check(
            zetesis_ferraris::FrozenSubject::new(&theory, evaluated.truth.truth()),
            &candidate,
            limits,
            &mut budget,
            &mut statistics,
            evaluated.narrowing,
        )
    })();
    assert!(matches!(
        workspace.finish_check(result, limit, &mut statistics),
        Err(Incomplete::WorkLimit)
    ));
    let retained = workspace.retained_bytes();
    assert!(retained > u128::from(limit));
    assert_eq!(statistics.reduct.peak_workspace_bytes, retained);
}

mod batch_control;
mod composition;
mod generator_order;
mod ordering;
mod preference;

#[test]
fn failed_narrowing_keeps_support_explanation_receipts() {
    let explanations = SupportExplanationStatistics {
        attempts: 2,
        certified: 1,
        matched: 3,
        cut: 4,
        forced: 6,
        refuted: 1,
        declined: 1,
        work: 19,
        peak_bytes: 43,
    };
    let attempt = NarrowingAttempt {
        result: Err(Incomplete::WorkLimit),
        statistics: zetesis_ferraris::NarrowingStatistics {
            work: 31,
            support_explanations: explanations,
            ..zetesis_ferraris::NarrowingStatistics::default()
        },
    };
    let mut counts = RegionCounts::default();
    assert_eq!(account(&attempt, &mut counts), Err(Incomplete::WorkLimit));
    assert_eq!(counts.support_explanations, explanations);
    assert_eq!(counts.work, 31, "explanation work is already included");
}

#[test]
fn explanation_overflow_leaves_all_counts_unchanged() {
    for field in 0..8 {
        let mut counts = RegionCounts::default();
        let counter = match field {
            0 => &mut counts.support_explanations.attempts,
            1 => &mut counts.support_explanations.certified,
            2 => &mut counts.support_explanations.matched,
            3 => &mut counts.support_explanations.cut,
            4 => &mut counts.support_explanations.refuted,
            5 => &mut counts.support_explanations.declined,
            6 => &mut counts.support_explanations.forced,
            _ => &mut counts.support_explanations.work,
        };
        *counter = u64::MAX;
        let before = counts;
        assert_eq!(
            counts.add(RegionCounts {
                work: 17,
                support_explanations: SupportExplanationStatistics {
                    attempts: 1,
                    certified: 1,
                    matched: 1,
                    cut: 1,
                    forced: 1,
                    refuted: 1,
                    declined: 1,
                    work: 1,
                    peak_bytes: 43,
                },
                ..RegionCounts::default()
            }),
            Err(Incomplete::CounterOverflow)
        );
        assert_eq!(counts, before);
    }
}
