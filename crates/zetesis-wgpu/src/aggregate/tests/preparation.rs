use super::super::{
    AggregateGpuCapability as Capability, AggregateGpuError as Error, AggregateGpuPlan,
    AggregateGpuPlanLimits, preparation::signed,
};
use zetesis_core::Value;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::native_aggregate::{Bound, Function, Guard};
use zetesis_theory_support::aggregate as reference;

#[test]
fn numerical_preparation_preserves_neutral_keys() {
    let theory = reference::theory();
    for (function, expected) in [
        (Function::Count, [1, 1, 1, 1]),
        (Function::Sum, [-3, 2, 0, 0]),
        (Function::SumPlus, [0, 2, 0, 0]),
    ] {
        let group = reference::operation(
            &theory,
            function,
            vec![
                Some(Value::Number(-3)),
                Some(Value::Number(2)),
                None,
                Some(Value::Symbol("ignored".into())),
            ],
            vec![],
        );
        let plan = AggregateGpuPlan::new(
            &group,
            AggregateGpuPlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(plan.numeric.tuple_count, 4);
        assert_eq!(
            plan.numeric
                .tuples
                .chunks_exact(2)
                .map(|record| signed(record[0]))
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            plan.numeric
                .tuples
                .chunks_exact(2)
                .all(|record| record[1] == 1)
        );
    }
}

#[test]
fn cancellation_cannot_hide_an_overflowing_positive_carrier() {
    let theory = reference::theory();
    let group = reference::operation(
        &theory,
        Function::Sum,
        [i32::MAX, 1, -1]
            .map(|value| Some(Value::Number(value)))
            .into(),
        vec![],
    );
    let error = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        Error::Capability(Capability::PositiveSum {
            total: i64::from(i32::MAX) + 1
        })
    );
}

#[test]
fn cancellation_cannot_hide_an_overflowing_negative_carrier() {
    let theory = reference::theory();
    let group = reference::operation(
        &theory,
        Function::Sum,
        [i32::MIN, -1, 1]
            .map(|value| Some(Value::Number(value)))
            .into(),
        vec![],
    );
    let error = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        Error::Capability(Capability::NegativeSum {
            total: i64::from(i32::MIN) - 1
        })
    );
}

#[test]
fn exact_signed_endpoints_are_admitted() {
    let theory = reference::theory();
    for function in reference::FUNCTIONS {
        let group = reference::operation(
            &theory,
            function,
            [i32::MIN, i32::MAX]
                .map(|value| Some(Value::Number(value)))
                .into(),
            vec![],
        );
        assert!(
            AggregateGpuPlan::new(
                &group,
                AggregateGpuPlanLimits::default(),
                &Cancellation::default()
            )
            .is_ok()
        );
    }
}

#[test]
fn extrema_refuse_nonnumeric_contributions_explicitly() {
    let theory = reference::theory();
    for function in [Function::Min, Function::Max] {
        let group = reference::operation(
            &theory,
            function,
            vec![Some(Value::Symbol("symbol".into()))],
            vec![],
        );
        assert_eq!(
            AggregateGpuPlan::new(
                &group,
                AggregateGpuPlanLimits::default(),
                &Cancellation::default()
            )
            .unwrap_err(),
            Error::Capability(Capability::NonNumericExtremum { tuple: 0 })
        );
    }
}

#[test]
fn numerical_guard_views_have_the_same_wire_representation() {
    let theory = reference::theory();
    for comparison in reference::COMPARISONS {
        let integer = reference::operation(
            &theory,
            Function::Count,
            vec![],
            vec![Guard {
                comparison,
                bound: Bound::Integer(-2),
            }],
        );
        let term = reference::operation(
            &theory,
            Function::Count,
            vec![],
            vec![Guard {
                comparison,
                bound: Bound::Term(Value::Number(-2)),
            }],
        );
        let prepare = |group| {
            AggregateGpuPlan::new(
                group,
                AggregateGpuPlanLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
        };
        assert_eq!(
            prepare(&integer).numeric.guards,
            prepare(&term).numeric.guards
        );
    }
}

#[test]
fn unsupported_guards_remain_typed_capability_failures() {
    let theory = reference::theory();
    for (bound, expected) in [
        (
            Bound::Integer(i128::from(i32::MAX) + 1),
            Capability::GuardRange { guard: 0 },
        ),
        (
            Bound::Term(Value::Supremum),
            Capability::NonNumericGuard { guard: 0 },
        ),
    ] {
        let group = reference::operation(
            &theory,
            Function::Count,
            vec![],
            vec![Guard {
                comparison: reference::COMPARISONS[0],
                bound,
            }],
        );
        assert_eq!(
            AggregateGpuPlan::new(
                &group,
                AggregateGpuPlanLimits::default(),
                &Cancellation::default()
            )
            .unwrap_err(),
            Error::Capability(expected)
        );
    }
}

#[test]
fn preparation_ceilings_are_inclusive() {
    let theory = reference::theory();
    let group = reference::group(&theory, Function::Sum, 4);
    let prepared = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let exact = AggregateGpuPlanLimits {
        max_tuples: 4,
        max_guards: 1,
        max_bytes: prepared.bytes(),
        max_work: prepared.work(),
    };
    assert!(AggregateGpuPlan::new(&group, exact, &Cancellation::default()).is_ok());
    for below in [
        AggregateGpuPlanLimits {
            max_tuples: 3,
            ..exact
        },
        AggregateGpuPlanLimits {
            max_guards: 0,
            ..exact
        },
        AggregateGpuPlanLimits {
            max_bytes: exact.max_bytes - 1,
            ..exact
        },
        AggregateGpuPlanLimits {
            max_work: exact.max_work - 1,
            ..exact
        },
    ] {
        assert!(
            matches!(AggregateGpuPlan::new(&group, below, &Cancellation::default()), Err(Error::Gpu(error)) if error.kind() == crate::GpuErrorKind::Capacity)
        );
    }
}

#[test]
fn preparation_observes_caller_cancellation() {
    let theory = reference::theory();
    let group = reference::group(&theory, Function::Sum, 0);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        AggregateGpuPlan::new(&group, AggregateGpuPlanLimits::default(), &cancellation)
            .unwrap_err(),
        Error::Stopped(Stop::Cancelled)
    );
}

#[test]
fn borrowed_canonical_group_uses_numeric_preparation() {
    let theory = reference::theory();
    let mut builder = zetesis_core::catalog::VocabularyBuilder::new(1 << 20).unwrap();
    let key = builder
        .import_term_with(
            (&Value::Number(-3)).into(),
            zetesis_core::catalog::Limits::default(),
            || Ok::<_, ()>(()),
        )
        .unwrap();
    let owner = builder.finish_with(0, || Ok::<_, ()>(())).unwrap();
    let data = zetesis_ferraris::native_aggregate::GroupData::new(
        &theory,
        Function::Sum,
        owner.read(),
        vec![zetesis_ferraris::native_aggregate::Tuple {
            key: vec![owner.read().term(&key).unwrap()],
            condition: 2,
        }],
        vec![],
        zetesis_ferraris::native_aggregate::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let view = data.bind_with(owner.read(), || Ok::<_, ()>(())).unwrap();
    let plan = AggregateGpuPlan::new(
        view,
        AggregateGpuPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(plan.group().same_group(view));
    assert_eq!(plan.numeric.tuple_count, 1);
    assert_eq!(
        plan.numeric.tuples,
        [super::super::preparation::bits(-3), 1]
    );
}
