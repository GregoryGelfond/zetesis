//! Direct original/frozen aggregate evaluation against existing exact lowering.

use crate::support::aggregate_theories::COMPARISONS;
use zetesis_core::{Sign, Value as Term, ValueLimits, ValueNode};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement, AggregateExtremum,
    AggregateLimits, Theory, ValueExtremumElement, append_aggregate, append_value_extremum_refs,
    models, models_reduct,
    native_aggregate::{self as native, Bound, Function, Group, Guard, Tuple},
};
use zetesis_theory_support::{
    aggregate::{FUNCTIONS, ferraris_theory as prefix},
    theories::interpretation as world,
};

fn tuples(weights: &[Term], conditions: &[usize]) -> Vec<Tuple> {
    weights
        .iter()
        .zip(conditions)
        .enumerate()
        .map(|(index, (value, condition))| Tuple {
            key: vec![value.clone(), Term::Number(i32::try_from(index).unwrap())],
            condition: *condition,
        })
        .collect()
}

fn lowered(group: &Group, comparison: Comparison, bound: &Term) -> Theory {
    let mut nodes = zetesis_ferraris::FormulaNodes::new(
        zetesis_ferraris::FormulaParts::new(
            group.theory().nodes().to_vec(),
            group.theory().operands().to_vec(),
        )
        .unwrap(),
    );
    let root = match group.function() {
        Function::Count | Function::Sum | Function::SumPlus => {
            let Term::Number(bound) = bound else {
                panic!("numeric lowering fixture requires an integer guard");
            };
            let elements: Vec<_> = group
                .tuples()
                .iter()
                .map(|tuple| {
                    let weight = match group.function() {
                        Function::Count => 1,
                        Function::Sum => match tuple
                            .key
                            .first()
                            .map(zetesis_core::catalog::TermRef::descriptor)
                        {
                            Some(zetesis_core::ValueNodeRef::Number(value)) => value,
                            _ => 0,
                        },
                        Function::SumPlus => {
                            match tuple
                                .key
                                .first()
                                .map(zetesis_core::catalog::TermRef::descriptor)
                            {
                                Some(zetesis_core::ValueNodeRef::Number(value)) => value.max(0),
                                _ => 0,
                            }
                        }
                        Function::Min | Function::Max => unreachable!("numeric fixture"),
                    };
                    AggregateElement {
                        weight,
                        condition: tuple.condition,
                    }
                })
                .collect();
            append_aggregate(
                &mut nodes,
                &elements,
                comparison,
                i64::from(*bound),
                AggregateLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .root()
        }
        Function::Min | Function::Max => {
            let elements: Vec<_> = group
                .tuples()
                .iter()
                .filter_map(|tuple| {
                    tuple.key.first().map(|value| ValueExtremumElement {
                        value,
                        condition: tuple.condition,
                    })
                })
                .collect();
            let kind = if group.function() == Function::Min {
                AggregateExtremum::Min
            } else {
                AggregateExtremum::Max
            };
            append_value_extremum_refs(
                &mut nodes,
                elements.into_iter(),
                kind,
                comparison,
                bound.into(),
                AggregateLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .root()
        }
    };
    Theory::new(
        group.theory().atom_count(),
        nodes.into_parts(),
        vec![root],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn compare_worlds(group: &Group, lowered: &Theory) {
    let cancellation = Cancellation::default();
    for m in 0..4 {
        let candidate = world(group.theory(), m);
        let lowered_candidate = world(lowered, m);
        let original = models(
            lowered,
            &lowered_candidate,
            zetesis_ferraris::Limits::default(),
            &cancellation,
        )
        .unwrap();
        for j in 0..4 {
            let tested = world(group.theory(), j);
            let masks = group
                .eligibility(
                    &candidate,
                    Some(&tested),
                    native::EligibilityLimits::default(),
                    &cancellation,
                )
                .unwrap();
            let direct = masks
                .reduce(native::ReductionLimits::default(), &cancellation)
                .unwrap();
            assert_eq!(direct.original().holds(), original);
            let frozen = models_reduct(
                lowered,
                &lowered_candidate,
                &world(lowered, j),
                zetesis_ferraris::Limits::default(),
                &cancellation,
            )
            .unwrap();
            assert_eq!(
                direct.reduct_truth(),
                Some(frozen),
                "{:?}, M={m}, J={j}",
                group.function()
            );
        }
    }
}

#[test]
fn native_guards_match_lowering_for_every_frozen_world() {
    let theory = prefix();
    for function in FUNCTIONS {
        for conditions in [[2, 3, 9], [4, 5, 8], [6, 7, 10]] {
            for comparison in COMPARISONS {
                for bound in [-3, -1, 0, 1, 2, 4, 7] {
                    let bound = Term::Number(bound);
                    let group = Group::new(
                        &theory,
                        function,
                        tuples(
                            &[Term::Number(-2), Term::Number(3), Term::Number(0)],
                            &conditions,
                        ),
                        vec![Guard {
                            comparison,
                            bound: Bound::Term(bound.clone()),
                        }],
                        native::AdmissionLimits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap();
                    compare_worlds(&group, &lowered(&group, comparison, &bound));
                }
            }
        }
    }
}

#[test]
fn neutral_weights_preserve_lowered_frozen_truth() {
    let theory = prefix();
    let weights = [
        Term::Number(-3),
        Term::Number(0),
        Term::Number(4),
        Term::Symbol("a".into()),
        Term::String("a".into()),
        Term::Infimum,
        Term::Supremum,
    ];
    for function in [Function::Count, Function::Sum, Function::SumPlus] {
        for comparison in COMPARISONS {
            for bound in [-3, 0, 1, 4, 7] {
                let bound = Term::Number(bound);
                let group = Group::new(
                    &theory,
                    function,
                    tuples(&weights, &[2, 3, 4, 5, 6, 7, 9]),
                    vec![Guard {
                        comparison,
                        bound: Bound::Term(bound.clone()),
                    }],
                    native::AdmissionLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                compare_worlds(&group, &lowered(&group, comparison, &bound));
            }
        }
    }
}

fn terms() -> Vec<Term> {
    vec![
        Term::Infimum,
        Term::Number(-2),
        Term::Symbol("a".into()),
        Term::from_nodes(
            vec![ValueNode::Function {
                name: "a".into(),
                sign: Sign::Negative,
                arity: 0,
            }],
            ValueLimits::default(),
        )
        .unwrap(),
        Term::String("a".into()),
        Term::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
            ValueLimits::default(),
        )
        .unwrap(),
        Term::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f".into(),
                    sign: Sign::Positive,
                    arity: 1,
                },
                ValueNode::Number(1),
            ],
            ValueLimits::default(),
        )
        .unwrap(),
        Term::Supremum,
    ]
}

#[test]
fn ordered_extrema_match_lowering_across_term_classes() {
    let theory = prefix();
    let terms = terms();
    for function in [Function::Min, Function::Max] {
        for comparison in COMPARISONS {
            for bound in &terms {
                let conditions = [2, 3, 4, 5, 6, 7, 9, 10];
                let group = Group::new(
                    &theory,
                    function,
                    tuples(&terms, &conditions),
                    vec![Guard {
                        comparison,
                        bound: Bound::Term(bound.clone()),
                    }],
                    native::AdmissionLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                compare_worlds(&group, &lowered(&group, comparison, bound));
            }
        }
    }
}

#[test]
fn empty_keys_retain_their_function_specific_contribution() {
    let theory = prefix();
    for function in FUNCTIONS {
        let bound = match function {
            Function::Count => Term::Number(1),
            Function::Sum | Function::SumPlus => Term::Number(0),
            Function::Min => Term::Supremum,
            Function::Max => Term::Infimum,
        };
        let group = Group::new(
            &theory,
            function,
            vec![Tuple {
                key: vec![],
                condition: 2,
            }],
            vec![Guard {
                comparison: Comparison::Eq,
                bound: Bound::Term(bound.clone()),
            }],
            native::AdmissionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        compare_worlds(&group, &lowered(&group, Comparison::Eq, &bound));
        assert_eq!(group.tuples().len(), 1);
    }
}

#[test]
fn wide_sums_never_narrow_to_source_integers() {
    let theory = prefix();
    for (function, value, expected) in [
        (Function::Sum, i32::MAX, 2 * i128::from(i32::MAX)),
        (Function::Sum, i32::MIN, 2 * i128::from(i32::MIN)),
        (Function::SumPlus, i32::MAX, 2 * i128::from(i32::MAX)),
    ] {
        let group = Group::new(
            &theory,
            function,
            tuples(&[Term::Number(value), Term::Number(value)], &[2, 2]),
            vec![Guard {
                comparison: Comparison::Eq,
                bound: Bound::Integer(expected),
            }],
            native::AdmissionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let result = group
            .reduce(
                &[true, true],
                Some(&[false, true]),
                native::ReductionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert!(result.original().holds());
        let native::Value::Integer(actual) = result.original().value() else {
            panic!("expected a wide integer");
        };
        assert_eq!(actual, expected);
        assert_eq!(result.reduct_truth(), Some(false));
    }
}

#[test]
fn numeric_comparison_respects_nonnumeric_term_order() {
    let theory = prefix();
    for (bound, comparison) in [
        (Term::Infimum, Comparison::Gt),
        (Term::Symbol("a".into()), Comparison::Lt),
        (Term::String("a".into()), Comparison::Lt),
        (Term::Supremum, Comparison::Lt),
    ] {
        let group = Group::new(
            &theory,
            Function::Sum,
            tuples(&[Term::Number(i32::MAX), Term::Number(i32::MAX)], &[2, 3]),
            vec![Guard {
                comparison,
                bound: Bound::Term(bound),
            }],
            native::AdmissionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert!(
            group
                .reduce(
                    &[true, true],
                    None,
                    native::ReductionLimits::default(),
                    &Cancellation::default()
                )
                .unwrap()
                .original()
                .holds()
        );
    }
}

#[test]
fn aggregate_inequality_is_not_default_negation() {
    let theory = prefix();
    let group = Group::new(
        &theory,
        Function::Count,
        tuples(&[Term::Number(1)], &[2]),
        vec![Guard {
            comparison: Comparison::Ne,
            bound: Bound::Integer(0),
        }],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let candidate = world(&theory, 1);
    let tested = world(&theory, 0);
    let result = group
        .eligibility(
            &candidate,
            Some(&tested),
            native::EligibilityLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .reduce(native::ReductionLimits::default(), &Cancellation::default())
        .unwrap();
    assert!(result.original().holds());
    assert_eq!(result.reduct_truth(), Some(false));
    // In contrast, not(count{p}=0) is true in every reduct interpretation when
    // M contains p; replacing inequality by that negation would lose this test.
}
