//! Several workers walking the region tree return exactly the stable
//! models one worker returns, once each, in an order that is the
//! schedule's and not a property of the result; a restriction narrows what
//! the workers have not yet visited; a shared ceiling stops them all.

use crate::support::choice_theories;
use crate::support::formula_theories as theories;

use std::collections::BTreeSet;

use zetesis_ferraris::{Node, TightPlanLimits};
use zetesis_sat::{Cancellation, Incomplete, Limits, SearchLimits, SearchMethod, StableModels};
use zetesis_test_support::{counts::nonzero as workers, harness};

use crate::support::interpretations::models as family;
use choice_theories::{choices, theory_over};
use theories::mixed;

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
        let mut parallel = StableModels::with_region_workers(
            &theory,
            workers(4),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let found = family(&mut parallel);
        assert!(parallel.exhausted());
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
    let mut parallel = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let first: Vec<usize> = parallel.next().unwrap().unwrap().atoms().collect();
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
    // The workers start on the first proposal, so before it the only work
    // between the two readings is the restriction's indexing, which the
    // shared allowance is charged and the region receipts count.
    let theory = choices(8);
    let mut parallel = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let before = parallel.statistics();
    let restriction = theory_over(&theory, vec![Node::atom(7)], vec![0]);
    parallel.restrict_candidates(&restriction).unwrap();
    let after = parallel.statistics();
    let indexed = after.regions.unwrap().counts.work - before.regions.unwrap().counts.work;
    assert!(indexed > 0);
    assert_eq!(after.search.work - before.search.work, indexed);
}

#[test]
fn a_restriction_beyond_the_shared_search_work_is_refused() {
    let theory = choices(8);
    let construction = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap()
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
    let mut parallel =
        StableModels::with_region_workers(&theory, workers(4), limits, Cancellation::default())
            .unwrap();
    let restriction = theory_over(&theory, vec![Node::atom(7)], vec![0]);
    assert!(matches!(
        parallel.restrict_candidates(&restriction),
        Err(Incomplete::WorkLimit)
    ));
    assert_eq!(parallel.statistics().candidate_restrictions, 0);
}

