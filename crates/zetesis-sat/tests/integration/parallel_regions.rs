//! A coordinator-first region walk returns exactly the scalar walk's stable
//! models, once each. Their order is the scalar walk's until engagement and
//! the schedule's afterwards; restrictions narrow the remaining frontier,
//! and the coordinator and workers share one ceiling.

use crate::support::choice_theories;
use crate::support::formula_theories as theories;

use std::collections::BTreeSet;
use std::time::Duration;

use zetesis_ferraris::{Node, Theory, TightPlanLimits};
use zetesis_sat::{
    Cancellation, Engagement, Incomplete, Limits, RegionWalkEngagement, SearchLimits, SearchMethod,
    StableModels,
};
use zetesis_test_support::{counts::nonzero as workers, harness};

use crate::support::interpretations::models as family;
use choice_theories::{choices, theory_over};
use theories::mixed;

/// The routes a claim that does not depend on the schedule runs on: workers
/// started at the first pull, and a walk that stays on the calling thread.
const ROUTES: [Engagement; 2] = [Engagement::Immediate, Engagement::After(Duration::MAX)];

/// `count` region workers that start as `engagement` says.
fn region_workers(
    theory: &Theory,
    count: usize,
    engagement: Engagement,
    limits: Limits,
    cancellation: Cancellation,
) -> StableModels {
    StableModels::with_region_workers_engaging(
        theory,
        workers(count),
        engagement,
        limits,
        cancellation,
    )
    .unwrap()
}

/// Several workers that start at the first pull: the claims about threads,
/// the model channel, stealing, joins and per-worker storage.
fn engaged(theory: &Theory, count: usize, limits: Limits) -> StableModels {
    region_workers(
        theory,
        count,
        Engagement::Immediate,
        limits,
        Cancellation::default(),
    )
}

/// The walk's record, once it has begun.
fn record(search: &StableModels) -> RegionWalkEngagement {
    search
        .statistics()
        .regions
        .unwrap()
        .workers
        .unwrap()
        .engagement
}

/// The walk ran on the route `engagement` names: every configured worker
/// started, or none.
fn assert_route(search: &StableModels, engagement: Engagement, count: usize) {
    match (engagement, record(search)) {
        (Engagement::Immediate, RegionWalkEngagement::Engaged { launched, .. }) => {
            assert_eq!(launched, count);
        }
        (Engagement::After(Duration::MAX), RegionWalkEngagement::Unengaged { .. }) => {}
        (engagement, record) => panic!("{engagement:?} ran as {record:?}"),
    }
}

