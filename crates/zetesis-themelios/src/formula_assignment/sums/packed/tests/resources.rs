use super::*;
use crate::test_support::given_site;

#[test]
fn empty_value_refusal_precedes_input_and_other_limits() {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        *counters = std::mem::take(counters).with_cancellation(Some(&cancellation));
        let before = (counters.accounting.work, counters.workspace_bytes());
        let limits = FormulaLimits {
            max_assignment_values: 0,
            max_work: 0,
            max_support_bytes: 0,
            ..Default::default()
        };
        let unseen =
            std::iter::from_fn(|| -> Option<i32> { panic!("no input before the empty limit") });
        assert!(matches!(
            sums(unseen, computation, &limits, counters, given_site()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::AssignmentValues,
                observed: 1,
                limit: 0,
                ..
            })
        ));
        assert_eq!(
            (counters.accounting.work, counters.workspace_bytes()),
            before
        );
    });
}

#[test]
fn every_packed_work_prefix_releases_storage_and_the_exact_budget_fits() {
    let (_, needed) = result_and_work(&[1; 24], false);
    for cutoff in 0..=needed {
        Fixture::default().with(given_site(), |_, computation, counters| {
            let before = counters.workspace_bytes();
            let limits = FormulaLimits {
                max_work: counters.accounting.work + cutoff,
                ..Default::default()
            };
            let result = sums([1; 24], computation, &limits, counters, given_site());
            if cutoff == needed {
                assert_eq!(result.unwrap().slice(), (0..=24).collect::<Vec<_>>());
            } else {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    })
                ));
            }
            assert!(counters.accounting.work <= limits.max_work);
            assert_eq!(counters.workspace_bytes(), before);
        });
    }
}

#[test]
fn cancellation_between_packed_contributions_releases_storage() {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let cancellation = zetesis_cpu::Cancellation::default();
        *counters = std::mem::take(counters).with_cancellation(Some(&cancellation));
        let before = counters.workspace_bytes();
        let mut offered = 0;
        let weights = [1; 24].into_iter().inspect(|_| {
            offered += 1;
            if offered == 17 {
                cancellation.cancel();
            }
        });
        assert!(matches!(
            sums(
                weights,
                computation,
                &FormulaLimits::default(),
                counters,
                given_site()
            ),
            Err(FormulaFailure::Interrupted {
                reason: zetesis_cpu::Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(offered, 17);
        assert_eq!(counters.workspace_bytes(), before);
    });
}

// A dense canonical run is test setup, not a bypass around production storage.
fn dense_packed(maximum: i32, context: &mut Context<'_, &Computation<'_, '_>>) -> Packed {
    let mut values = Numbers::new(
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )
    .unwrap();
    for value in 0..=maximum {
        values
            .push(
                value,
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )
            .unwrap();
    }
    let plan = Plan::for_runs(&values, 0, context)
        .unwrap()
        .expect("dense entry");
    Packed::from_values(values, plan, context).unwrap()
}

fn live_bytes(computation: &Computation<'_, '_>, limits: &FormulaLimits) -> usize {
    let observer = computation.lease();
    limits.max_support_bytes
        - computation
            .allowance(&observer, limits, given_site())
            .unwrap()
}

#[test]
fn retained_updates_need_no_new_header_or_payload_allowance() {
    for extra_capacity in [false, true] {
        Fixture::default().with(given_site(), |_, computation, counters| {
            let defaults = FormulaLimits::default();
            let mut packed = dense_packed(
                32,
                &mut Context::new(computation, &defaults, counters, given_site()),
            );
            let weight = if extra_capacity {
                packed
                    .words
                    .reserve(1, computation, &defaults, counters, given_site())
                    .unwrap();
                32
            } else {
                1
            };
            let before = counters.workspace_bytes();
            let limits = FormulaLimits {
                max_support_bytes: live_bytes(computation, &defaults),
                ..defaults
            };
            let mut context = Context::new(&*computation, &limits, counters, given_site());
            let plan = packed
                .next_plan(weight, &mut context)
                .unwrap()
                .expect("all requested words fit the retained capacity");
            packed.add(weight, plan, &mut context).unwrap();
            assert_eq!(packed.maximum, 32 + usize::try_from(weight).unwrap());
            assert_eq!(packed.count, packed.maximum + 1);
            assert_eq!(context.work.counters.workspace_bytes(), before);
        });
    }
}

#[test]
fn replacement_growth_declines_before_allocation_at_the_live_ceiling() {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let defaults = FormulaLimits::default();
        let packed = dense_packed(
            32,
            &mut Context::new(computation, &defaults, counters, given_site()),
        );
        let weight = i32::try_from(packed.words.capacity() * WORD_BITS).unwrap();
        let proposed = Plan::select(packed.count, 32, weight, given_site())
            .unwrap()
            .expect("the proposed growth passes the independent work/space comparison");
        assert!(proposed.words > packed.words.capacity());
        let before = (counters.accounting.work, counters.workspace_bytes());
        let limits = FormulaLimits {
            max_support_bytes: live_bytes(computation, &defaults),
            ..defaults
        };
        assert!(
            packed
                .next_plan(
                    weight,
                    &mut Context::new(computation, &limits, counters, given_site())
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(
            (counters.accounting.work, counters.workspace_bytes()),
            before
        );
    });
}

#[test]
fn selected_entry_refusal_releases_both_transition_owners() {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let defaults = FormulaLimits::default();
        let before = counters.workspace_bytes();
        let mut runs = Runs::new(&mut Context::new(
            computation,
            &defaults,
            counters,
            given_site(),
        ))
        .unwrap();
        for _ in 0..16 {
            tick(&mut Context::new(
                computation,
                &defaults,
                counters,
                given_site(),
            ))
            .unwrap();
            runs.add(
                1,
                &mut Context::new(computation, &defaults, counters, given_site()),
            )
            .unwrap();
        }
        let plan = Plan::for_runs(
            &runs.values,
            1,
            &mut Context::new(computation, &defaults, counters, given_site()),
        )
        .unwrap()
        .unwrap();
        drop(runs.next);
        // Test the selected transition's refusal directly. Once selected it
        // must propagate the refusal; it cannot reinterpret it as a decline.
        let limits = FormulaLimits {
            max_support_bytes: live_bytes(computation, &defaults),
            ..defaults
        };
        assert!(matches!(
            super::super::finish(
                runs.values,
                1,
                [],
                plan,
                &mut Context::new(computation, &limits, counters, given_site())
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert_eq!(counters.workspace_bytes(), before);
    });
}
