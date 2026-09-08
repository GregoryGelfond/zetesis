use super::{
    super::{
        AggregateGpuEvaluation, AggregateGpuLimits, AggregateGpuPlan, AggregateGpuPlanLimits,
        AggregateGpuValue,
        packing::{Plan, decode},
        preparation::bits,
    },
    fixtures,
};
use crate::GpuErrorKind;
use std::time::Duration;
use zetesis_core::Value;
use zetesis_cpu::Control;
use zetesis_ferraris::native_aggregate::{self as native, Function};

#[test]
fn packed_eligibility_preserves_each_occurrence() {
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    for count in [0, 1, 31, 32, 33, 63, 64, 65] {
        let group = fixtures::group(&theory, Function::Sum, count);
        let prepared = AggregateGpuPlan::new(
            &group,
            AggregateGpuPlanLimits::default(),
            &Control::default(),
        )
        .unwrap();
        let records = fixtures::observations(&group, &worlds, 33);
        let plan = Plan::new(
            &prepared,
            &records,
            AggregateGpuLimits::default(),
            &wgpu::Limits::default(),
            1,
            &Control::default(),
        )
        .unwrap();
        let packed = plan.pack(&records, &Control::default()).unwrap();
        let width = count.div_ceil(32);
        assert_eq!(plan.stride as usize, 1 + 2 * width);
        for (record, row) in records
            .iter()
            .zip(packed.chunks_exact(plan.stride as usize))
        {
            assert_eq!(row[0], u32::from(record.frozen().is_some()));
            for index in 0..width * 32 {
                assert_eq!(
                    (row[1 + index / 32] >> (index % 32)) & 1,
                    u32::from(record.original().get(index).copied().unwrap_or(false))
                );
                assert_eq!(
                    (row[1 + width + index / 32] >> (index % 32)) & 1,
                    u32::from(
                        record
                            .frozen()
                            .and_then(|mask| mask.get(index))
                            .copied()
                            .unwrap_or(false)
                    )
                );
            }
        }
    }
}

fn encoded(evaluation: native::Evaluation<'_>) -> [u32; 3] {
    let (value, present) = match evaluation.value() {
        native::Value::Integer(number) => (bits(i32::try_from(number).unwrap()), 1),
        native::Value::Term(Value::Infimum | Value::Supremum) => (0, 0),
        native::Value::Term(_) => panic!("numeric fixture"),
    };
    [value, present, u32::from(evaluation.holds())]
}

