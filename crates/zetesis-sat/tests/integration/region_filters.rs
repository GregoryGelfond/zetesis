//! Original region constraints compose with candidate coverage, never reduct queries.

use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

use zetesis_ferraris::{Node, TightPlanLimits};
use zetesis_sat::{Cancellation, Incomplete, Limits, SearchMethod, StableModels};

use crate::support::region_filters::{Condition, Filter, ROUTES, Route, choices};
use zetesis_theory_support::theories::theory;

#[test]
fn absent_filter_has_no_receipt() {
    for route in ROUTES {
        let mut search = route.search(&choices(1), Cancellation::default());
        assert!(search.statistics().region_filter.is_none());
        assert_eq!(route.collect(&mut search).len(), 2);
        assert!(search.statistics().region_filter.is_none());
    }
}

#[test]
fn filter_reads_original_narrowing_before_membership() {
    let subject = theory(1, vec![Node::Atom(0)], vec![0]);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search.enable_phase_timing();
        search
            .set_region_filter(Filter::new(&subject, Condition::ForbidHeld(0)))
            .unwrap();
        assert!(route.collect(&mut search).is_empty(), "{route:?}");
        let receipt = search.statistics();
        assert_eq!(receipt.candidates, 0);
        assert_eq!(receipt.search.decisions, 0);
        assert_eq!(receipt.countermodel_queries, 0);
        let filtered = receipt.region_filter.unwrap();
        assert_eq!(filtered.preparations, 1);
        assert_eq!(filtered.checks, 1);
        assert_eq!(filtered.refuted, 1);
        assert_eq!(filtered.failed, 0);
        assert_eq!(receipt.phase_timings.unwrap().original_validation.calls, 1);
    }
}

#[test]
fn partial_region_refutations_preserve_the_answer_family() {
    let subject = choices(3);
    let expected: BTreeSet<Vec<usize>> = (0..8)
        .filter(|mask| mask & 3 != 3)
        .map(|mask| (0..3).filter(|atom| mask & (1 << atom) != 0).collect())
        .collect();
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, Condition::ForbidPair(0, 1)))
            .unwrap();
        assert_eq!(route.collect(&mut search), expected, "{route:?}");
        assert!(search.statistics().region_filter.unwrap().refuted > 0);
    }
}

#[test]
fn original_filter_never_excludes_reduct_countermodels() {
    // a <- b; b <- a. Requiring a removes the empty original answer, but
    // must not remove the empty frozen-reduct witness against {a,b}.
    let subject = theory(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Implies(0, 1),
            Node::Implies(1, 0),
        ],
        vec![2, 3],
    );
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, Condition::Require(0)))
            .unwrap();
        assert!(route.collect(&mut search).is_empty(), "{route:?}");
        let receipt = search.statistics();
        assert!(
            receipt.candidates > 0,
            "the nonminimal original model was checked"
        );
        assert!(
            receipt.countermodels > 0,
            "its empty reduct witness survived"
        );
    }
}

#[test]
fn failed_filter_preparation_never_establishes_exhaustion() {
    let subject = choices(2);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, Condition::FailPreparation))
            .unwrap();
        assert!(matches!(search.next(), Some(Err(Incomplete::RegionFilter))));
        let _ = search.stop();
        assert!(!search.exhausted());
        assert!(search.next().is_none());
        let filtered = search.statistics().region_filter.unwrap();
        assert!(filtered.preparations > 0);
        assert_eq!(filtered.failed, filtered.preparations);
        assert_eq!(filtered.checks, 0);
    }
}

#[test]
fn failed_filter_preparation_is_timed() {
    let subject = choices(2);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search.enable_phase_timing();
        search
            .set_region_filter(Filter::new(&subject, Condition::FailPreparation))
            .unwrap();
        assert!(matches!(search.next(), Some(Err(Incomplete::RegionFilter))));
        let _ = search.stop();
        let receipt = search.statistics();
        let prepared = receipt.region_filter.unwrap().preparations;
        assert!(prepared > 0);
        assert_eq!(
            receipt.phase_timings.unwrap().original_validation.calls,
            prepared
        );
    }
}

#[test]
fn failed_region_check_retains_its_attempt_receipt() {
    let subject = choices(2);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search.enable_phase_timing();
        search
            .set_region_filter(Filter::new(&subject, Condition::FailCheck))
            .unwrap();
        assert!(matches!(search.next(), Some(Err(Incomplete::RegionFilter))));
        let _ = search.stop();
        assert!(!search.exhausted());
        let receipt = search.statistics();
        let filtered = receipt.region_filter.unwrap();
        assert_eq!(filtered.checks, 1);
        assert_eq!(filtered.failed, 1);
        assert_eq!(filtered.refuted, 0);
        assert_eq!(receipt.phase_timings.unwrap().original_validation.calls, 1);
        assert!(receipt.regions.unwrap().counts.work > 0);
    }
}

