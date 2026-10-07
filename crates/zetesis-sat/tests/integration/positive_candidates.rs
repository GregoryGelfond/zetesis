//! Complete positive evidence removes region search while preserving boundaries.

use std::{convert::Infallible, num::NonZeroUsize};
use zetesis_ferraris::{Interpretation, Node, Theory, TightPlanLimits};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Cancellation, CertificateLimits, CertificateOrder,
    Incomplete, Limits, StableModels,
};
use zetesis_theory_support::theories::theory;

use crate::support::region_filters::{Condition, Filter, ROUTES, Route};

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
                CertificateOrder::PositiveFirst
            )
            .unwrap()
    );
}

fn cycle(seed: bool) -> Theory {
    let mut roots = vec![2, 3];
    if seed {
        roots.push(0);
    }
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        roots,
    )
}

fn failed_constraint() -> Theory {
    // not not a has the classical model {a}, but its least producer closure is empty.
    theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::implies(2, 1),
        ],
        vec![3],
    )
}

fn batch() -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(4).unwrap(),
        max_pending_bytes: 4096,
    }
}

#[test]
fn every_region_mode_proposes_only_the_least_interpretation() {
    for route in ROUTES {
        for seed in [false, true] {
            let original = cycle(seed);
            let mut limits = Limits::default();
            limits.search.max_decisions = 0;
            limits.max_candidates = 1;
            let mut stream = search(route, &original, limits, Cancellation::default());
            enable(&mut stream);
            let candidate = stream.next().unwrap().unwrap();
            assert!(candidate.theory().same_instance(&original));
            assert_eq!(
                candidate.atoms().collect::<Vec<_>>(),
                if seed { vec![0, 1] } else { vec![] }
            );
            assert!(stream.next().is_none());
            assert!(stream.exhausted());
            let stats = stream.statistics();
            assert_eq!(stats.candidates, 1);
            assert_eq!(stats.stable_models, 1);
            assert_eq!(stats.certified.unwrap().checks, 1);
            assert_eq!(stats.certified.unwrap().refuted, 0);
            assert_eq!(stats.search.decisions, 0);
            assert_eq!(stats.regions.unwrap().counts.regions, 0);
            assert_eq!(stats.countermodel_queries, 0);
            assert!(stats.reduct.preparation.is_none());
        }
    }
}

#[test]
fn failed_constraint_skips_larger_classical_models() {
    let original = failed_constraint();
    let larger = Interpretation::new(&original, [0]).unwrap();
    assert!(
        zetesis_ferraris::models(
            &original,
            &larger,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
    );
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
        let before = stream.statistics().search;
        assert!(stream.next().is_none());
        assert!(stream.exhausted());
        let stats = stream.statistics();
        assert_eq!(stats.candidates, 0);
        assert_eq!(stats.certified.unwrap().checks, 0);
        assert_eq!(stats.search, before);
        assert_eq!(stats.regions.unwrap().counts.regions, 0);
    }
}

#[test]
fn restrictions_on_either_side_of_configuration_are_conjoined() {
    let original = cycle(true);
    let allow = theory(3, vec![Node::atom(0)], vec![0]);
    let refuse = theory(3, vec![Node::atom(2)], vec![0]);
    for route in ROUTES {
        for before in [false, true] {
            let mut stream = search(
                route,
                &original,
                Limits {
                    max_candidates: 0,
                    ..Limits::default()
                },
                Cancellation::default(),
            );
            stream.restrict_candidates(&allow).unwrap();
            if before {
                stream.restrict_candidates(&refuse).unwrap();
            }
            enable(&mut stream);
            if !before {
                stream.restrict_candidates(&refuse).unwrap();
            }
            assert!(stream.next().is_none());
            assert!(stream.exhausted());
            assert_eq!(stream.statistics().candidate_restrictions, 2);
            assert_eq!(stream.statistics().candidates, 0);
        }
    }
}

#[test]
fn restriction_after_delivery_cannot_repeat_the_candidate() {
    let original = cycle(true);
    let allow = theory(3, vec![Node::atom(0)], vec![0]);
    for route in ROUTES {
        let mut stream = search(route, &original, Limits::default(), Cancellation::default());
        enable(&mut stream);
        assert!(stream.next().unwrap().is_ok());
        stream.restrict_candidates(&allow).unwrap();
        assert!(stream.next().is_none());
        assert!(stream.exhausted());
        assert_eq!(stream.statistics().stable_models, 1);
    }
}