#[test]
fn four_workers_return_the_scalar_family_once_each() {
    for theory in [mixed(), choices(6)] {
        let mut scalar = StableModels::with_method(
            &theory,
            SearchMethod::Regions,
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let expected: BTreeSet<Vec<usize>> = family(&mut scalar).into_iter().collect();
        let mut parallel = engaged(&theory, 4, Limits::default());
        let found = family(&mut parallel);
        assert!(parallel.exhausted());
        assert_route(&parallel, Engagement::Immediate, 4);
        let distinct: BTreeSet<Vec<usize>> = found.iter().cloned().collect();
        assert_eq!(distinct.len(), found.len(), "each model once");
        assert_eq!(distinct, expected);
        let statistics = parallel.statistics();
        assert_eq!(
            statistics.stable_models,
            u64::try_from(found.len()).unwrap()
        );
        assert!(statistics.regions.expect("region receipts").counts.regions > 0);
        assert_eq!(statistics.candidate_queries, 0);
    }
}

#[test]
fn a_restriction_reaches_the_workers_for_what_they_have_not_visited() {
    let theory = choices(8);
    let mut parallel = engaged(&theory, 4, Limits::default());
    let first: Vec<usize> = parallel.next().unwrap().unwrap().atoms().collect();
    assert_route(&parallel, Engagement::Immediate, 4);
    let restriction = theory_over(&theory, vec![Node::atom(7)], vec![0]);
    parallel.restrict_candidates(&restriction).unwrap();
    let rest = family(&mut parallel);
    assert!(parallel.exhausted());
    // Models already sent before the restriction may still arrive; every
    // model the workers narrow afterwards holds atom 7, and no model
    // repeats.
    let mut all = vec![first];
    all.extend(rest);
    let distinct: BTreeSet<&Vec<usize>> = all.iter().collect();
    assert_eq!(distinct.len(), all.len());
    let without: usize = all.iter().filter(|model| !model.contains(&7)).count();
    assert!(
        without <= 4 * 16 + 1,
        "at most the channel's slack lacked the restriction"
    );
    assert!(all.iter().filter(|model| model.contains(&7)).count() >= 1);
}

#[test]
fn a_restriction_charges_its_indexing_to_the_shared_search_work() {
    // The walk begins on the first pull, so before it the only work between
    // the two readings is the restriction's indexing, which the shared
    // allowance is charged and the region receipts count.
    let theory = choices(8);
    for engagement in ROUTES {
        let mut parallel = region_workers(
            &theory,
            4,
            engagement,
            Limits::default(),
            Cancellation::default(),
        );
        let before = parallel.statistics();
        let restriction = theory_over(&theory, vec![Node::atom(7)], vec![0]);
        parallel.restrict_candidates(&restriction).unwrap();
        let after = parallel.statistics();
        let indexed = after.regions.unwrap().counts.work - before.regions.unwrap().counts.work;
        assert!(indexed > 0);
        assert_eq!(after.search.work - before.search.work, indexed);
        assert_eq!(record(&parallel), RegionWalkEngagement::NotBegun);
    }
}

#[test]
fn a_restriction_beyond_the_shared_search_work_is_refused() {
    let theory = choices(8);
    for engagement in ROUTES {
        let construction = region_workers(
            &theory,
            4,
            engagement,
            Limits::default(),
            Cancellation::default(),
        )
        .statistics()
        .search
        .work;
        let limits = Limits {
            search: SearchLimits {
                max_work: construction,
                ..SearchLimits::default()
            },
            ..Limits::default()
        };
        let mut parallel = region_workers(&theory, 4, engagement, limits, Cancellation::default());
        let restriction = theory_over(&theory, vec![Node::atom(7)], vec![0]);
        assert!(matches!(
            parallel.restrict_candidates(&restriction),
            Err(Incomplete::WorkLimit)
        ));
        assert_eq!(parallel.statistics().candidate_restrictions, 0);
        assert_eq!(record(&parallel), RegionWalkEngagement::NotBegun);
    }
}

#[test]
fn a_shared_work_ceiling_stops_every_worker_without_exhaustion() {
    let theory = choices(10);
    let construction = engaged(&theory, 4, Limits::default())
        .statistics()
        .search
        .work;
    let limits = Limits {
        search: SearchLimits {
            max_work: construction + 200,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let mut parallel = engaged(&theory, 4, limits);
    let outcomes: Vec<_> = parallel.by_ref().collect();
    assert!(matches!(outcomes.last(), Some(Err(Incomplete::WorkLimit))));
    assert!(!parallel.exhausted());
    assert_route(&parallel, Engagement::Immediate, 4);
}

#[test]
fn a_candidate_ceiling_delivers_the_admitted_leaves_before_it_stops() {
    // Two independent choices leave four leaves for sixteen workers. One
    // leaf is admitted under the ceiling; the workers refused by the
    // ceiling raise the stop while that leaf is still being decided, and
    // its model must arrive before the stop does. Repeated, because the
    // interleaving is the schedule's.
    let theory = choices(2);
    let limits = Limits {
        max_candidates: 1,
        ..Limits::default()
    };
    for _ in 0..20 {
        let mut parallel = engaged(&theory, 16, limits);
        let outcomes: Vec<_> = parallel.by_ref().collect();
        assert_route(&parallel, Engagement::Immediate, 16);
        assert_eq!(outcomes.len(), 2, "{outcomes:?}");
        assert!(outcomes[0].is_ok(), "{outcomes:?}");
        assert!(matches!(outcomes[1], Err(Incomplete::CandidateLimit)));
        assert!(parallel.next().is_none());
        assert!(!parallel.exhausted());
        assert_eq!(parallel.statistics().candidates, 1);
        assert_eq!(parallel.statistics().stable_models, 1);
    }
}

#[test]
fn a_tight_certificate_decides_the_workers_leaves() {
    let theory = choices(5);
    let mut parallel = engaged(&theory, 3, Limits::default());
    assert!(
        parallel
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let outcomes: Vec<_> = parallel.by_ref().collect();
    assert_route(&parallel, Engagement::Immediate, 3);
    let found: Vec<Vec<usize>> = outcomes
        .into_iter()
        .map(|model| match model {
            Ok(model) => model.atoms().collect::<Vec<_>>(),
            Err(error) => panic!("{error:?}"),
        })
        .collect();
    assert_eq!(found.len(), 32);
    assert!(parallel.exhausted());
    assert_eq!(parallel.statistics().countermodel_queries, 0);
}

#[test]
fn phase_timings_sum_the_workers_narrowing_and_leaf_decisions() {
    let theory = choices(6);
    let mut parallel = engaged(&theory, 4, Limits::default());
    parallel.enable_phase_timing();
    assert_eq!(family(&mut parallel).len(), 64);
    assert_route(&parallel, Engagement::Immediate, 4);
    let timings = parallel.statistics().phase_timings.unwrap();
    // Every leaf is decided by the reduct query once, in some worker; every
    // region is narrowed once; no certificate was enabled.
    assert_eq!(timings.reduct.calls, 64);
    assert!(timings.candidates.calls >= 64);
    assert_eq!(timings.certified.calls, 0);
    assert!(timings.reduct.elapsed > std::time::Duration::ZERO);
}

#[test]
fn a_timed_parallel_walk_reports_its_index_build_as_candidate_generation() {
    let theory = choices(6);
    for engagement in ROUTES {
        let mut parallel = region_workers(
            &theory,
            4,
            engagement,
            Limits::default(),
            Cancellation::default(),
        );
        parallel.enable_phase_timing();
        assert_eq!(family(&mut parallel).len(), 64);
        assert_route(&parallel, engagement, 4);
        let statistics = parallel.statistics();
        let regions = statistics.regions.unwrap().counts.regions;
        // The coordinator and each worker time one candidate call per region
        // they narrow; the index build, before the walk begins, is one more.
        assert_eq!(
            statistics.phase_timings.unwrap().candidates.calls,
            u64::try_from(regions).unwrap() + 1
        );
    }
}

#[test]
fn four_workers_report_the_scalar_walks_reading_work() {
    // The workers walk the tree the scalar walk walks, so the regions and
    // leaves they report are its, and so is the reading work, counted once:
    // a worker's narrowing work goes to the live counters as it goes and
    // is not added again when the workers are joined.
    for theory in [mixed(), choices(6)] {
        let mut scalar = StableModels::with_method(
            &theory,
            SearchMethod::Regions,
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let models = family(&mut scalar).len();
        let mut parallel = engaged(&theory, 4, Limits::default());
        assert_eq!(family(&mut parallel).len(), models);
        assert_route(&parallel, Engagement::Immediate, 4);
        let one = scalar.statistics().regions.unwrap();
        let four = parallel.statistics().regions.unwrap();
        assert_eq!(four.counts.regions, one.counts.regions);
        assert_eq!(four.counts.leaves, one.counts.leaves);
        assert_eq!(four.counts.work, one.counts.work);
    }
}

#[test]
fn an_unengaged_walk_reports_the_scalar_walks_receipts() {
    // The walk that never engages steps the scalar walk's order on the
    // calling thread with one retained workspace, so every receipt is the
    // scalar walk's, checking work and peak check storage included.
    for (theory, certified) in [(mixed(), false), (choices(6), false), (choices(5), true)] {
        let mut scalar = StableModels::with_method(
            &theory,
            SearchMethod::Regions,
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let unengaged = Engagement::After(Duration::MAX);
        let mut walk = region_workers(
            &theory,
            4,
            unengaged,
            Limits::default(),
            Cancellation::default(),
        );
        if certified {
            for search in [&mut scalar, &mut walk] {
                assert!(
                    search
                        .enable_certified_checking(TightPlanLimits::default())
                        .unwrap()
                );
            }
        }
        let expected = family(&mut scalar);
        let found = family(&mut walk);
        assert_eq!(found, expected, "the scalar walk's model sequence");
        assert_eq!(
            found.iter().collect::<BTreeSet<_>>().len(),
            found.len(),
            "each model once"
        );
        assert!(scalar.exhausted() && walk.exhausted());
        assert_route(&walk, unengaged, 4);
        let (one, coordinator) = (scalar.statistics(), walk.statistics());
        assert_eq!(
            (
                coordinator.candidates,
                coordinator.countermodel_queries,
                coordinator.countermodels,
                coordinator.stable_models,
            ),
            (
                one.candidates,
                one.countermodel_queries,
                one.countermodels,
                one.stable_models,
            )
        );
        assert_eq!(coordinator.certified, one.certified);
        assert_eq!(coordinator.reduct, one.reduct);
        assert_eq!(
            coordinator.regions.unwrap().counts,
            one.regions.unwrap().counts
        );
        assert_eq!(coordinator.search, one.search);
    }
}

#[test]
fn region_workers_report_nonzero_countermodel_workspace() {
    // A parallel walk that ran countermodel queries must retain their peak
    // workspace receipt when its worker reports are merged.
    let mut parallel = engaged(&mixed(), 4, Limits::default());
    family(&mut parallel);
    assert_route(&parallel, Engagement::Immediate, 4);
    let statistics = parallel.statistics();
    assert!(statistics.countermodel_queries > 0);
    assert!(statistics.reduct.peak_workspace_bytes > 0);
}

#[test]
fn a_determined_candidate_run_never_begins_the_walk() {
    // A positive program has one possible answer set, its least model, which
    // the certificate decides without a candidate walk.
    let theory = theory_over(
        &choices(2),
        vec![Node::atom(0), Node::atom(1), Node::implies(0, 1)],
        vec![0, 2],
    );
    let mut search = region_workers(
        &theory,
        4,
        Engagement::Immediate,
        Limits::default(),
        Cancellation::default(),
    );
    assert!(
        search
            .enable_class_checking(
                zetesis_sat::CertificateLimits::default(),
                zetesis_sat::CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
    assert_eq!(family(&mut search), vec![vec![0, 1]]);
    assert!(search.exhausted());
    assert_eq!(record(&search), RegionWalkEngagement::NotBegun);
}

#[test]
fn one_worker_returns_the_scalar_walks_sequence() {
    // The sequence, not only the family: the scalar walk returns the models
    // in the tree's order, held branch first. A multi-worker walk keeps that
    // order only until engagement.
    let theory = mixed();
    let mut one = StableModels::with_region_workers(
        &theory,
        workers(1),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut scalar = StableModels::with_method(
        &theory,
        SearchMethod::Regions,
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    assert_eq!(family(&mut one), family(&mut scalar));
}

/// Run a possibly blocking worker lifecycle in a child, so a regression is
/// killed and reaped instead of stranding a thread in the test runner. The
/// child re-runs this module's test `name` alone.
fn bounded_child(name: &str, run: impl FnOnce()) {
    if std::env::var("ZETESIS_REGION_CHILD").as_deref() == Ok(name) {
        run();
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", harness::test_name(module_path!(), name).as_str()])
        .env("ZETESIS_REGION_CHILD", name)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let expires = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut output = String::new();
            std::io::Read::read_to_string(&mut child.stdout.take().unwrap(), &mut output).unwrap();
            assert!(
                status.success(),
                "worker lifecycle failed: {status}\n{output}"
            );
            // A name the harness does not know runs nothing and still succeeds.
            assert!(output.contains("test result: ok. 1 passed"), "{output}");
            return;
        }
        if std::time::Instant::now() >= expires {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("worker lifecycle did not finish before the deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn dropping_a_full_model_channel_joins_the_workers() {
    bounded_child("dropping_a_full_model_channel_joins_the_workers", || {
        let mut search = engaged(&choices(10), 2, Limits::default());
        assert!(search.next().unwrap().is_ok());
        assert_route(&search, Engagement::Immediate, 2);
        // One returned model, 32 channel slots and two workers holding the
        // next models: joining must release blocked sends as well as waiters.
        let expires = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while search.statistics().candidates < 35 {
            assert!(std::time::Instant::now() < expires, "channel never filled");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        drop(search);
    });
}

#[test]
fn idle_workers_return_unused_work_permits() {
    bounded_child("idle_workers_return_unused_work_permits", || {
        let theory = choices(2);
        let construction = engaged(&theory, 4, Limits::default())
            .statistics()
            .search
            .work;
        // Small allowances cross each point where one branch can finish
        // while another still needs an idle worker's unused grant.
        for remaining in 0..256 {
            let limits = Limits {
                search: SearchLimits {
                    max_work: construction + remaining,
                    ..SearchLimits::default()
                },
                ..Limits::default()
            };
            let cancellation = Cancellation::with_deadline(
                std::time::Instant::now() + std::time::Duration::from_millis(500),
            )
            .unwrap();
            let mut search =
                region_workers(&theory, 4, Engagement::Immediate, limits, cancellation);
            for result in search.by_ref() {
                assert!(
                    result.is_ok() || matches!(result, Err(Incomplete::WorkLimit)),
                    "allowance {remaining} waited instead of returning its work outcome: {result:?}"
                );
            }
            // An allowance too small to begin the walk leaves it unbegun.
            match record(&search) {
                RegionWalkEngagement::NotBegun => {}
                _ => assert_route(&search, Engagement::Immediate, 4),
            }
        }
    });
}

/// The certified family of `choices(5)`, with the walk's statistics and
/// whether it was exhausted; several workers start as `engagement` says, and
/// the route is asserted.
fn certified_family(
    worker_count: usize,
    engagement: Engagement,
    limits: Limits,
) -> (usize, zetesis_sat::Statistics, bool) {
    let mut search = region_workers(
        &choices(5),
        worker_count,
        engagement,
        limits,
        Cancellation::default(),
    );
    assert!(
        search
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let found = family(&mut search).len();
    if worker_count > 1 {
        assert_route(&search, engagement, worker_count);
    }
    (found, search.statistics(), search.exhausted())
}

#[test]
fn joined_workers_report_their_certificate_checks() {
    let (found, statistics, exhausted) =
        certified_family(3, Engagement::Immediate, Limits::default());
    assert!(exhausted);
    assert_eq!(found, 32);
    let certified = statistics.certified.unwrap();
    assert_eq!(certified.checks, 32);
    assert_eq!(certified.stable, 32);
    assert!(certified.checking_work > 0);
}

#[test]
fn parallel_certificates_charge_the_scalar_regions_and_construction() {
    // Checking work depends on what each worker checked before (its retained
    // evaluation), so only region and construction work match the scalar run.
    let (scalar_models, scalar, scalar_exhausted) =
        certified_family(1, Engagement::default(), Limits::default());
    for engagement in ROUTES {
        let (parallel_models, parallel, parallel_exhausted) =
            certified_family(3, engagement, Limits::default());
        assert!(scalar_exhausted && parallel_exhausted);
        assert_eq!((scalar_models, parallel_models), (32, 32));
        assert_eq!(
            parallel.regions.unwrap().counts.work,
            scalar.regions.unwrap().counts.work
        );
        let (scalar, parallel) = (scalar.certified.unwrap(), parallel.certified.unwrap());
        assert_eq!(parallel.construction_work, scalar.construction_work);
    }
}

#[test]
fn no_certified_check_is_refused_under_its_reservation() {
    // Region workers reserve the run's own plan bound for each certified check
    // and run the check under it; were the bound too small for a check, the
    // check would be refused and the enumeration would not complete. A single
    // worker is the scalar method, which reserves nothing, so only worker
    // counts above one apply.
    for worker_count in [2, 3] {
        for engagement in ROUTES {
            let mut search = region_workers(
                &choices(5),
                worker_count,
                engagement,
                Limits::default(),
                Cancellation::default(),
            );
            assert!(
                search
                    .enable_certified_checking(TightPlanLimits::default())
                    .unwrap()
            );
            assert_eq!(family(&mut search).len(), 32);
            assert!(search.exhausted());
            assert_route(&search, engagement, worker_count);
            let certified = search.statistics().certified.unwrap();
            assert_eq!((certified.checks, certified.stable), (32, 32));
        }
    }
}

#[test]
fn tight_check_storage_is_local_to_each_worker() {
    let (scalar_models, scalar, scalar_exhausted) =
        certified_family(1, Engagement::default(), Limits::default());
    let (parallel_models, parallel, parallel_exhausted) =
        certified_family(3, Engagement::Immediate, Limits::default());
    assert!(scalar_exhausted && parallel_exhausted);
    assert_eq!((scalar_models, parallel_models), (32, 32));
    let scalar = scalar.certified.unwrap();
    let parallel = parallel.certified.unwrap();
    assert!(matches!(
        scalar.plan,
        Some(zetesis_sat::CertificatePlanStatistics::Tight(_))
    ));
    assert!(matches!(
        parallel.plan,
        Some(zetesis_sat::CertificatePlanStatistics::Tight(_))
    ));
    assert_eq!((scalar.checks, parallel.checks), (32, 32));
    let original = choices(5);
    let Some(zetesis_sat::CertificatePlanStatistics::Tight(plan)) = scalar.plan else {
        panic!("tight certificate required");
    };
    // One workspace's footprint once it has updated an evaluation: the plan,
    // truth and support words of a complete check, then the retained candidate,
    // queued marks, support counts, unsupported atoms, false roots, the first
    // reservations of the changed-word list (8) and the worklist (64). A peak
    // summed over workers would exceed it.
    let (atoms, nodes, roots) = (
        original.atom_count(),
        original.nodes().len(),
        original.roots().len(),
    );
    let words = atoms.div_ceil(64) as u128;
    let expected = u128::from(plan.resident_bytes)
        + nodes as u128
        + atoms.div_ceil(32) as u128 * size_of::<u32>() as u128
        + (2 * words + nodes.div_ceil(64) as u128 + roots.div_ceil(64) as u128 + 8)
            * size_of::<u64>() as u128
        + atoms as u128 * size_of::<u32>() as u128
        + 64 * size_of::<usize>() as u128;
    assert_eq!(scalar.tight_check_peak_bytes, Some(expected));
    assert_eq!(parallel.tight_check_peak_bytes, Some(expected));
}

#[test]
fn certificate_work_cannot_exceed_the_shared_ceiling() {
    // Region, construction and decision work do not depend on the schedule,
    // unlike checking work, which follows each worker's retained evaluation. A
    // ceiling one unit below them alone therefore denies the enumeration
    // whatever the checks cost, and every check that starts must fit beneath it.
    let (_, scalar, _) = certified_family(1, Engagement::default(), Limits::default());
    let checking = scalar.certified.unwrap().checking_work;
    let limits = Limits {
        search: SearchLimits {
            max_work: scalar.search.work - checking - 1,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    for engagement in ROUTES {
        let mut search =
            region_workers(&choices(5), 3, engagement, limits, Cancellation::default());
        assert!(
            search
                .enable_certified_checking(TightPlanLimits::default())
                .unwrap()
        );
        let last = search.by_ref().last();
        assert!(matches!(last, Some(Err(Incomplete::WorkLimit))), "{last:?}");
        assert!(
            !search.exhausted(),
            "full enumeration requires more than the ceiling admits"
        );
        assert_route(&search, engagement, 3);
        let statistics = search.statistics();
        let certificate = statistics.certified.unwrap();
        assert!(
            certificate.construction_work
                + certificate.checking_work
                + statistics.regions.unwrap().counts.work
                + statistics.search.decisions
                <= limits.search.max_work,
            "certificate work must be bounded before execution: {statistics:?}",
        );
    }
}

#[test]
fn a_certificate_retains_its_verification_work_refusal() {
    let limits = Limits {
        max_verification_work: 1,
        ..Limits::default()
    };
    for (worker_count, engagement) in [(1, Engagement::default()), (3, ROUTES[0]), (3, ROUTES[1])] {
        let mut search = region_workers(
            &choices(2),
            worker_count,
            engagement,
            limits,
            Cancellation::default(),
        );
        assert!(
            search
                .enable_certified_checking(TightPlanLimits::default())
                .unwrap()
        );
        let last = search.by_ref().last();
        assert!(
            matches!(
                last,
                Some(Err(Incomplete::Certificate(
                    zetesis_sat::CertificateError::Tight(zetesis_ferraris::TightError::Limit(
                        zetesis_ferraris::TightResource::Work
                    ),)
                ))),
            ),
            "{last:?}"
        );
        assert!(!search.exhausted());
        if worker_count > 1 {
            assert_route(&search, engagement, worker_count);
        }
    }
}

#[test]
fn certificate_configuration_precedes_region_candidates() {
    for (worker_count, engagement) in [(1, Engagement::default()), (3, ROUTES[0]), (3, ROUTES[1])] {
        let theory = choices(2);
        let mut before = region_workers(
            &theory,
            worker_count,
            engagement,
            Limits::default(),
            Cancellation::default(),
        );
        assert!(
            before
                .enable_certified_checking(TightPlanLimits::default())
                .unwrap()
        );
        assert_eq!(family(&mut before).len(), 4);
        if worker_count > 1 {
            assert_route(&before, engagement, worker_count);
        }

        let mut after = region_workers(
            &theory,
            worker_count,
            engagement,
            Limits::default(),
            Cancellation::default(),
        );
        assert!(after.next().unwrap().is_ok());
        assert_eq!(
            after.enable_certified_checking(TightPlanLimits::default()),
            Err(Incomplete::LateCertificate),
        );
        assert!(after.statistics().certified.is_none());
        assert_eq!(family(&mut after).len(), 3);
        if worker_count > 1 {
            assert_route(&after, engagement, worker_count);
        }
    }
}

#[test]
fn restrictions_after_certificate_setup_charge_the_shared_work() {
    let theory = choices(3);
    for engagement in ROUTES {
        let mut search = region_workers(
            &theory,
            3,
            engagement,
            Limits::default(),
            Cancellation::default(),
        );
        assert!(
            search
                .enable_certified_checking(TightPlanLimits::default())
                .unwrap()
        );
        let before = search.statistics().search.work;
        let restriction = theory_over(&theory, vec![Node::atom(2)], vec![0]);
        search.restrict_candidates(&restriction).unwrap();
        assert_eq!(search.statistics().search.work, before + 1);
        assert_eq!(family(&mut search).len(), 4);
        assert_route(&search, engagement, 3);
        let final_statistics = search.statistics();
        let certificate = final_statistics.certified.unwrap();
        assert_eq!(
            final_statistics.search.work,
            final_statistics.regions.unwrap().counts.work
                + final_statistics.search.decisions
                + certificate.construction_work
                + certificate.checking_work,
        );
    }
}
