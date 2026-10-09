//! Checked strata determine one candidate without weakening enumeration boundaries.

use std::{collections::BTreeSet, convert::Infallible, num::NonZeroUsize};

use crate::support::batching::{batch, residual};
use crate::support::choice_theories::choices;
use crate::support::clause_search::by_clauses;
use crate::support::region_filters::{ROUTES, Route};
use zetesis_ferraris::{
    Interpretation, Node, StratifiedError, StratifiedPlanLimits, StratifiedResource, Theory,
    TightPlanLimits,
};
use zetesis_sat::{
    BatchError, BatchVerdict, Cancellation, CertificateLimits, CertificateOrder,
    CertificatePlanStatistics, Incomplete, Limits, StableModels,
};
use zetesis_theory_support::theories::theory;

/// a :- b. b :- a. c :- not b. d :- not c. An optional fact seeds the cycle;
/// an optional constraint forbids d. The fifth carrier atom is unmentioned.
fn strata(seed: bool, forbid_d: bool) -> Theory {
    let mut roots = vec![5, 6, 8, 10];
    if seed {
        roots.push(0);
    }
    if forbid_d {
        roots.push(11);
    }
    theory(
        5,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::falsum(),
            Node::implies(0, 1),
            Node::implies(1, 0),
            Node::implies(1, 4),
            Node::implies(7, 2),
            Node::implies(2, 4),
            Node::implies(9, 3),
            Node::implies(3, 4),
        ],
        roots,
    )
}

fn search(route: Route, original: &Theory, limits: Limits, control: Cancellation) -> StableModels {
    let workers = NonZeroUsize::new(3).unwrap();
    match route {
        Route::Scalar => StableModels::new(original, limits, control),
        Route::Producers => StableModels::with_region_producers(original, workers, limits, control),
        Route::Native => StableModels::with_region_workers(original, workers, limits, control),
    }
    .unwrap()
}

fn enable(stream: &mut StableModels) {
    assert!(
        stream
            .enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::StratifiedFirst
            )
            .unwrap()
    );
    let stats = stream.statistics().certified.unwrap();
    assert!(matches!(
        stats.plan,
        Some(CertificatePlanStatistics::Stratified(_))
    ));
    assert_eq!(stats.stratified_refusal, None);
}