#[test]
fn singleton_filter_refutation_establishes_coverage() {
    let original = cycle(true);
    for route in ROUTES {
        let mut stream = search(route, &original, Limits::default(), Cancellation::default());
        enable(&mut stream);
        stream
            .set_region_filter(Filter::new(&original, Condition::ForbidHeld(0)))
            .unwrap();
        assert!(stream.next().is_none());
        assert!(stream.exhausted());
        assert_eq!(stream.statistics().candidates, 0);
        let stats = stream.statistics().region_filter.unwrap();
        assert_eq!(stats.preparations, 1);
        assert_eq!(stats.refuted, 1);
        assert_eq!(stats.failed, 0);
    }
}

#[test]
fn singleton_filter_stops_leave_coverage_open() {
    let original = cycle(true);
    for route in ROUTES {
        for (condition, expected) in [
            (Condition::FailPreparation, Incomplete::RegionFilter),
            (Condition::FailCheck, Incomplete::RegionFilter),
            (Condition::Cancel, Incomplete::Cancelled),
        ] {
            let mut stream = search(route, &original, Limits::default(), Cancellation::default());
            enable(&mut stream);
            stream
                .set_region_filter(Filter::new(&original, condition))
                .unwrap();
            assert!(matches!(stream.next(), Some(Err(found)) if found == expected));
            assert!(!stream.exhausted());
            assert_eq!(stream.statistics().candidates, 0);
            let stats = stream.statistics().region_filter.unwrap();
            assert_eq!(stats.preparations, 1);
            assert_eq!(stats.failed, 1);
            assert_eq!(stats.refuted, 0);
        }
    }
}

#[test]
fn batch_retries_keep_the_single_original_subject() {
    let original = cycle(true);
    let refuse = theory(3, vec![Node::atom(2)], vec![0]);
    for route in ROUTES {
        let mut stream = search(route, &original, Limits::default(), Cancellation::default());
        enable(&mut stream);
        assert!(matches!(
            stream.next_batch(batch(), |owner, candidates| {
                assert!(owner.same_instance(&original));
                assert_eq!(candidates.len(), 1);
                assert!(candidates[0].theory().same_instance(&original));
                assert_eq!(candidates[0].atoms().collect::<Vec<_>>(), vec![0, 1]);
                Err::<Vec<BatchVerdict>, _>("retry")
            }),
            Err(BatchError::Checker("retry"))
        ));
        stream.restrict_candidates(&refuse).unwrap();
        let returned = stream
            .next_batch(batch(), |owner, candidates| {
                assert!(owner.same_instance(&original));
                assert_eq!(candidates.len(), 1);
                Ok::<_, Infallible>(vec![BatchVerdict::NoProperSubset])
            })
            .unwrap();
        assert_eq!(returned.len(), 1);
        assert!(returned[0].theory().same_instance(&original));
        assert!(
            stream
                .next_batch(batch(), |_, _| -> Result<Vec<BatchVerdict>, Infallible> {
                    panic!("coverage does not call the checker")
                })
                .unwrap()
                .is_empty()
        );
        assert!(stream.exhausted());
        assert_eq!(stream.statistics().candidates, 1);
        assert_eq!(stream.statistics().stable_models, 1);
        assert_eq!(stream.statistics().certified.unwrap().checks, 0);
        assert_eq!(stream.batch_statistics().checker_calls, 2);
        assert_eq!(stream.batch_statistics().pending, 0);
    }
}

#[test]
fn original_validation_still_precedes_the_external_checker() {
    let original = cycle(true);
    for route in ROUTES {
        let mut stream = search(
            route,
            &original,
            Limits {
                max_verification_work: 0,
                ..Limits::default()
            },
            Cancellation::default(),
        );
        enable(&mut stream);
        assert!(matches!(
            stream.next_batch(batch(), |_, _| -> Result<Vec<BatchVerdict>, Infallible> {
                panic!("the original validation must refuse first")
            }),
            Err(BatchError::Search(Incomplete::Verification(
                zetesis_cpu::Stop::WorkLimit
            )))
        ));
        assert_eq!(stream.statistics().candidates, 0);
        assert_eq!(stream.batch_statistics().pending, 0);
        assert!(!stream.exhausted());
    }
}