#[test]
fn a_shared_work_ceiling_stops_every_worker_without_exhaustion() {
    let theory = choices(10);
    let construction = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap()
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
    let mut parallel =
        StableModels::with_region_workers(&theory, workers(4), limits, Cancellation::default())
            .unwrap();
    let outcomes: Vec<_> = parallel.by_ref().collect();
    assert!(matches!(outcomes.last(), Some(Err(Incomplete::WorkLimit))));
    assert!(!parallel.exhausted());
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
        let mut parallel = StableModels::with_region_workers(
            &theory,
            workers(16),
            limits,
            Cancellation::default(),
        )
        .unwrap();
        let outcomes: Vec<_> = parallel.by_ref().collect();
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
    let mut parallel = StableModels::with_region_workers(
        &theory,
        workers(3),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    assert!(
        parallel
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let outcomes: Vec<_> = parallel.by_ref().collect();
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
    let mut parallel = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    parallel.enable_phase_timing();
    assert_eq!(family(&mut parallel).len(), 64);
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
    let mut parallel = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    parallel.enable_phase_timing();
    assert_eq!(family(&mut parallel).len(), 64);
    let statistics = parallel.statistics();
    let regions = statistics.regions.unwrap().counts.regions;
    // Each worker times one candidate call per region it narrows; the
    // coordinator's index build, before the workers launch, is one more.
    assert_eq!(
        statistics.phase_timings.unwrap().candidates.calls,
        u64::try_from(regions).unwrap() + 1
    );
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
        let mut parallel = StableModels::with_region_workers(
            &theory,
            workers(4),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        assert_eq!(family(&mut parallel).len(), models);
        let one = scalar.statistics().regions.unwrap();
        let four = parallel.statistics().regions.unwrap();
        assert_eq!(four.counts.regions, one.counts.regions);
        assert_eq!(four.counts.leaves, one.counts.leaves);
        assert_eq!(four.counts.work, one.counts.work);
    }
}

#[test]
fn one_worker_returns_the_scalar_walks_sequence() {
    // The sequence, not only the family: the scalar walk returns the models
    // in the tree's order, cut branch first, which the parallel walk does
    // not promise.
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
        let mut search = StableModels::with_region_workers(
            &choices(10),
            workers(2),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        assert!(search.next().unwrap().is_ok());
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
        let construction = StableModels::with_region_workers(
            &theory,
            workers(4),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap()
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
                StableModels::with_region_workers(&theory, workers(4), limits, cancellation)
                    .unwrap();
            for result in search.by_ref() {
                assert!(
                    result.is_ok() || matches!(result, Err(Incomplete::WorkLimit)),
                    "allowance {remaining} waited instead of returning its work outcome: {result:?}"
                );
            }
        }
    });
}

fn certified_family(worker_count: usize, limits: Limits) -> (usize, zetesis_sat::Statistics, bool) {
    let mut search = StableModels::with_region_workers(
        &choices(5),
        workers(worker_count),
        limits,
        Cancellation::default(),
    )
    .unwrap();
    assert!(
        search
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let found = family(&mut search).len();
    (found, search.statistics(), search.exhausted())
}

#[test]
fn joined_workers_report_their_certificate_checks() {
    let (found, statistics, exhausted) = certified_family(3, Limits::default());
    assert!(exhausted);
    assert_eq!(found, 32);
    let certified = statistics.certified.unwrap();
    assert_eq!(certified.checks, 32);
    assert_eq!(certified.stable, 32);
    assert!(certified.checking_work > 0);
}

#[test]
fn parallel_certificates_charge_the_scalar_work() {
    let (scalar_models, scalar, scalar_exhausted) = certified_family(1, Limits::default());
    let (parallel_models, parallel, parallel_exhausted) = certified_family(3, Limits::default());
    assert!(scalar_exhausted && parallel_exhausted);
    assert_eq!((scalar_models, parallel_models), (32, 32));
    assert_eq!(
        parallel.regions.unwrap().counts.work,
        scalar.regions.unwrap().counts.work
    );
    assert_eq!(
        parallel.certified.unwrap().construction_work,
        scalar.certified.unwrap().construction_work,
    );
    assert_eq!(parallel.search.work, scalar.search.work);
}

#[test]
fn certificate_work_cannot_exceed_the_shared_ceiling() {
    let (_, scalar, _) = certified_family(1, Limits::default());
    let limits = Limits {
        search: SearchLimits {
            max_work: scalar.search.work - 1,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let mut search =
        StableModels::with_region_workers(&choices(5), workers(3), limits, Cancellation::default())
            .unwrap();
    assert!(
        search
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let last = search.by_ref().last();
    assert!(matches!(last, Some(Err(Incomplete::WorkLimit))), "{last:?}");
    assert!(
        !search.exhausted(),
        "full enumeration requires the missing certificate work"
    );
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

#[test]
fn a_certificate_retains_its_verification_work_refusal() {
    let limits = Limits {
        max_verification_work: 1,
        ..Limits::default()
    };
    for worker_count in [1, 3] {
        let mut search = StableModels::with_region_workers(
            &choices(2),
            workers(worker_count),
            limits,
            Cancellation::default(),
        )
        .unwrap();
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
    }
}

#[test]
fn certificate_configuration_precedes_region_candidates() {
    for worker_count in [1, 3] {
        let theory = choices(2);
        let mut before = StableModels::with_region_workers(
            &theory,
            workers(worker_count),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        assert!(
            before
                .enable_certified_checking(TightPlanLimits::default())
                .unwrap()
        );
        assert_eq!(family(&mut before).len(), 4);

        let mut after = StableModels::with_region_workers(
            &theory,
            workers(worker_count),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        assert!(after.next().unwrap().is_ok());
        assert_eq!(
            after.enable_certified_checking(TightPlanLimits::default()),
            Err(Incomplete::LateCertificate),
        );
        assert!(after.statistics().certified.is_none());
        assert_eq!(family(&mut after).len(), 3);
    }
}

#[test]
fn restrictions_after_certificate_setup_charge_the_shared_work() {
    let theory = choices(3);
    let mut search = StableModels::with_region_workers(
        &theory,
        workers(3),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
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
