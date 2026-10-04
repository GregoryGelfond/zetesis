use super::super::{GroundAggregate, Purpose};
use super::*;
use crate::ExpansionLimits;
use crate::FormulaLimits;
use crate::expansion::Budget;
use crate::formula_ir::{Expression, Operation};
use crate::formula_support::testing::Fixture;
use std::sync::Arc;
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};
use themelios_program::{
    program::Relation,
    term::{BinaryOp, EvalError},
};
use zetesis_core::Value;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AggregateErrorKind, Interpretation, Limits, models, models_reduct};
use zetesis_ferraris::{Node, Theory, append_aggregate};

fn location() -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(19),
        span: Span::empty(ByteOffset::new(0)),
    })
}

fn with_guards<T>(
    limits: &FormulaLimits,
    weights: [i32; 2],
    guards: &[(Relation, Value)],
    run: impl FnOnce(
        &mut Builder<'_, '_, '_>,
        &GroundAggregate,
        Vec<AggregateGuard>,
        &Binding<'static>,
    ) -> T,
) -> T {
    let mut fixture = Fixture::default();
    let guards = guards
        .iter()
        .map(|(relation, value)| AggregateGuard {
            relation: *relation,
            bound: Expression {
                nodes: vec![Operation::Constant(fixture.scalar(value, location()))],
            },
        })
        .collect();
    fixture.with(location(), |_, computation, counters| {
        let binding = Binding::new(computation, limits, counters, location()).unwrap();
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )
        .unwrap();
        builder.initialize(location()).unwrap();
        let a = builder.node(Node::Atom(0), location()).unwrap();
        let b = builder.node(Node::Atom(1), location()).unwrap();
        let absent = builder.neg(b, location()).unwrap();
        let first = builder.or(a, absent, location()).unwrap();
        let second = builder.node(Node::Implies(a, b), location()).unwrap();
        // A nontrivial valid prefix makes duplicated validation observable.
        let mut tail = second;
        for _ in 0..32 {
            tail = builder
                .node(Node::Implies(tail, first), location())
                .unwrap();
        }
        let mut elements = Buffer::new(
            builder.computation,
            limits,
            &mut builder.counters,
            location(),
        )
        .unwrap();
        for (weight, condition) in weights.into_iter().zip([first, second]) {
            elements
                .push(
                    AggregateElement { weight, condition },
                    builder.computation,
                    limits,
                    &mut builder.counters,
                    location(),
                )
                .unwrap();
        }
        let elements = GroundAggregate::Numeric(Arc::new(elements));
        run(&mut builder, &elements, guards, &binding)
    })
}

fn numeric(guards: &[(Relation, Value)]) -> Vec<NumericGuard> {
    guards
        .iter()
        .map(|(relation, bound)| {
            let Value::Number(bound) = bound else {
                panic!("numeric test guards")
            };
            NumericGuard {
                comparison: aggregate_comparison(*relation),
                bound: i64::from(*bound),
            }
        })
        .collect()
}

fn sample() -> [(Relation, Value); 2] {
    [
        (Relation::Ge, Value::Number(1)),
        (Relation::Le, Value::Number(2)),
    ]
}

fn interpretation(theory: &Theory, bits: usize) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| bits & (1 << atom) != 0)).unwrap()
}

