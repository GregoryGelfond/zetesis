use super::*;
use crate::{AggregateLimits, FormulaParts, Node};
use zetesis_core::Value;

fn prefix() -> FormulaNodes {
    FormulaNodes::new(FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap())
}

fn attempt(
    nodes: &mut FormulaNodes,
    limits: AggregateFamilyLimits,
    cancellation: &Cancellation,
) -> ValueExtremumFamilyAttempt {
    let values = [Value::Number(2), Value::Number(1), Value::Number(3)];
    let elements = values.each_ref().map(|value| ValueExtremumElement {
        value: value.into(),
        condition: 0,
    });
    let guards = values.each_ref().map(|bound| ValueExtremumGuard {
        comparison: AggregateComparison::Eq,
        bound: bound.into(),
    });
    nodes.append_value_extremum_family_refs(
        &elements,
        AggregateExtremum::Min,
        &guards,
        limits,
        cancellation,
    )
}

fn build(
    nodes: &mut FormulaNodes,
    limits: AggregateFamilyLimits,
    cancellation: &Cancellation,
) -> Result<AggregateFamilyBuild, AggregateError> {
    attempt(nodes, limits, cancellation).result
}

#[test]
fn full_carrier_keeps_ties_occurrences_and_both_endpoints() {
    let values = [
        Value::Infimum,
        Value::Number(-1),
        Value::Number(0),
        Value::Number(2),
        Value::Symbol("a".into()),
        Value::Symbol("b".into()),
        Value::String("a".into()),
        Value::Supremum,
        Value::Number(0),
    ];
    let elements = values.each_ref().map(|value| ValueExtremumElement {
        value: value.into(),
        condition: 0,
    });
    let bounds = [Value::Supremum, Value::Number(1), Value::Infimum];
    for extremum in [AggregateExtremum::Min, AggregateExtremum::Max] {
        // Every different guard family receives the same complete rank carrier.
        for bound in &bounds {
            let mut nodes = prefix();
            let guard = ValueExtremumGuard {
                comparison: AggregateComparison::Eq,
                bound: bound.into(),
            };
            nodes
                .append_value_extremum_family_refs(
                    &elements,
                    extremum,
                    &[guard],
                    AggregateFamilyLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap();
            assert_eq!(nodes.parts().rows().len(), 1);
            assert_eq!(nodes.parts().extremum_cuts().len(), 2);
            assert_eq!(nodes.parts().conditions(), [0; 9]);
            assert_eq!(nodes.parts().numeric_values(), [0, 1, 2, 3, 4, 5, 6, 7, 2]);
            let NodeView::ExtremumCut(cut) = nodes.view().node(1).unwrap() else {
                panic!("native cut");
            };
            assert_eq!(cut.row.rank_count, 8);
            let (lower, upper) = match bound {
                Value::Supremum => (7, 8),
                Value::Infimum => (0, 1),
                Value::Number(1) => (3, 3),
                _ => unreachable!(),
            };
            let expected = match extremum {
                AggregateExtremum::Min => [
                    ExtremumCutRule::Prefix(upper),
                    ExtremumCutRule::Prefix(lower),
                ],
                AggregateExtremum::Max => [
                    ExtremumCutRule::Suffix(lower),
                    ExtremumCutRule::Suffix(upper),
                ],
            };
            for (descriptor, expected) in nodes.parts().extremum_cuts().iter().zip(expected) {
                assert_eq!(descriptor.row, 0);
                assert_eq!(descriptor.rule, expected);
            }
        }
    }
}

#[test]
fn endpoint_only_and_empty_rows_keep_a_complete_carrier() {
    for values in [vec![], vec![Value::Infimum], vec![Value::Supremum]] {
        let elements: Vec<_> = values
            .iter()
            .map(|value| ValueExtremumElement {
                value: value.into(),
                condition: 0,
            })
            .collect();
        for extremum in [AggregateExtremum::Min, AggregateExtremum::Max] {
            let mut nodes = prefix();
            let bounds = [Value::Supremum, Value::Infimum];
            let guards = bounds.each_ref().map(|bound| ValueExtremumGuard {
                comparison: AggregateComparison::Ne,
                bound: bound.into(),
            });
            let result = nodes
                .append_value_extremum_family_refs(
                    &elements,
                    extremum,
                    &guards,
                    AggregateFamilyLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap();
            assert_eq!(nodes.parts().rows().len(), 1);
            assert_eq!(nodes.parts().extremum_cuts().len(), 4);
            for node in 1..5 {
                let NodeView::ExtremumCut(cut) = nodes.view().node(node).unwrap() else {
                    panic!("native cut");
                };
                assert_eq!(cut.row.rank_count, 2);
            }
            for (index, &root) in result.roots().iter().enumerate() {
                assert_eq!(
                    nodes.view().node(root).unwrap(),
                    NodeView::Implies(1 + index * 2, 2 + index * 2)
                );
            }
        }
    }
}

#[test]
fn every_work_refusal_rolls_back_the_family() {
    let complete = attempt(
        &mut prefix(),
        AggregateFamilyLimits::default(),
        &Cancellation::default(),
    );
    let peak = complete.auxiliary_peak_bytes;
    let complete = complete.result.unwrap();
    for allowance in 0..complete.statistics().work {
        let mut nodes = prefix();
        let limits = AggregateFamilyLimits {
            aggregate: AggregateLimits {
                max_work: allowance,
                ..AggregateLimits::default()
            },
            ..AggregateFamilyLimits::default()
        };
        let attempt = attempt(&mut nodes, limits, &Cancellation::default());
        let error = attempt.result.unwrap_err();
        assert_eq!(error.kind(), AggregateErrorKind::WorkLimit);
        assert_eq!(error.statistics().work, allowance);
        assert!(attempt.auxiliary_peak_bytes <= peak);
        assert_eq!(nodes.view().len(), 1);
        assert!(nodes.parts().operands().is_empty());
        assert!(nodes.parts().rows().is_empty());
        assert!(nodes.parts().conditions().is_empty());
        assert!(nodes.parts().numeric_values().is_empty());
        assert!(nodes.parts().extremum_cuts().is_empty());
    }
}

#[test]
fn family_state_ceiling_is_inclusive() {
    let states = ValueExtremumFamilySpace::new(3, 3).unwrap().states();
    for count in [states - 1, states] {
        let mut nodes = prefix();
        let limits = AggregateFamilyLimits {
            aggregate: AggregateLimits {
                max_states: count,
                ..AggregateLimits::default()
            },
            ..AggregateFamilyLimits::default()
        };
        let result = build(&mut nodes, limits, &Cancellation::default());
        if count == states {
            assert_eq!(result.unwrap().statistics().states, states);
        } else {
            assert_eq!(result.unwrap_err().kind(), AggregateErrorKind::StateLimit);
            assert_eq!(nodes.view().len(), 1);
        }
    }
}

#[test]
fn empty_guards_still_validate_conditions() {
    let value = Value::Number(0);
    let limits = AggregateFamilyLimits {
        aggregate: AggregateLimits {
            max_states: 0,
            ..AggregateLimits::default()
        },
        max_guards: 0,
    };
    for condition in [0, 1] {
        let mut nodes = prefix();
        let elements = [ValueExtremumElement {
            value: (&value).into(),
            condition,
        }];
        let result = nodes
            .append_value_extremum_family_refs(
                &elements,
                AggregateExtremum::Min,
                &[],
                limits,
                &Cancellation::default(),
            )
            .result;
        if condition == 0 {
            assert!(result.unwrap().roots().is_empty());
        } else {
            assert_eq!(
                result.unwrap_err().kind(),
                AggregateErrorKind::InvalidCondition { element: 0 }
            );
        }
        assert_eq!(nodes.view().len(), 1);
    }
}

#[test]
fn cancelled_family_preserves_the_original_prefix() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut nodes = prefix();
    let error = build(&mut nodes, AggregateFamilyLimits::default(), &cancellation).unwrap_err();
    assert_eq!(
        error.kind(),
        AggregateErrorKind::Control(zetesis_cpu::Stop::Cancelled)
    );
    assert_eq!(nodes.view().len(), 1);
}

#[test]
fn checked_space_refuses_dimension_overflow() {
    assert!(ValueExtremumFamilySpace::new(usize::MAX, 1).is_none());
    assert!(ValueExtremumFamilySpace::new(0, usize::MAX).is_none());
    let space = ValueExtremumFamilySpace::new(3, 3).unwrap();
    assert_eq!(space.states(), 17);
    assert_eq!(
        space.storage_bytes(),
        11 * size_of::<usize>()
            + 3 * size_of::<u64>()
            + 6 * size_of::<ExtremumCutRule>()
            + size_of::<Preparation>()
    );
}

#[test]
fn refused_initialization_reports_allocated_capacity() {
    let mut nodes = prefix();
    let attempt = attempt(
        &mut nodes,
        AggregateFamilyLimits {
            aggregate: AggregateLimits {
                max_work: 5,
                ..AggregateLimits::default()
            },
            ..AggregateFamilyLimits::default()
        },
        &Cancellation::default(),
    );
    let error = attempt.result.unwrap_err();
    assert_eq!(error.kind(), AggregateErrorKind::WorkLimit);
    assert!(
        attempt.auxiliary_peak_bytes
            >= ValueExtremumFamilySpace::new(3, 3).unwrap().storage_bytes() as u128
    );
    assert_eq!(nodes.view().len(), 1);
}

#[test]
fn non_work_limits_preserve_the_prefix() {
    let defaults = AggregateFamilyLimits::default();
    for (limits, expected) in [
        (
            AggregateFamilyLimits {
                max_guards: 2,
                ..defaults
            },
            AggregateErrorKind::GuardLimit,
        ),
        (
            AggregateFamilyLimits {
                aggregate: AggregateLimits {
                    max_elements: 2,
                    ..defaults.aggregate
                },
                ..defaults
            },
            AggregateErrorKind::ElementLimit,
        ),
        (
            AggregateFamilyLimits {
                aggregate: AggregateLimits {
                    max_nodes: 1,
                    ..defaults.aggregate
                },
                ..defaults
            },
            AggregateErrorKind::NodeLimit,
        ),
        (
            AggregateFamilyLimits {
                aggregate: AggregateLimits {
                    max_operands: 0,
                    ..defaults.aggregate
                },
                ..defaults
            },
            AggregateErrorKind::OperandLimit,
        ),
    ] {
        let mut nodes = prefix();
        let error = build(&mut nodes, limits, &Cancellation::default()).unwrap_err();
        assert_eq!(error.kind(), expected);
        assert_eq!(nodes.view().len(), 1);
        assert!(nodes.parts().operands().is_empty());
        assert!(nodes.parts().rows().is_empty());
        assert!(nodes.parts().conditions().is_empty());
        assert!(nodes.parts().numeric_values().is_empty());
        assert!(nodes.parts().extremum_cuts().is_empty());
    }
}

#[test]
fn zero_capacity_attempt_keeps_an_observable_receipt() {
    let mut nodes = prefix();
    transaction_value(
        Destination::retained(nodes.transaction()),
        AggregateLimits::default(),
        &Cancellation::default(),
        |builder| {
            let mut peak = 0;
            Preparation::default().observe(builder, &mut peak, Ok(()))?;
            assert_eq!(peak, size_of::<Preparation>() as u128);
            Ok(())
        },
    )
    .unwrap();
}