fn independent(original: &Theory) -> BTreeSet<Vec<usize>> {
    (0..1usize << original.atom_count())
        .filter_map(|mask| {
            let candidate = Interpretation::new(
                original,
                (0..original.atom_count()).filter(|atom| mask & (1 << atom) != 0),
            )
            .unwrap();
            zetesis_ferraris::check(
                original,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
            .then(|| candidate.atoms().collect())
        })
        .collect()
}

#[test]
fn stratified_regions_preserve_the_complete_answer_family() {
    for route in ROUTES {
        for seed in [false, true] {
            let original = strata(seed, false);
            let mut limits = Limits {
                max_candidates: 1,
                ..Limits::default()
            };
            limits.search.max_decisions = 0;
            let mut stream = search(route, &original, limits, Cancellation::default());
            enable(&mut stream);
            let family = route.collect(&mut stream);
            assert_eq!(family, independent(&original));
            assert_eq!(family.len(), 1);
            let stats = stream.statistics();
            assert_eq!(stats.candidates, 1);
            assert_eq!(stats.stable_models, 1);
            assert_eq!(stats.certified.unwrap().checks, 1);
            assert!(
                stats
                    .certified
                    .unwrap()
                    .stratified_check_peak_bytes
                    .is_some()
            );
            assert_eq!(stats.countermodel_queries, 0);
            assert_eq!(stats.search.decisions, 0);
            assert_eq!(stats.regions.unwrap().counts.regions, 0);
            assert!(stats.reduct.preparation.is_none());
        }
    }
}

#[test]
fn failed_stratified_constraints_exhaust_without_proposals() {
    let original = strata(true, true);
    assert!(independent(&original).is_empty());
    for route in ROUTES {
        let mut stream = search(
            route,
            &original,
            Limits {
                max_candidates: 0,
                ..Limits::default()
            },
            Cancellation::default(),
        );
        enable(&mut stream);
        assert!(route.collect(&mut stream).is_empty());
        let stats = stream.statistics();
        assert_eq!(stats.candidates, 0);
        assert_eq!(stats.stable_models, 0);
        assert_eq!(stats.certified.unwrap().checks, 0);
        assert_eq!(stats.countermodel_queries, 0);
        assert_eq!(stats.regions.unwrap().counts.regions, 0);
    }
}

#[test]
fn stratified_batch_retries_preserve_the_pending_subject() {
    let original = strata(true, false);
    for route in ROUTES {
        let mut stream = search(route, &original, Limits::default(), Cancellation::default());
        enable(&mut stream);
        assert!(matches!(
            stream.next_batch(batch(4), |owner, rows| {
                assert!(owner.same_instance(&original));
                assert_eq!(rows.len(), 1);
                assert!(rows[0].theory().same_instance(&original));
                assert_eq!(rows[0].atoms().collect::<Vec<_>>(), [0, 1, 3]);
                Err::<Vec<BatchVerdict>, _>("retry")
            }),
            Err(BatchError::Checker("retry"))
        ));
        assert_eq!(stream.statistics().stable_models, 0);
        assert_eq!(stream.batch_statistics().pending, 1);
        assert_eq!(stream.statistics().certified.unwrap().checks, 0);
        let answers = stream.next_batch(batch(4), residual).unwrap();
        assert_eq!(answers.len(), 1);
        assert!(answers[0].theory().same_instance(&original));
        assert_eq!(answers[0].atoms().collect::<Vec<_>>(), [0, 1, 3]);
        assert!(
            stream
                .next_batch(batch(4), |_, _| -> Result<Vec<BatchVerdict>, Infallible> {
                    panic!("exhausted singleton does not enter a checker")
                })
                .unwrap()
                .is_empty()
        );
        assert!(stream.exhausted());
        assert_eq!(stream.statistics().candidates, 1);
        assert_eq!(stream.statistics().stable_models, 1);
        assert_eq!(stream.statistics().certified.unwrap().checks, 1);
        assert_eq!(stream.batch_statistics().committed, 1);
        assert_eq!(stream.batch_statistics().pending, 0);
    }
}

#[test]
fn restrictions_filter_the_stratified_singleton() {
    let original = strata(true, false);
    let restriction = theory(5, vec![Node::atom(2)], vec![0]);
    for route in ROUTES {
        for before in [false, true] {
            let mut stream = search(route, &original, Limits::default(), Cancellation::default());
            if before {
                stream.restrict_candidates(&restriction).unwrap();
            }
            enable(&mut stream);
            if !before {
                stream.restrict_candidates(&restriction).unwrap();
            }
            assert!(route.collect(&mut stream).is_empty());
            assert_eq!(stream.statistics().candidates, 0);
            assert_eq!(stream.statistics().candidate_restrictions, 1);
        }
    }
}

#[test]
fn unsupported_strata_fall_back_to_tight_membership() {
    let negative_cycle = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(1, 2),
            Node::implies(3, 0),
            Node::implies(0, 2),
            Node::implies(5, 1),
        ],
        vec![4, 6],
    );
    for (original, negative) in [(negative_cycle, true), (choices(2), false)] {
        for route in ROUTES {
            let mut stream = route.search(&original, Cancellation::default());
            assert!(
                stream
                    .enable_class_checking(
                        CertificateLimits::default(),
                        CertificateOrder::StratifiedFirst
                    )
                    .unwrap()
            );
            let stats = stream.statistics().certified.unwrap();
            assert!(matches!(
                stats.plan,
                Some(CertificatePlanStatistics::Tight(_))
            ));
            if negative {
                assert!(matches!(
                    stats.stratified_refusal,
                    Some(StratifiedError::NegativeCycle { .. })
                ));
            } else {
                assert!(matches!(
                    stats.stratified_refusal,
                    Some(StratifiedError::UnsupportedRoot { .. })
                ));
            }
            assert_eq!(route.collect(&mut stream), independent(&original));
            assert!(stream.statistics().certified.unwrap().checks > 1);
        }
    }
}

