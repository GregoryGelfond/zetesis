//! Completed-source component receipts, without a timing or solve harness.

use super::*;
use crate::formula_ground::{Builder, FALSUM, Purpose, VERUM};
use crate::formula_support::Counters;
use crate::grounding_observer::Profile;
use crate::{
    ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, GroundingObserver,
    GroundingOutcome, GroundingPhase, ProgramSite,
};
use std::cell::Cell;

const SOURCE: &str = "p(1;2;3). :- p(X),p(Y),X!=Y.";

#[derive(Default)]
struct Peak(Cell<Option<u64>>);
impl GroundingObserver for Peak {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: crate::GroundingWork,
    ) {
        self.0.set(work.support_peak_bytes);
    }
}

#[derive(Clone, Copy, Default)]
struct Bounds {
    work: Option<u64>,
    substitutions: Option<u64>,
    bytes: Option<usize>,
    cancel: Option<route::Point>,
}

struct Attempt {
    result: Result<bool, FormulaFailure>,
    work: u64,
    substitutions: u64,
    roots: usize,
    counts: Counts,
    peak: u64,
}

fn attempt(bounds: Bounds) -> Attempt {
    let peak = Peak::default();
    let profile = Profile::new(Some(&peak));
    let mut observed = None;
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let accounting = std::mem::take(&mut counters.accounting);
        *counters = Counters::resume(accounting, profile.work());
        let rule = program
            .rules_without_facts()
            .iter()
            .find(|rule| matches!(rule.head, crate::formula_ir::HeadIr::Normal(None)))
            .unwrap();
        let limits = FormulaLimits::default();
        let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            rule.location,
        )
        .unwrap();
        builder.initialize(rule.location).unwrap();
        let before = builder.counters.accounting.work;
        let substitutions = builder.counters.accounting.substitutions;
        let bounded = FormulaLimits {
            max_work: bounds.work.map_or(limits.max_work, |work| before + work),
            max_substitutions: bounds
                .substitutions
                .map_or(limits.max_substitutions, |rows| substitutions + rows),
            max_support_bytes: bounds.bytes.unwrap_or(limits.max_support_bytes),
            ..limits
        };
        builder.limits = &bounded;
        let cancellation = Cancellation::default();
        builder.counters.attach_test_control(&cancellation);
        let (result, counts) = route::controlled(
            true,
            bounds.cancel.map(|point| (point, cancellation)),
            || {
                profile.phase(
                    GroundingPhase::RuleInstantiation,
                    Some(rule.location),
                    || super::super::rule(&mut builder, rule, support),
                )
            },
        );
        observed = Some(Attempt {
            result,
            work: builder.counters.accounting.work - before,
            substitutions: builder.counters.accounting.substitutions - substitutions,
            roots: builder.roots.len(),
            counts,
            peak: peak.0.get().unwrap_or(0),
        });
    });
    observed.unwrap()
}

#[test]
fn every_work_prefix_keeps_performed_work_and_no_partial_group_root() {
    let full = attempt(Bounds::default());
    assert!(matches!(full.result, Ok(true)));
    assert_eq!(full.roots, 1);
    assert!(full.work > 0);
    for ceiling in 0..full.work {
        let stopped = attempt(Bounds {
            work: Some(ceiling),
            ..Default::default()
        });
        assert!(
            matches!(stopped.result, Err(FormulaFailure::Limit {
            resource: FormulaResource::Work, limit, observed, ..
        }) if observed > limit),
            "prefix {ceiling}"
        );
        assert!(stopped.work <= ceiling);
        assert_eq!(
            stopped.roots, 0,
            "one group publishes only after its complete witness stream"
        );
    }
    let exact = attempt(Bounds {
        work: Some(full.work),
        ..Default::default()
    });
    assert!(matches!(exact.result, Ok(true)));
    assert_eq!(exact.work, full.work);
}

#[test]
fn substitutions_count_the_two_actual_lane_traversals() {
    let full = attempt(Bounds::default());
    assert!(matches!(full.result, Ok(true)));
    assert_eq!(full.counts.first_rows, 3);
    assert_eq!(full.counts.second_rows, 3);
    assert_eq!(full.substitutions, 6);
    let short = attempt(Bounds {
        substitutions: Some(5),
        ..Default::default()
    });
    assert!(matches!(short.result, Err(FormulaFailure::Limit {
        resource: FormulaResource::Substitutions, observed, limit, ..
    }) if observed == limit + 1));
    assert_eq!(short.substitutions, 5);
    assert_eq!(short.counts.first_rows, 3);
    assert_eq!(short.counts.second_rows, 2);
    assert_eq!(short.roots, 0);
    assert!(matches!(
        attempt(Bounds {
            substitutions: Some(6),
            ..Default::default()
        })
        .result,
        Ok(true)
    ));
}

