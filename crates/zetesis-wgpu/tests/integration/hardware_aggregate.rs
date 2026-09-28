//! Explicit physical numeric aggregate qualification; absence is a hard failure.

use crate::support::physical;
mod fixtures;

use zetesis_backend::GpuApi;
use zetesis_core::Value;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::native_aggregate::{self as native, Function};
use zetesis_wgpu::{
    AggregateGpuActivity, AggregateGpuError, AggregateGpuEvaluation, AggregateGpuLimits,
    AggregateGpuPlan, AggregateGpuPlanLimits, AggregateGpuValue, GpuAggregateOracle, GpuErrorKind,
    GpuOptions, GpuSelection,
};

fn oracle(backend: GpuApi) -> GpuAggregateOracle {
    let oracle =
        GpuAggregateOracle::new_selected(GpuOptions::default(), GpuSelection { api: backend })
            .unwrap();
    physical::verify(backend, oracle.info());
    oracle
}

fn assert_evaluation(actual: AggregateGpuEvaluation, expected: native::Evaluation<'_>) {
    let value = match expected.value() {
        native::Value::Integer(value) => AggregateGpuValue::Integer(i32::try_from(value).unwrap()),
        native::Value::Term(value) => match value.descriptor() {
            zetesis_core::ValueNodeRef::Infimum => AggregateGpuValue::Infimum,
            zetesis_core::ValueNodeRef::Supremum => AggregateGpuValue::Supremum,
            _ => panic!("numeric fixture"),
        },
    };
    assert_eq!(actual.value(), value);
    assert_eq!(actual.holds(), expected.holds());
}