#[test]
fn stratified_storage_refusal_keeps_general_membership() {
    let original = strata(true, false);
    for route in ROUTES {
        let mut stream = route.search(&original, Cancellation::default());
        assert!(
            !stream
                .enable_class_checking(
                    CertificateLimits {
                        stratified: StratifiedPlanLimits {
                            max_bytes: 0,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    CertificateOrder::StratifiedFirst
                )
                .unwrap()
        );
        let stats = stream.statistics().certified.unwrap();
        assert!(matches!(
            stats.stratified_refusal,
            Some(StratifiedError::Limit {
                resource: StratifiedResource::Bytes,
                ..
            })
        ));
        assert_eq!(stats.plan, None);
        assert_eq!(route.collect(&mut stream), independent(&original));
        assert!(stream.statistics().countermodel_queries > 0);
    }
}

#[test]
fn zero_candidate_allowance_cannot_publish_stratified_answers() {
    let original = strata(true, false);
    for route in ROUTES {
        let mut stream = search(
            route,
            &original,
            Limits {
                max_candidates: 0,
                ..Limits::default()
            },
            Cancellation::default(),
        );
        enable(&mut stream);
        assert!(matches!(
            stream.next(),
            Some(Err(Incomplete::CandidateLimit))
        ));
        assert!(!stream.exhausted());
        assert_eq!(stream.statistics().candidates, 0);
        assert_eq!(stream.statistics().stable_models, 0);
    }
}

#[test]
fn stratified_setup_failure_cannot_establish_coverage() {
    let original = strata(true, false);
    for route in ROUTES {
        let mut probe = search(route, &original, Limits::default(), Cancellation::default());
        let initial = probe.statistics().search.work;
        enable(&mut probe);
        let required = probe.statistics().search.work;
        for max_work in [initial, required - 1, required] {
            let mut limits = Limits::default();
            limits.search.max_work = max_work;
            let mut stream = search(route, &original, limits, Cancellation::default());
            let result = stream.enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::StratifiedFirst,
            );
            assert_eq!(stream.statistics().search.work, max_work);
            assert_eq!(stream.statistics().stable_models, 0);
            if max_work == required {
                assert_eq!(result, Ok(true));
                assert!(matches!(
                    stream.statistics().certified.unwrap().plan,
                    Some(CertificatePlanStatistics::Stratified(_))
                ));
            } else {
                assert_eq!(result, Err(Incomplete::WorkLimit));
                assert!(stream.next().is_none());
                assert!(!stream.exhausted());
            }
        }
    }
}

#[test]
fn cancelled_stratified_setup_publishes_no_plan() {
    let original = strata(true, false);
    for route in ROUTES {
        let cancellation = Cancellation::default();
        let mut stream = route.search(&original, cancellation.clone());
        cancellation.cancel();
        assert_eq!(
            stream.enable_class_checking(
                CertificateLimits::default(),
                CertificateOrder::StratifiedFirst,
            ),
            Err(Incomplete::Cancelled),
        );
        let stats = stream.statistics();
        assert_eq!(stats.certified.unwrap().plan, None);
        assert_eq!(stats.certified.unwrap().stratified_attempt.unwrap().work, 0);
        assert_eq!(stats.stable_models, 0);
        assert!(stream.next().is_none());
        assert!(!stream.exhausted());
    }
}

#[test]
fn cancelled_stratified_pulls_preserve_completed_prefixes() {
    for route in ROUTES {
        for delivered in [false, true] {
            let original = strata(true, false);
            let cancellation = Cancellation::default();
            let mut stream = route.search(&original, cancellation.clone());
            enable(&mut stream);
            if delivered {
                assert!(stream.next().unwrap().is_ok());
            }
            cancellation.cancel();
            assert!(matches!(stream.next(), Some(Err(Incomplete::Cancelled))));
            assert!(!stream.exhausted());
            assert_eq!(stream.statistics().stable_models, u64::from(delivered));
        }
    }
}

#[test]
fn failed_stratified_checks_keep_the_pending_answer() {
    let original = strata(true, false);
    for route in ROUTES {
        let mut stream = search(
            route,
            &original,
            Limits {
                max_verification_work: (original.nodes().len()
                    + original.parts().occurrences()
                    + original.roots().len()) as u64,
                ..Limits::default()
            },
            Cancellation::default(),
        );
        enable(&mut stream);
        assert!(matches!(
            stream.next_batch(batch(4), residual),
            Err(BatchError::Search(Incomplete::Verification(
                zetesis_cpu::Stop::WorkLimit
            )))
        ));
        let stats = stream.statistics();
        assert_eq!(stats.candidates, 1);
        assert_eq!(stats.stable_models, 0);
        assert_eq!(stats.certified.unwrap().failed, 1);
        assert!(
            stats
                .certified
                .unwrap()
                .stratified_check_peak_bytes
                .is_some()
        );
        assert_eq!(stream.batch_statistics().pending, 1);
        assert_eq!(stream.batch_statistics().committed, 0);
        assert!(!stream.exhausted());
    }
}

#[test]
fn refused_determined_restrictions_accumulate_work() {
    let original = theory(3, vec![], vec![]);
    let mut limits = Limits::default();
    limits.admission.max_clauses = 1;
    let mut stream = by_clauses(&original, limits, Cancellation::default()).unwrap();
    let initial = stream.statistics().search.work;
    assert!(
        !stream
            .enable_class_checking(
                CertificateLimits {
                    tight: TightPlanLimits {
                        max_bytes: 0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                CertificateOrder::StratifiedFirst
            )
            .unwrap()
    );
    let stats = stream.statistics().certified.unwrap();
    assert!(stats.stratified_attempt.unwrap().retained_bytes > 0);
    assert!(stats.positive_attempt.unwrap().retained_bytes > 0);
    assert_eq!(stats.plan, None);
    // Both determined plans attempt two units before the same admission refusal.
    assert_eq!(stats.restriction_work, 8);
    assert_eq!(stats.restriction_clauses, 0);
    assert_eq!(
        stream.statistics().search.work - initial,
        stats.construction_work + stats.restriction_work
    );
    let family: BTreeSet<Vec<usize>> = stream
        .by_ref()
        .map(|answer| answer.unwrap().atoms().collect())
        .collect();
    assert_eq!(family, BTreeSet::from([vec![]]));
    assert!(stream.exhausted());
    assert_eq!(stream.statistics().candidates, 8);
}