#[test]
fn filter_configuration_requires_an_unstarted_stream() {
    let subject = choices(2);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        assert!(search.next().unwrap().is_ok());
        assert!(matches!(
            search.set_region_filter(Filter::new(&subject, Condition::Pass)),
            Err(Incomplete::LateRegionFilter),
        ));
        search.stop().unwrap();
    }
}

#[test]
fn a_configured_filter_cannot_be_replaced() {
    let subject = choices(1);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, Condition::Pass))
            .unwrap();
        assert!(matches!(
            search.set_region_filter(Filter::new(&subject, Condition::Require(0))),
            Err(Incomplete::RegionFilterAlreadySet),
        ));
        assert_eq!(route.collect(&mut search).len(), 2);
    }
}

#[test]
fn clause_search_refuses_region_filter_configuration() {
    let subject = choices(1);
    let mut search = StableModels::with_method(
        &subject,
        SearchMethod::Clauses,
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    assert!(matches!(
        search.set_region_filter(Filter::new(&subject, Condition::Pass)),
        Err(Incomplete::RegionFilterUnsupported),
    ));
    assert!(search.statistics().region_filter.is_none());
}

#[test]
fn certificate_preparation_keeps_filter_installation_open() {
    let subject = choices(2);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        assert!(
            search
                .enable_certified_checking(TightPlanLimits::default())
                .unwrap()
        );
        search
            .set_region_filter(Filter::new(&subject, Condition::Require(0)))
            .unwrap();
        assert_eq!(
            route.collect(&mut search),
            BTreeSet::from([vec![0], vec![0, 1]])
        );
    }
}

#[test]
fn explicit_stop_joins_borrowed_checkers_without_cancelling() {
    let subject = choices(10);
    let cancellation = Cancellation::default();
    let filter = Filter::new(&subject, Condition::Pass);
    let mut search = Route::Native.search(&subject, cancellation.clone());
    search.set_region_filter(filter.clone()).unwrap();
    assert!(search.next().unwrap().is_ok());
    search.stop().unwrap();
    assert_eq!(filter.live.load(Ordering::SeqCst), 0);
    assert!(cancellation.poll().is_ok());
    assert!(!search.exhausted());
    assert!(search.next().is_none());
    let joined = search.statistics();
    assert!(joined.region_filter.unwrap().checks > 0);
    assert_eq!(joined.stable_models, 1, "queued answers were not delivered");
    search.stop().unwrap();
    assert_eq!(search.statistics(), joined);
}

#[test]
fn stopping_complete_search_preserves_exhaustion() {
    for route in ROUTES {
        let mut search = route.search(&choices(1), Cancellation::default());
        assert_eq!(route.collect(&mut search).len(), 2);
        let complete = search.statistics();
        search.stop().unwrap();
        assert!(search.exhausted());
        assert_eq!(search.statistics(), complete);
    }
}

#[test]
fn stopping_unstarted_workers_preserves_certificate_work() {
    let subject = choices(2);
    let mut search = Route::Native.search(&subject, Cancellation::default());
    let construction = search.statistics().search.work;
    assert!(
        search
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let prepared = search.statistics();
    assert!(prepared.search.work > construction);
    search.stop().unwrap();
    assert_eq!(search.statistics(), prepared);
    assert!(!search.exhausted());
}

#[test]
fn filter_subject_authentication_precedes_region_checks() {
    let subject = choices(1);
    let other = choices(1);
    assert!(!subject.same_instance(&other));
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&other, Condition::Pass))
            .unwrap();
        assert!(matches!(search.next(), Some(Err(Incomplete::WrongTheory))));
        let _ = search.stop();
        assert_eq!(search.statistics().region_filter.unwrap().checks, 0);
        assert!(!search.exhausted());
    }
}

#[test]
fn cancellation_inside_a_filter_stops_without_exhaustion() {
    let subject = choices(2);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, Condition::Cancel))
            .unwrap();
        assert!(matches!(search.next(), Some(Err(Incomplete::Cancelled))));
        let _ = search.stop();
        assert!(!search.exhausted());
        assert_eq!(search.statistics().candidates, 0);
        assert_eq!(search.statistics().region_filter.unwrap().checks, 1);
    }
}

#[test]
fn core_refutation_never_prepares_an_external_checker() {
    let subject = theory(0, vec![Node::False], vec![0]);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, Condition::FailPreparation))
            .unwrap();
        assert!(route.collect(&mut search).is_empty());
        assert_eq!(
            search.statistics().region_filter.unwrap(),
            zetesis_sat::RegionFilterStatistics::default()
        );
    }
}

#[test]
fn empty_final_pull_never_prepares_an_external_checker() {
    let subject = theory(1, vec![Node::Atom(0)], vec![0]);
    for route in ROUTES {
        let mut search = route.search(&subject, Cancellation::default());
        search
            .set_region_filter(Filter::new(&subject, Condition::Pass))
            .unwrap();
        assert!(search.next().unwrap().is_ok());
        assert!(search.next().is_none());
        assert!(search.exhausted());
        assert_eq!(search.statistics().region_filter.unwrap().preparations, 1);
    }
}
