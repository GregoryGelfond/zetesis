//! Prospective payload bounds and submitted per-binding observations.

use std::num::NonZeroU32;

use super::{
    Capacity, LazyBufferUsage, LazyGpuStatistics, LazyTransportUsage, Plan, Selection,
    plan::Allowance, tests::inspect,
};
use crate::{GpuErrorKind, GpuLimits};

#[test]
fn selective_retention_preserves_every_admitted_ceiling() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        let exact = plan.capacity.accounted(&plan).unwrap();
        // Exhaust the independent smaller/exact/larger input shapes, each
        // result shape direction, and both tight and generous allowances.
        for shape in 0..81 {
            let mut digits = shape;
            let inputs = std::array::from_fn(|index| {
                let adjustment = digits % 3;
                digits /= 3;
                match adjustment {
                    0 => plan.capacity.inputs[index] - 4,
                    1 => plan.capacity.inputs[index],
                    _ => plan.capacity.inputs[index] + 16,
                }
            });
            for result in [
                plan.result_bytes - 4,
                plan.result_bytes,
                plan.result_bytes + 4,
            ] {
                let previous = Capacity { inputs, result };
                for maximum in [exact, exact + 16, exact + 64, u64::MAX] {
                    let chosen = Selection::new(Some(previous), &plan, maximum);
                    assert!(chosen.capacity.accounted(&plan).unwrap() <= maximum);
                    assert_eq!(chosen.capacity.result, plan.result_bytes);
                    assert!(chosen.retention.uniform);
                    for index in 0..4 {
                        assert!(chosen.capacity.inputs[index] >= plan.capacity.inputs[index]);
                        assert_eq!(
                            chosen.retention.inputs[index],
                            previous.inputs[index] == chosen.capacity.inputs[index]
                        );
                    }
                    assert_eq!(chosen.retention.result, result == plan.result_bytes);
                    assert_eq!(
                        chosen.transition.is_reuse(),
                        chosen.retention.inputs.into_iter().all(|kept| kept)
                            && chosen.retention.result
                    );
                    if chosen.slack != Allowance::Fits {
                        assert_eq!(chosen.capacity, plan.capacity);
                    }
                }
            }
        }
    });
}

#[test]
fn prospective_growth_can_require_releasing_input_slack() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        let mut previous = plan.capacity;
        previous.inputs[0] -= 4;
        previous.inputs[1] += 64;
        let maximum = previous.accounted(&plan).unwrap();
        let chosen = Selection::new(Some(previous), &plan, maximum);
        assert_eq!(chosen.capacity, plan.capacity);
        assert_eq!(chosen.slack, Allowance::Exceeded);
        assert_eq!(chosen.retention.inputs, [false, false, true, true]);
        assert!(chosen.retention.result);
        let observed = LazyGpuStatistics::default()
            .submitted(&plan, chosen)
            .unwrap();
        // The old whole-shape accounting fit. It is the prospective growth
        // plus independently retained slack that would cross the ceiling.
        assert_eq!(observed.transport_replacements.budget, 0);
        assert_eq!(observed.transport_usage.budget_releases, 1);
    });
}

#[test]
fn tight_fallback_preserves_exact_fit_buffers() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        let mut previous = plan.capacity;
        previous.inputs[1] += 128;
        let maximum = plan.capacity.accounted(&plan).unwrap();
        let chosen = Selection::new(Some(previous), &plan, maximum);
        assert_eq!(chosen.capacity.accounted(&plan), Some(maximum));
        assert_eq!(chosen.retention.inputs, [true, false, true, true]);
        assert!(chosen.retention.uniform);
        assert!(chosen.retention.result);
    });
}