#[test]
fn a_zero_candidate_allowance_cannot_publish_the_least() {
    let original = cycle(false);
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
        assert_eq!(stream.statistics().candidates, 0);
        assert!(!stream.exhausted());
    }
}

#[test]
fn positive_pulls_use_one_inclusive_work_budget() {
    let original = cycle(true);
    for route in ROUTES {
        let mut probe = search(route, &original, Limits::default(), Cancellation::default());
        enable(&mut probe);
        let setup = probe.statistics().search.work;
        assert!(probe.next().unwrap().is_ok());
        let required = probe.statistics().search.work - setup;
        assert_eq!(
            required,
            (original.atom_count().div_ceil(64)
                + original.atom_count()
                + original.nodes().len()
                + original.parts().occurrences()
                + original.roots().len()) as u64
        );
        for remaining in 0..=required {
            let mut limits = Limits::default();
            limits.search.max_work = setup + remaining;
            let mut stream = search(route, &original, limits, Cancellation::default());
            enable(&mut stream);
            let result = stream.next();
            assert_eq!(stream.statistics().search.work, setup + remaining);
            if remaining == required {
                assert!(result.unwrap().is_ok());
                assert!(stream.next().is_none());
                assert!(stream.exhausted());
            } else {
                assert!(matches!(result, Some(Err(Incomplete::WorkLimit))));
                assert!(!stream.exhausted());
            }
        }
    }
}

#[test]
fn cancelled_positive_pulls_leave_coverage_open() {
    for original in [cycle(true), failed_constraint(), theory(0, vec![], vec![])] {
        for route in ROUTES {
            let control = Cancellation::default();
            let mut stream = search(route, &original, Limits::default(), control.clone());
            enable(&mut stream);
            control.cancel();
            assert!(matches!(stream.next(), Some(Err(Incomplete::Cancelled))));
            assert_eq!(stream.statistics().stable_models, 0);
            assert!(!stream.exhausted());
        }
    }
}

#[test]
fn cancellation_after_delivery_does_not_establish_exhaustion() {
    let original = theory(0, vec![], vec![]);
    for route in ROUTES {
        let control = Cancellation::default();
        let mut stream = search(route, &original, Limits::default(), control.clone());
        enable(&mut stream);
        let candidate = stream.next().unwrap().unwrap();
        assert!(candidate.theory().same_instance(&original));
        assert_eq!(candidate.atoms().count(), 0);
        control.cancel();
        assert!(matches!(stream.next(), Some(Err(Incomplete::Cancelled))));
        assert!(!stream.exhausted());
        assert_eq!(stream.statistics().stable_models, 1);
    }
}

#[test]
fn external_tight_preparation_keeps_region_proposals() {
    let original = theory(2, vec![Node::atom(0)], vec![0]);
    for route in [Route::Scalar, Route::Producers] {
        let mut stream = search(route, &original, Limits::default(), Cancellation::default());
        let plan = stream
            .prepare_tight_certificate(TightPlanLimits::default())
            .unwrap()
            .unwrap();
        assert!(plan.theory().same_instance(&original));
        let rows = stream
            .next_batch(batch(), |owner, candidates| {
                assert!(owner.same_instance(&original));
                assert_eq!(candidates.len(), 1);
                Ok::<_, Infallible>(vec![BatchVerdict::NoProperSubset])
            })
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(stream.statistics().certified.unwrap().checks, 0);
        assert!(stream.statistics().regions.unwrap().counts.regions > 0);
    }
}

#[test]
fn singleton_restriction_receipts_agree_across_region_modes() {
    let original = cycle(true);
    let restriction = theory(
        3,
        vec![Node::atom(0), Node::atom(1), Node::and_pair([0, 1])],
        vec![2],
    );
    let mut expected = None;
    for route in ROUTES {
        let mut stream = search(route, &original, Limits::default(), Cancellation::default());
        stream.restrict_candidates(&restriction).unwrap();
        enable(&mut stream);
        assert!(stream.next().unwrap().is_ok());
        assert!(stream.next().is_none());
        assert!(stream.exhausted());
        let counts = stream.statistics().regions.unwrap().counts;
        assert!(
            counts.propagations > 0,
            "the restriction actually propagated formula truth"
        );
        if let Some(expected) = expected {
            assert_eq!(counts, expected);
        } else {
            expected = Some(counts);
        }
    }
}