#[test]
fn a_full_run_respects_its_recorded_actual_capacity_allowance() {
    let full = attempt(Bounds::default());
    assert!(matches!(full.result, Ok(true)));
    assert!(full.peak > 0);
    let bounded = attempt(Bounds {
        bytes: Some(usize::try_from(full.peak).unwrap()),
        ..Default::default()
    });
    assert!(matches!(bounded.result, Ok(true)));
    assert!(bounded.peak <= full.peak);
    assert_eq!(bounded.counts, full.counts);
    assert_eq!(bounded.substitutions, full.substitutions);
    // Spare geometric capacity is optional. One byte below an observed peak
    // need not refuse: ordinary reserve may choose its exact requirement.
}

#[test]
fn both_group_map_headers_share_one_exact_live_byte_boundary() {
    for short in [false, true] {
        testing::Fixture::default().with(
            crate::test_support::location(),
            |_, computation, counters| {
                let location = crate::test_support::location();
                let limits = FormulaLimits::default();
                let mut budget =
                    crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
                let mut builder = Builder::empty(
                    computation,
                    &limits,
                    &mut budget,
                    counters,
                    Purpose::Theory,
                    None,
                    location,
                )
                .unwrap();
                builder.initialize(location).unwrap();
                let lease = builder.computation.lease();
                let baseline = limits.max_support_bytes
                    - builder
                        .computation
                        .allowance(&lease, &limits, location)
                        .unwrap();
                let bytes = size_of::<super::super::Groups>();
                let bounded = FormulaLimits {
                    max_support_bytes: baseline + bytes - usize::from(short),
                    ..limits
                };
                builder.limits = &bounded;
                let result = super::super::Groups::new(&mut builder, location);
                if short {
                    assert!(matches!(result, Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes, observed, limit, ..
                }) if observed == (baseline + bytes) as u128
                    && limit == (baseline + bytes - 1) as u128));
                } else {
                    let groups = result.unwrap_or_else(|error| panic!("{error}"));
                    assert_eq!(
                        limits.max_support_bytes
                            - builder
                                .computation
                                .allowance(&lease, &limits, location)
                                .unwrap(),
                        baseline + bytes
                    );
                    drop(groups);
                }
                assert_eq!(
                    limits.max_support_bytes
                        - builder
                            .computation
                            .allowance(&lease, &limits, location)
                            .unwrap(),
                    baseline
                );
            },
        );
    }
}

#[test]
fn cancellation_during_either_pass_remains_a_typed_stop() {
    for point in [
        route::Point::Accepted,
        route::Point::FirstRow,
        route::Point::SecondRow,
    ] {
        let stopped = attempt(Bounds {
            cancel: Some(point),
            ..Default::default()
        });
        assert!(matches!(
            stopped.result,
            Err(FormulaFailure::Interrupted {
                reason: zetesis_algebra::Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(stopped.roots, 0);
    }
}

#[test]
fn repeated_witnesses_use_any_without_counting_another_lane() {
    testing::Fixture::default().with(
        crate::test_support::location(),
        |_, computation, counters| {
            let location = crate::test_support::location();
            let limits = FormulaLimits::default();
            let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
            let mut builder = Builder::empty(
                computation,
                &limits,
                &mut budget,
                counters,
                Purpose::Theory,
                None,
                location,
            )
            .unwrap();
            builder.initialize(location).unwrap();
            let binding = crate::formula_binding::Binding::new(
                builder.computation,
                &limits,
                builder.counters,
                location,
            )
            .unwrap();
            let fiber = builder.head_tuple(&[], &binding, location).unwrap();
            let value = builder
                .computation
                .number(7, &limits, builder.counters, location)
                .unwrap();
            let lane = builder.term(&value, location).unwrap();
            let key = [fiber, lane];
            let mut groups = super::super::Groups::new(&mut builder, location).unwrap();
            groups.identity(key, &mut builder, location).unwrap();
            groups.identity(key, &mut builder, location).unwrap();
            for witness in [FALSUM, VERUM, FALSUM, VERUM] {
                let previous = groups
                    .lanes
                    .find(
                        &key,
                        builder.computation,
                        &limits,
                        builder.counters,
                        location,
                    )
                    .unwrap()
                    .unwrap();
                groups
                    .merge_witness(key, previous, witness, &mut builder, location)
                    .unwrap();
            }
            assert_eq!(
                groups
                    .lanes
                    .find(
                        &key,
                        builder.computation,
                        &limits,
                        builder.counters,
                        location
                    )
                    .unwrap(),
                Some(Some(VERUM))
            );
            assert_eq!(
                groups
                    .fibers
                    .find(
                        &fiber,
                        builder.computation,
                        &limits,
                        builder.counters,
                        location
                    )
                    .unwrap()
                    .unwrap()
                    .count,
                1
            );
            assert!(!groups.has_pairs(&mut builder, location).unwrap());
        },
    );
}