#[test]
fn overflow_releases_slack_without_refusing_the_exact_shape() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        let mut previous = plan.capacity;
        previous.inputs[2] = u64::MAX;
        let chosen = Selection::new(Some(previous), &plan, u64::MAX);
        assert_eq!(chosen.capacity, plan.capacity);
        assert_eq!(chosen.retention.inputs, [true, true, false, true]);
        assert_eq!(chosen.slack, Allowance::Overflow);
        let observed = LazyGpuStatistics::default()
            .submitted(&plan, chosen)
            .unwrap();
        assert_eq!(observed.transport_usage.accounting_overflow_releases, 1);
        assert_eq!(observed.transport_usage.budget_releases, 0);
    });
}

fn bindings(usage: LazyTransportUsage) -> [LazyBufferUsage; 7] {
    [
        usage.uniform,
        usage.offsets,
        usage.records,
        usage.snapshots,
        usage.seeds,
        usage.output,
        usage.readback,
    ]
}

#[test]
fn every_submitted_binding_has_one_usage_observation() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        let mut previous = plan.capacity;
        let mut observed = LazyGpuStatistics::default()
            .submitted(&plan, Selection::new(None, &plan, u64::MAX))
            .unwrap();
        for changed in 0..4 {
            previous.inputs[changed] -= 4;
            observed = observed
                .submitted(&plan, Selection::new(Some(previous), &plan, u64::MAX))
                .unwrap();
            previous = plan.capacity;
        }
        assert_eq!(observed.dispatches, 5);
        for binding in bindings(observed.transport_usage) {
            assert_eq!(binding.allocations + binding.reuses, observed.dispatches);
        }
        assert_eq!(
            observed.transport_usage.output,
            observed.transport_usage.readback
        );
        assert_eq!(
            observed.transport_usage.uniform,
            LazyBufferUsage {
                allocations: 1,
                reuses: 4
            }
        );
        assert_eq!(observed.transport_allocations, 5);
        assert_eq!(observed.transport_reuses, 0);
    });
}

#[test]
fn usage_sums_refuse_each_field_overflow() {
    let fields: [fn(&mut LazyTransportUsage) -> &mut LazyBufferUsage; 7] = [
        |value| &mut value.uniform,
        |value| &mut value.offsets,
        |value| &mut value.records,
        |value| &mut value.snapshots,
        |value| &mut value.seeds,
        |value| &mut value.output,
        |value| &mut value.readback,
    ];
    for field in fields {
        for retained in [false, true] {
            let mut left = LazyTransportUsage::default();
            let mut right = LazyTransportUsage::default();
            if retained {
                field(&mut left).reuses = u64::MAX;
                field(&mut right).reuses = 1;
            } else {
                field(&mut left).allocations = u64::MAX;
                field(&mut right).allocations = 1;
            }
            assert!(left.checked_add(right).is_none());
            assert_eq!(left.checked_add(LazyTransportUsage::default()), Some(left));
        }
    }
    for field in [
        (|value: &mut LazyTransportUsage| &mut value.budget_releases)
            as fn(&mut LazyTransportUsage) -> &mut u64,
        |value| &mut value.accounting_overflow_releases,
    ] {
        let mut left = LazyTransportUsage::default();
        let mut right = LazyTransportUsage::default();
        *field(&mut left) = u64::MAX;
        *field(&mut right) = 1;
        assert!(left.checked_add(right).is_none());
    }
}

#[test]
fn usage_overflow_keeps_submission_statistics_unchanged() {
    inspect(|chunk| {
        let plan = Plan::new(
            NonZeroU32::MIN,
            chunk,
            GpuLimits::default(),
            &wgpu::Limits::default(),
        )
        .unwrap();
        let observed = LazyGpuStatistics {
            transport_usage: LazyTransportUsage {
                uniform: LazyBufferUsage {
                    allocations: u64::MAX,
                    reuses: 0,
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let before = observed;
        let error = observed
            .submitted(&plan, Selection::new(None, &plan, u64::MAX))
            .unwrap_err();
        assert_eq!(error.kind(), GpuErrorKind::Capacity);
        assert_eq!(observed, before);
    });
}