fn compare(
    oracle: &mut GpuAggregateOracle,
    plan: &AggregateGpuPlan<'_>,
    records: &[native::Eligibility<'_>],
    limits: AggregateGpuLimits,
) {
    let actual = oracle
        .check_batch(plan, records, limits, &Cancellation::default())
        .unwrap();
    assert_eq!(actual.len(), records.len());
    for (actual, record) in actual.iter().zip(records) {
        let expected = record
            .reduce(native::ReductionLimits::default(), &Cancellation::default())
            .unwrap();
        assert_evaluation(actual.original(), expected.original());
        assert_eq!(actual.frozen().is_some(), expected.frozen().is_some());
        if let (Some(actual), Some(expected)) = (actual.frozen(), expected.frozen()) {
            assert_evaluation(actual, expected);
        }
        assert_eq!(actual.reduct_truth(), expected.reduct_truth());
    }
    let stats = oracle.last_batch_stats().unwrap();
    let activity = oracle.activity();
    assert_eq!(stats.occurrences, records.len() as u64);
    assert_eq!(activity.submissions, 1);
    assert_eq!(activity.submitted_occurrences, records.len() as u64);
    assert_eq!(activity.completed_occurrences, records.len() as u64);
    assert_eq!(activity.scheduled_work, stats.device_work);
    assert_eq!(activity.completed_work, stats.device_work);
    assert_eq!(activity.downloaded_bytes, 40 * records.len() as u64);
    let mask_bytes =
        records.len() as u64 * (1 + 2 * plan.group().tuples().len().div_ceil(32)) as u64 * 4;
    assert_eq!(
        activity.uploaded_bytes,
        u64::from(stats.group_uploaded) * plan.bytes() + 32 + mask_bytes
    );
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_aggregate_reductions_match_native_occurrences() {
    qualify_reductions(GpuApi::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_aggregate_reductions_match_native_occurrences() {
    qualify_reductions(GpuApi::Vulkan);
}

fn qualify_reductions(backend: GpuApi) {
    let mut oracle = oracle(backend);
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    let mut occurrences = 0;
    for function in fixtures::FUNCTIONS {
        for count in [0, 1, 31, 32, 33, 63, 64, 65, 129, 1024] {
            let group = fixtures::group(&theory, function, count);
            let plan = AggregateGpuPlan::new(
                &group,
                AggregateGpuPlanLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            for batch in [1, 3, 31, 32, 33, 65] {
                let mut records = fixtures::observations(&group, &worlds, batch);
                compare(&mut oracle, &plan, &records, AggregateGpuLimits::default());
                assert_eq!(
                    oracle.last_batch_stats().unwrap().group_uploaded,
                    batch == 1
                );
                assert!(oracle.last_batch_stats().unwrap().transport_allocated);
                records.reverse();
                compare(
                    &mut oracle,
                    &plan.clone(),
                    &records,
                    AggregateGpuLimits::default(),
                );
                assert!(!oracle.last_batch_stats().unwrap().group_uploaded);
                assert!(!oracle.last_batch_stats().unwrap().transport_allocated);
                occurrences += batch * 2;
            }
        }
    }
    println!("aggregate exact ordered occurrences={occurrences}");
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_aggregate_guards_preserve_numeric_boundaries() {
    qualify_guards(GpuApi::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_aggregate_guards_preserve_numeric_boundaries() {
    qualify_guards(GpuApi::Vulkan);
}

fn qualify_guards(backend: GpuApi) {
    let mut oracle = oracle(backend);
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    for function in fixtures::FUNCTIONS {
        let mut carrier = vec![
            Some(Value::Number(i32::MIN)),
            Some(Value::Number(i32::MAX)),
            None,
        ];
        if matches!(
            function,
            Function::Count | Function::Sum | Function::SumPlus
        ) {
            carrier.push(Some(Value::Symbol("neutral".into())));
        }
        for comparison in fixtures::COMPARISONS {
            for bound in [i32::MIN, 0, i32::MAX] {
                // Both numerical bound views are admissible; all declared guards
                // must be checked even when the first one is false.
                let guards = vec![
                    native::Guard {
                        comparison,
                        bound: native::Bound::Integer(i128::from(bound)),
                    },
                    native::Guard {
                        comparison: zetesis_ferraris::AggregateComparison::Ne,
                        bound: native::Bound::Term(Value::Number(2)),
                    },
                ];
                let group = fixtures::operation(&theory, function, carrier.clone(), guards);
                let plan = AggregateGpuPlan::new(
                    &group,
                    AggregateGpuPlanLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                compare(
                    &mut oracle,
                    &plan,
                    &fixtures::observations(&group, &worlds, 41),
                    AggregateGpuLimits::default(),
                );
            }
        }
    }
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_aggregate_exact_admission_preserves_cache_lifecycle() {
    qualify_admission(GpuApi::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_aggregate_exact_admission_preserves_cache_lifecycle() {
    qualify_admission(GpuApi::Vulkan);
}

fn qualify_admission(backend: GpuApi) {
    let mut oracle = oracle(backend);
    let theory = fixtures::theory();
    let worlds = fixtures::worlds(&theory);
    let group = fixtures::group(&theory, Function::Sum, 4);
    let plan = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let records = fixtures::observations(&group, &worlds, 3);
    compare(&mut oracle, &plan, &records, AggregateGpuLimits::default());
    let stats = *oracle.last_batch_stats().unwrap();
    let exact = AggregateGpuLimits {
        max_occurrences: records.len(),
        max_batch_bytes: stats.accounted_bytes,
        max_host_work: stats.host_work,
        max_device_work: stats.device_work,
        ..Default::default()
    };
    refuse_preflight(&mut oracle, &plan, &records, exact);
    let equal = fixtures::group(&theory, Function::Sum, 4);
    let foreign = fixtures::observations(&equal, &worlds, 3);
    assert!(
        matches!(oracle.check_batch(&plan, &foreign, exact, &Cancellation::default()), Err(AggregateGpuError::Gpu(error)) if error.kind() == GpuErrorKind::Seed)
    );
    compare(&mut oracle, &plan, &records, exact);
    assert!(!oracle.last_batch_stats().unwrap().group_uploaded);
    assert!(!oracle.last_batch_stats().unwrap().transport_allocated);

    replace_exact(&mut oracle, &plan, &records, exact, &theory, &worlds);
}

fn refuse_preflight(
    oracle: &mut GpuAggregateOracle,
    plan: &AggregateGpuPlan<'_>,
    records: &[native::Eligibility<'_>],
    exact: AggregateGpuLimits,
) {
    for limit in [
        AggregateGpuLimits {
            max_occurrences: 2,
            ..exact
        },
        AggregateGpuLimits {
            max_batch_bytes: exact.max_batch_bytes - 1,
            ..exact
        },
        AggregateGpuLimits {
            max_host_work: exact.max_host_work - 1,
            ..exact
        },
        AggregateGpuLimits {
            max_device_work: exact.max_device_work - 1,
            ..exact
        },
    ] {
        assert!(
            matches!(oracle.check_batch(plan, records, limit, &Cancellation::default()), Err(AggregateGpuError::Gpu(error)) if error.kind() == GpuErrorKind::Capacity)
        );
        assert_eq!(oracle.activity(), AggregateGpuActivity::default());
        assert!(oracle.last_batch_stats().is_none());
    }
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert_eq!(
        oracle
            .check_batch(plan, records, exact, &cancelled)
            .unwrap_err(),
        AggregateGpuError::Stopped(Stop::Cancelled)
    );
}

fn replace_exact(
    oracle: &mut GpuAggregateOracle,
    plan: &AggregateGpuPlan<'_>,
    records: &[native::Eligibility<'_>],
    exact: AggregateGpuLimits,
    theory: &zetesis_ferraris::Theory,
    worlds: &[zetesis_ferraris::Interpretation],
) {
    let group = plan.group();
    let large = fixtures::group(theory, Function::Sum, 1024);
    let large_plan = AggregateGpuPlan::new(
        &large,
        AggregateGpuPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    compare(
        oracle,
        &large_plan,
        &fixtures::observations(&large, worlds, 65),
        AggregateGpuLimits::default(),
    );
    compare(oracle, plan, records, exact);
    let replaced = oracle.last_batch_stats().unwrap();
    assert!(replaced.incoming_resident_bytes > exact.max_batch_bytes);
    assert!(replaced.group_uploaded);
    assert!(replaced.transport_allocated);
    assert_eq!(replaced.accounted_bytes, exact.max_batch_bytes);

    compare(
        oracle,
        plan,
        &fixtures::observations(group, worlds, 65),
        AggregateGpuLimits::default(),
    );
    compare(oracle, plan, records, exact);
    assert!(!oracle.last_batch_stats().unwrap().group_uploaded);
    assert!(oracle.last_batch_stats().unwrap().transport_allocated);
    assert!(oracle.last_batch_stats().unwrap().incoming_resident_bytes > exact.max_batch_bytes);
    let independent = AggregateGpuPlan::new(
        group,
        AggregateGpuPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    compare(oracle, &independent, records, exact);
    assert!(oracle.last_batch_stats().unwrap().group_uploaded);
    oracle.clear_residency();
    compare(oracle, plan, records, exact);
    assert!(oracle.last_batch_stats().unwrap().group_uploaded);
    assert_eq!(
        oracle.last_batch_stats().unwrap().incoming_resident_bytes,
        0
    );
    assert!(
        oracle
            .check_batch(
                plan,
                &[],
                AggregateGpuLimits {
                    max_occurrences: 0,
                    max_batch_bytes: 0,
                    max_host_work: 0,
                    max_device_work: 0,
                    ..Default::default()
                },
                &Cancellation::default()
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(oracle.activity(), AggregateGpuActivity::default());
    assert!(oracle.last_batch_stats().is_none());
}