#[test]
fn grouped_guards_preserve_scalar_frozen_truth() {
    for guards in [
        sample(),
        [
            (Relation::Eq, Value::Number(1)),
            (Relation::Neq, Value::Number(2)),
        ],
        [
            (Relation::Gt, Value::Number(0)),
            (Relation::Lt, Value::Number(3)),
        ],
    ] {
        with_guards(
            &FormulaLimits::default(),
            [1, 2],
            &guards,
            |builder, elements, source, binding| {
                let mut scalar_nodes = builder.nodes.to_vec();
                let GroundAggregate::Numeric(entries) = &elements else {
                    unreachable!()
                };
                let mut scalar_root = VERUM;
                for guard in numeric(&guards) {
                    let build = append_aggregate(
                        &mut scalar_nodes,
                        entries.slice(),
                        guard.comparison,
                        guard.bound,
                        builder.limits.aggregate,
                        &Cancellation::default(),
                    )
                    .unwrap();
                    scalar_nodes.push(Node::And(scalar_root, build.root()));
                    scalar_root = scalar_nodes.len() - 1;
                }
                let root = builder
                    .aggregate_guards(elements, &source, binding, None, location())
                    .unwrap();
                let grouped =
                    Theory::new(2, builder.nodes.to_vec(), vec![root], builder.limits.theory)
                        .unwrap();
                let scalar =
                    Theory::new(2, scalar_nodes, vec![scalar_root], builder.limits.theory).unwrap();
                for candidate in 0..4 {
                    let outer = interpretation(&grouped, candidate);
                    let reference = interpretation(&scalar, candidate);
                    assert_eq!(
                        models(
                            &grouped,
                            &outer,
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap(),
                        models(
                            &scalar,
                            &reference,
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap()
                    );
                    for tested in 0..4 {
                        assert_eq!(
                            models_reduct(
                                &grouped,
                                &outer,
                                &interpretation(&grouped, tested),
                                Limits::default(),
                                &Cancellation::default()
                            )
                            .unwrap(),
                            models_reduct(
                                &scalar,
                                &reference,
                                &interpretation(&scalar, tested),
                                Limits::default(),
                                &Cancellation::default()
                            )
                            .unwrap()
                        );
                    }
                }
            },
        );
    }
}

#[test]
fn family_receipt_avoids_repeated_prefix_validation() {
    with_guards(
        &FormulaLimits::default(),
        [1, 2],
        &sample(),
        |builder, elements, _, _| {
            let GroundAggregate::Numeric(elements) = elements else {
                unreachable!()
            };
            let guards = numeric(&sample());
            let mut nodes = builder.nodes.to_vec();
            let scalar_work: u64 = guards
                .iter()
                .map(|guard| {
                    append_aggregate(
                        &mut nodes,
                        elements.slice(),
                        guard.comparison,
                        guard.bound,
                        builder.limits.aggregate,
                        &Cancellation::default(),
                    )
                    .unwrap()
                    .statistics()
                    .work
                })
                .sum();
            let before = builder.counters.accounting.work;
            let family = builder
                .append_guard_family(elements.slice(), &guards, guards.len(), location())
                .unwrap();
            assert!(family.build.statistics().work < scalar_work);
            assert_eq!(
                builder.counters.accounting.work - before,
                family.build.statistics().work
            );
        },
    );
}

#[test]
fn canonical_remapping_retains_only_checked_prefix() {
    with_guards(
        &FormulaLimits::default(),
        [1, 2],
        &sample(),
        |builder, elements, _, _| {
            let GroundAggregate::Numeric(elements) = elements else {
                unreachable!()
            };
            let guards = numeric(&sample());
            let prefix = builder.nodes.len();
            let first = builder
                .append_guard_family(elements.slice(), &guards, guards.len(), location())
                .unwrap();
            assert!(first.build.appended_nodes() > 0);
            builder.intern_appended(prefix, location()).unwrap();

            // Reindexing removed the compiler suffix. A subsequent family must
            // inspect the replacement nodes, but need not reinspect the prefix.
            let mut reference = builder.nodes.to_vec();
            assert!(reference.len() > prefix);
            let scalar = zetesis_ferraris::append_aggregate_family(
                &mut reference,
                elements.slice(),
                &guards,
                AggregateFamilyLimits {
                    aggregate: builder.aggregate_limits(),
                    max_guards: guards.len(),
                },
                &Cancellation::default(),
            )
            .unwrap();
            let reused = builder
                .append_guard_family(elements.slice(), &guards, guards.len(), location())
                .unwrap();
            assert_eq!(&*builder.nodes, reference);
            assert_eq!(reused.build.roots(), scalar.roots());
            assert_eq!(
                scalar.statistics().work - reused.build.statistics().work,
                prefix as u64
            );
        },
    );
}

#[test]
fn failed_family_work_survives_rollback() {
    let completed_work = with_guards(
        &FormulaLimits::default(),
        [1, 2],
        &sample(),
        |builder, elements, _, _| {
            let GroundAggregate::Numeric(elements) = elements else {
                unreachable!()
            };
            let guards = numeric(&sample());
            builder
                .append_guard_family(elements.slice(), &guards, guards.len(), location())
                .unwrap()
                .build
                .statistics()
                .work
        },
    );
    // Refuse the final accepted operation, after the shared threshold DAG has
    // appended nodes. Prefix-validation refusals alone cannot exercise rollback.
    let appended_prefix = completed_work - 1;
    for cap in [0, 1, 20, appended_prefix] {
        let mut limits = FormulaLimits::default();
        limits.aggregate.max_work = cap;
        with_guards(&limits, [1, 2], &sample(), |builder, elements, _, _| {
            let GroundAggregate::Numeric(elements) = elements else {
                unreachable!()
            };
            let before = builder.counters.accounting.work;
            let nodes = builder.nodes.to_vec();
            let guards = numeric(&sample());
            let result =
                builder.append_guard_family(elements.slice(), &guards, guards.len(), location());
            let Err(FormulaFailure::Aggregate {
                error,
                location: origin,
            }) = result
            else {
                panic!("family must refuse")
            };
            assert_eq!(error.kind(), AggregateErrorKind::WorkLimit);
            assert_eq!(origin, location());
            assert_eq!(error.statistics().work, cap);
            if cap == appended_prefix {
                assert!(error.statistics().nodes > 0);
            }
            assert_eq!(builder.counters.accounting.work - before, cap);
            assert_eq!(&*builder.nodes, nodes);
        });
    }
}

#[test]
fn refused_scalar_compilation_retains_spent_work() {
    // Both attempts do the same source work; only the compiler allowance differs.
    let work = [0, 5].map(|allowance| {
        let mut limits = FormulaLimits::default();
        limits.aggregate.max_work = allowance;
        with_guards(
            &limits,
            [1, 2],
            &[(Relation::Ge, Value::Number(1))],
            |builder, elements, guards, binding| {
                let before = builder.counters.accounting.work;
                let nodes = builder.nodes.to_vec();
                let result = builder.aggregate_guards(elements, &guards, binding, None, location());
                let Err(FormulaFailure::Aggregate { error, .. }) = result else {
                    panic!("compiler must exhaust its smaller allowance")
                };
                assert_eq!(error.kind(), AggregateErrorKind::WorkLimit);
                assert_eq!(error.statistics().work, allowance);
                assert_eq!(&*builder.nodes, nodes);
                builder.counters.accounting.work - before
            },
        )
    });
    assert_eq!(work[1] - work[0], 5);
}

#[test]
fn returned_roots_remain_live_during_remapping() {
    with_guards(
        &FormulaLimits::default(),
        [1, 2],
        &sample(),
        |builder, elements, _, _| {
            let GroundAggregate::Numeric(elements) = elements else {
                unreachable!()
            };
            let observer = builder.computation.lease();
            let before = builder
                .computation
                .allowance(&observer, builder.limits, location())
                .unwrap();
            let guards = numeric(&sample());
            let first = builder.nodes.len();
            let family = builder
                .append_guard_family(elements.slice(), &guards, guards.len(), location())
                .unwrap();
            let bytes = family.build.root_storage_bytes();
            assert!(bytes >= size_of::<Vec<usize>>() + guards.len() * size_of::<usize>());
            let canonical = builder.intern_appended(first, location()).unwrap();
            assert!(!canonical.is_empty());
            assert_eq!(
                before
                    - builder
                        .computation
                        .allowance(&observer, builder.limits, location())
                        .unwrap(),
                bytes
            );
            drop(family);
            assert_eq!(
                builder
                    .computation
                    .allowance(&observer, builder.limits, location())
                    .unwrap(),
                before
            );
        },
    );
}

#[test]
fn root_refusal_reports_the_configured_storage_limit() {
    let limits = FormulaLimits::default();
    let used = with_guards(&limits, [1, 2], &sample(), |builder, _, _, _| {
        let empty = builder.computation.lease();
        limits.max_support_bytes
            - builder
                .computation
                .allowance(&empty, &limits, location())
                .unwrap()
    });
    let requested = size_of::<Vec<usize>>() + sample().len() * size_of::<usize>();
    let limits = FormulaLimits {
        max_support_bytes: used + requested - 1,
        ..limits
    };
    with_guards(&limits, [1, 2], &sample(), |builder, elements, _, _| {
        let GroundAggregate::Numeric(elements) = elements else {
            unreachable!()
        };
        let nodes = builder.nodes.to_vec();
        let guards = numeric(&sample());
        let result =
            builder.append_guard_family(elements.slice(), &guards, guards.len(), location());
        let Err(FormulaFailure::Limit {
            resource,
            limit,
            observed,
            ..
        }) = result
        else {
            panic!("root storage must refuse")
        };
        assert_eq!(resource, FormulaResource::SupportBytes);
        assert_eq!(limit, limits.max_support_bytes as u128);
        assert_eq!(observed, (used + requested) as u128);
        assert_eq!(&*builder.nodes, nodes);
    });
}

#[test]
fn signed_guards_keep_independent_subset_ceilings() {
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_subsets = 4;
    with_guards(
        &limits,
        [-1, 2],
        &sample(),
        |builder, elements, guards, binding| {
            // Two signed elements require four subsets per guard. A family would
            // exceed this cumulative cap on its second guard.
            builder
                .aggregate_guards(elements, &guards, binding, None, location())
                .unwrap();
        },
    );
}

#[test]
fn numeric_guard_capture_retains_both_bounds() {
    let guards = [
        (Relation::Ge, Value::Number(1)),
        (Relation::Le, Value::Number(1)),
    ];
    with_guards(
        &FormulaLimits::default(),
        [1, 1],
        &guards,
        |builder, elements, guards, binding| {
            let mut capture = crate::formula_count_plan::Bounds::new(2);
            builder
                .aggregate_guards_with_capture(
                    elements,
                    &guards,
                    binding,
                    None,
                    location(),
                    Some(&mut capture),
                )
                .unwrap();
            assert!(matches!(
                capture,
                crate::formula_count_plan::Bounds::Numeric { lower: 1, upper: 1 }
            ));
        },
    );
}

#[test]
fn logical_guards_exclude_numeric_capture() {
    let guards = [
        (Relation::Lt, Value::Symbol("z".into())),
        (Relation::Ge, Value::Number(1)),
    ];
    with_guards(
        &FormulaLimits::default(),
        [1, 1],
        &guards,
        |builder, elements, guards, binding| {
            let mut capture = crate::formula_count_plan::Bounds::new(2);
            builder
                .aggregate_guards_with_capture(
                    elements,
                    &guards,
                    binding,
                    None,
                    location(),
                    Some(&mut capture),
                )
                .unwrap();
            assert!(matches!(
                capture,
                crate::formula_count_plan::Bounds::NonNumeric
            ));
        },
    );
}

#[test]
fn false_logical_guards_preserve_later_errors() {
    let guards = [
        (Relation::Ge, Value::Symbol("z".into())),
        (Relation::Ge, Value::Number(i32::MAX)),
    ];
    with_guards(
        &FormulaLimits::default(),
        [1, 1],
        &guards,
        |builder, elements, mut guards, binding| {
            guards[1]
                .bound
                .nodes
                .push(Operation::Binary(BinaryOp::Add, 0, 0));
            let result = builder.aggregate_guards(elements, &guards, binding, None, location());
            assert!(matches!(
                result,
                Err(FormulaFailure::Expansion(
                    crate::ExpansionFailure::Evaluation {
                        error: EvalError::Overflow,
                        ..
                    }
                ))
            ));
        },
    );
}

#[test]
fn logical_guard_families_need_no_threshold_budget() {
    let guards = [
        (Relation::Lt, Value::Symbol("a".into())),
        (Relation::Le, Value::Symbol("z".into())),
    ];
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_work = 0;
    with_guards(
        &limits,
        [1, 1],
        &guards,
        |builder, elements, guards, binding| {
            assert_eq!(
                builder
                    .aggregate_guards(elements, &guards, binding, None, location())
                    .unwrap(),
                VERUM
            );
        },
    );
}