fn wire(plan: &Plan, group: &native::Group, records: &[native::Eligibility<'_>]) -> Vec<u32> {
    let empty = vec![false; group.tuples().len()];
    let absent = group
        .reduce(
            &empty,
            None,
            native::ReductionLimits::default(),
            &Control::default(),
        )
        .unwrap()
        .original();
    records
        .iter()
        .enumerate()
        .flat_map(|(index, record)| {
            let value = record
                .reduce(native::ReductionLimits::default(), &Control::default())
                .unwrap();
            let original = encoded(value.original());
            let frozen = encoded(value.frozen().unwrap_or(absent));
            [
                plan.epoch,
                u32::try_from(index).unwrap(),
                u32::from(record.frozen().is_some()),
                original[0],
                original[1],
                original[2],
                frozen[0],
                frozen[1],
                frozen[2],
                plan.work,
            ]
        })
        .collect()
}

fn value(value: native::Value<'_>) -> AggregateGpuValue {
    match value {
        native::Value::Integer(value) => AggregateGpuValue::Integer(i32::try_from(value).unwrap()),
        native::Value::Term(Value::Infimum) => AggregateGpuValue::Infimum,
        native::Value::Term(Value::Supremum) => AggregateGpuValue::Supremum,
        native::Value::Term(_) => panic!("numeric fixture"),
    }
}

#[test]
fn readback_preserves_native_reduction_observations() {
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    for function in fixtures::FUNCTIONS {
        for count in [0, 1, 33, 65] {
            for comparison in fixtures::COMPARISONS {
                let group = fixtures::operation(
                    &theory,
                    function,
                    (0..count)
                        .map(|index| Some(Value::Number(index % 7 - 3)))
                        .collect(),
                    vec![native::Guard {
                        comparison,
                        bound: native::Bound::Integer(0),
                    }],
                );
                let prepared = AggregateGpuPlan::new(
                    &group,
                    AggregateGpuPlanLimits::default(),
                    &Control::default(),
                )
                .unwrap();
                let mut records = fixtures::observations(&group, &worlds, 41);
                records.reverse();
                let plan = Plan::new(
                    &prepared,
                    &records,
                    AggregateGpuLimits::default(),
                    &wgpu::Limits::default(),
                    9,
                    &Control::default(),
                )
                .unwrap();
                let packed = plan.pack(&records, &Control::default()).unwrap();
                let decoded = decode(
                    &wire(&plan, &group, &records),
                    &prepared.numeric,
                    &plan,
                    &packed,
                    &Control::default(),
                )
                .unwrap();
                assert_eq!(decoded.len(), records.len());
                for (actual, record) in decoded.iter().zip(&records) {
                    let expected = record
                        .reduce(native::ReductionLimits::default(), &Control::default())
                        .unwrap();
                    assert_eq!(
                        actual.original().value(),
                        value(expected.original().value())
                    );
                    assert_eq!(actual.original().holds(), expected.original().holds());
                    assert_eq!(
                        actual.frozen().map(AggregateGpuEvaluation::value),
                        expected.frozen().map(|item| value(item.value()))
                    );
                    assert_eq!(
                        actual.frozen().map(AggregateGpuEvaluation::holds),
                        expected.frozen().map(native::Evaluation::holds)
                    );
                    assert_eq!(actual.reduct_truth(), expected.reduct_truth());
                }
            }
        }
    }
}

#[test]
fn malformed_readback_returns_no_partial_verdicts() {
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    let group = fixtures::group(&theory, Function::Count, 3);
    let prepared = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let records = fixtures::observations(&group, &worlds, 3);
    let plan = Plan::new(
        &prepared,
        &records,
        AggregateGpuLimits::default(),
        &wgpu::Limits::default(),
        7,
        &Control::default(),
    )
    .unwrap();
    let masks = plan.pack(&records, &Control::default()).unwrap();
    let valid = wire(&plan, &group, &records);
    for offset in 0..10 {
        let mut malformed = valid.clone();
        // Corrupt the final record, after two valid records have been decoded.
        malformed[20 + offset] = match offset {
            3 | 6 => bits(-1),
            _ => u32::MAX,
        };
        assert_eq!(
            decode(
                &malformed,
                &prepared.numeric,
                &plan,
                &masks,
                &Control::default()
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::Readback
        );
    }
    assert_eq!(
        decode(
            &valid[..valid.len() - 1],
            &prepared.numeric,
            &plan,
            &masks,
            &Control::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
    let mut false_guard = valid.clone();
    false_guard[25] ^= 1;
    assert_eq!(
        decode(
            &false_guard,
            &prepared.numeric,
            &plan,
            &masks,
            &Control::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn empty_extrema_require_canonical_absence() {
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    for function in [Function::Min, Function::Max] {
        let group = fixtures::group(&theory, function, 0);
        let prepared = AggregateGpuPlan::new(
            &group,
            AggregateGpuPlanLimits::default(),
            &Control::default(),
        )
        .unwrap();
        let records = fixtures::observations(&group, &worlds, 1);
        let plan = Plan::new(
            &prepared,
            &records,
            AggregateGpuLimits::default(),
            &wgpu::Limits::default(),
            1,
            &Control::default(),
        )
        .unwrap();
        let masks = plan.pack(&records, &Control::default()).unwrap();
        assert_eq!(plan.stride, 1);
        assert_eq!(prepared.numeric.tuples, [0, 0]);
        let mut malformed = wire(&plan, &group, &records);
        malformed[3] = 1;
        assert_eq!(
            decode(
                &malformed,
                &prepared.numeric,
                &plan,
                &masks,
                &Control::default()
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::Readback
        );
    }
}

#[test]
fn batch_ceilings_are_inclusive() {
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    let group = fixtures::group(&theory, Function::Sum, 65);
    let prepared = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let records = fixtures::observations(&group, &worlds, 33);
    let device = wgpu::Limits::default();
    let plan = Plan::new(
        &prepared,
        &records,
        AggregateGpuLimits::default(),
        &device,
        1,
        &Control::default(),
    )
    .unwrap();
    let exact = AggregateGpuLimits {
        max_occurrences: records.len(),
        max_batch_bytes: plan.accounted,
        max_host_work: plan.host_work,
        max_device_work: plan.total_work,
        ..Default::default()
    };
    assert!(Plan::new(&prepared, &records, exact, &device, 1, &Control::default()).is_ok());
    for below in [
        AggregateGpuLimits {
            max_occurrences: records.len() - 1,
            ..exact
        },
        AggregateGpuLimits {
            max_batch_bytes: plan.accounted - 1,
            ..exact
        },
        AggregateGpuLimits {
            max_host_work: plan.host_work - 1,
            ..exact
        },
        AggregateGpuLimits {
            max_device_work: plan.total_work - 1,
            ..exact
        },
        AggregateGpuLimits {
            timeout: Duration::ZERO,
            ..exact
        },
    ] {
        assert_eq!(
            Plan::new(&prepared, &records, below, &device, 1, &Control::default())
                .err()
                .unwrap()
                .kind(),
            GpuErrorKind::Capacity
        );
    }
    for limited in [
        wgpu::Limits {
            max_compute_workgroups_per_dimension: 32,
            ..device.clone()
        },
        wgpu::Limits {
            max_storage_buffer_binding_size: plan.results - 1,
            ..device.clone()
        },
        wgpu::Limits {
            max_buffer_size: plan.results - 1,
            ..device.clone()
        },
        wgpu::Limits {
            max_uniform_buffer_binding_size: 31,
            ..device.clone()
        },
    ] {
        assert_eq!(
            Plan::new(&prepared, &records, exact, &limited, 1, &Control::default())
                .err()
                .unwrap()
                .kind(),
            GpuErrorKind::Capacity
        );
    }
}

#[test]
fn equal_group_contents_do_not_forge_eligibility_identity() {
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    let group = fixtures::group(&theory, Function::Sum, 4);
    let other = fixtures::group(&theory, Function::Sum, 4);
    let prepared = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let records = fixtures::observations(&other, &worlds, 1);
    assert_eq!(
        Plan::new(
            &prepared,
            &records,
            AggregateGpuLimits::default(),
            &wgpu::Limits::default(),
            1,
            &Control::default()
        )
        .err()
        .unwrap()
        .kind(),
        GpuErrorKind::Seed
    );
}

#[test]
fn cancellation_interrupts_host_batch_work() {
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    let group = fixtures::group(&theory, Function::Sum, 4);
    let prepared = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let records = fixtures::observations(&group, &worlds, 1);
    let plan = Plan::new(
        &prepared,
        &records,
        AggregateGpuLimits::default(),
        &wgpu::Limits::default(),
        1,
        &Control::default(),
    )
    .unwrap();
    let masks = plan.pack(&records, &Control::default()).unwrap();
    let control = Control::default();
    control.cancel();
    assert!(
        Plan::new(
            &prepared,
            &records,
            AggregateGpuLimits::default(),
            &wgpu::Limits::default(),
            1,
            &control
        )
        .is_err()
    );
    assert!(plan.pack(&records, &control).is_err());
    assert!(
        decode(
            &wire(&plan, &group, &records),
            &prepared.numeric,
            &plan,
            &masks,
            &control
        )
        .is_err()
    );
}

#[test]
fn complete_work_bounds_shader_index_arithmetic() {
    use super::super::packing::device_work;
    let maximum = u32::MAX / 2 - 63;
    for (tuples, guards) in [
        (maximum, 0),
        (0, maximum),
        (maximum / 2, maximum - maximum / 2),
    ] {
        assert_eq!(device_work(tuples, guards).unwrap(), u32::MAX - 1);
        for count in [tuples, guards].into_iter().filter(|count| *count > 0) {
            assert!(
                count
                    .checked_sub(1)
                    .and_then(|index| index.checked_mul(2))
                    .and_then(|index| index.checked_add(1))
                    .is_some()
            );
            assert!((count - 1).checked_add(64).is_some());
        }
        assert_eq!(
            device_work(tuples + 1, guards).unwrap_err().kind(),
            GpuErrorKind::Capacity
        );
    }
    assert_eq!(
        device_work(u32::MAX, 1).unwrap_err().kind(),
        GpuErrorKind::Capacity
    );
}
