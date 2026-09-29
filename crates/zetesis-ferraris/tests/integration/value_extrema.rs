//! Ordered values are checked against independent complete failing-subset formulas.
use crate::support::aggregate_theories::{COMPARISONS, EXTREMA, prefix, push};
use crate::support::worlds::eval;
use zetesis_core::{Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AggregateComparison as Comparison, AggregateErrorKind as Error, AggregateExtremum as Extremum,
    AggregateLimits, Node, ValueExtremumElement as Element, append_value_extremum,
    append_value_extremum_refs,
};

fn ordered() -> Vec<Value> {
    let structure = |nodes| Value::from_nodes(nodes, ValueLimits::default()).unwrap();
    let function = |sign, arity| ValueNode::Function {
        name: "f".into(),
        sign,
        arity,
    };
    vec![
        Value::Infimum,
        Value::Number(-2),
        Value::Symbol("a".into()),
        structure(vec![function(Sign::Negative, 0)]),
        Value::String("a".into()),
        structure(vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)]),
        structure(vec![function(Sign::Positive, 1), ValueNode::Number(1)]),
        structure(vec![function(Sign::Negative, 1), ValueNode::Number(1)]),
        Value::Supremum,
    ]
}
// Independent explicit ASP ordering for this finite carrier; never storage Ord
// or the production comparator. Source regressions cover a broader carrier.
fn rank(value: &Value) -> usize {
    match value {
        Value::Infimum => 0,
        Value::Number(-2) => 1,
        Value::Symbol(_) => 2,
        Value::String(_) => 4,
        Value::Structured(value) => match value.nodes()[0] {
            ValueNode::Function { arity: 0, .. } => 3,
            ValueNode::Tuple { .. } => 5,
            ValueNode::Function {
                sign: Sign::Positive,
                ..
            } => 6,
            ValueNode::Function {
                sign: Sign::Negative,
                ..
            } => 7,
            _ => panic!("outside independently ordered fixture"),
        },
        Value::Supremum => 8,
        Value::Number(_) => panic!("outside independently ordered fixture"),
    }
}
fn value<'a>(extremum: Extremum, values: impl Iterator<Item = &'a Value>) -> usize {
    match extremum {
        Extremum::Min => values.map(rank).min().unwrap_or(8),
        Extremum::Max => values.map(rank).max().unwrap_or(0),
    }
}
fn holds(comparison: Comparison, value: usize, bound: &Value) -> bool {
    let bound = rank(bound);
    match comparison {
        Comparison::Eq => value == bound,
        Comparison::Ne => value != bound,
        Comparison::Lt => value < bound,
        Comparison::Le => value <= bound,
        Comparison::Gt => value > bound,
        Comparison::Ge => value >= bound,
    }
}

// Recursive evaluation is only the small independent test interpreter. It
// materializes neither the production topological mask nor its witness formula.

fn reference(
    elements: &[Element],
    extremum: Extremum,
    comparison: Comparison,
    bound: &Value,
) -> (Vec<Node>, usize) {
    let mut nodes = prefix();
    let mut root = 1;
    for subset in 0usize..(1 << elements.len()) {
        let selected = elements
            .iter()
            .enumerate()
            .filter(|(index, _)| subset & (1 << index) != 0)
            .map(|(_, element)| &element.value);
        if holds(comparison, value(extremum, selected), bound) {
            continue;
        }
        let mut antecedent = 1;
        let mut consequent = 0;
        for (index, element) in elements.iter().enumerate() {
            if subset & (1 << index) == 0 {
                consequent = push(&mut nodes, Node::Or(consequent, element.condition));
            } else {
                antecedent = push(&mut nodes, Node::And(antecedent, element.condition));
            }
        }
        let implication = push(&mut nodes, Node::Implies(antecedent, consequent));
        root = push(&mut nodes, Node::And(root, implication));
    }
    (nodes, root)
}

fn verify(elements: &[Element], extremum: Extremum, comparison: Comparison, bound: &Value) {
    let mut nodes = prefix();
    let built = append_value_extremum(
        &mut nodes,
        elements,
        extremum,
        comparison,
        bound,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let predicate = zetesis_core::Predicate::new("value", 1).unwrap();
    let mut inputs: Vec<_> = elements
        .iter()
        .map(|element| {
            zetesis_core::Atom::new(predicate.clone(), vec![element.value.clone()]).unwrap()
        })
        .collect();
    inputs.push(zetesis_core::Atom::new(predicate, vec![bound.clone()]).unwrap());
    let catalog = zetesis_core::AtomCatalog::new(inputs).unwrap();
    let value = |slot| catalog.atoms().at(slot).unwrap().values().at(0).unwrap();
    let mut borrowed_nodes = prefix();
    let borrowed = append_value_extremum_refs(
        &mut borrowed_nodes,
        elements.iter().enumerate().map(|(index, element)| Element {
            value: value(index),
            condition: element.condition,
        }),
        extremum,
        comparison,
        value(elements.len()),
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let mut borrowed_root = borrowed.root();
    let (mut specified, expected) = reference(elements, extremum, comparison, bound);
    let mut root = built.root();
    let mut expected_root = expected;
    // Positive, default-negated and double-default-negated formulas.
    for _ in 0..3 {
        for outer in 0..4 {
            assert_eq!(
                eval(&nodes, root, outer, None),
                eval(&specified, expected_root, outer, None)
            );
            assert_eq!(
                eval(&borrowed_nodes, borrowed_root, outer, None),
                eval(&specified, expected_root, outer, None)
            );
            for inner in 0..4 {
                assert_eq!(
                    eval(&borrowed_nodes, borrowed_root, inner, Some(outer)),
                    eval(&specified, expected_root, inner, Some(outer))
                );
                assert_eq!(
                    eval(&nodes, root, inner, Some(outer)),
                    eval(&specified, expected_root, inner, Some(outer)),
                    "{elements:?} {extremum:?} {comparison:?} {bound:?} M={outer} J={inner}"
                );
            }
        }
        borrowed_root = push(&mut borrowed_nodes, Node::Implies(borrowed_root, 0));
        root = push(&mut nodes, Node::Implies(root, 0));
        expected_root = push(&mut specified, Node::Implies(expected_root, 0));
    }
}
#[test]
fn complete_value_order_and_every_guard_preserve_all_original_and_frozen_worlds() {
    let values = ordered();
    for left in &values {
        for right in &values {
            for condition in [2, 4, 5, 9, 10] {
                let elements = [
                    Element {
                        value: left.clone(),
                        condition: 2,
                    },
                    Element {
                        value: right.clone(),
                        condition,
                    },
                ];
                for extremum in EXTREMA {
                    for comparison in COMPARISONS {
                        for bound in &values {
                            verify(&elements, extremum, comparison, bound);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn empty_and_tied_complete_values_keep_their_sentinel_and_eligibility_semantics() {
    for extremum in EXTREMA {
        for comparison in COMPARISONS {
            for value in ordered() {
                verify(&[], extremum, comparison, &value);
                let elements = [
                    Element {
                        value: value.clone(),
                        condition: 10,
                    },
                    Element {
                        value: value.clone(),
                        condition: 3,
                    },
                    Element {
                        value: value.clone(),
                        condition: 5,
                    },
                ];
                verify(&elements, extremum, comparison, &value);
            }
        }
    }
}
#[test]
fn exact_limits_restore_the_existing_prefix() {
    let value = ordered()[6].clone();
    let elements = [
        Element {
            value: value.clone(),
            condition: 2,
        },
        Element {
            value: value.clone(),
            condition: 3,
        },
    ];
    let mut nodes = prefix();
    let built = append_value_extremum(
        &mut nodes,
        &elements,
        Extremum::Min,
        Comparison::Eq,
        &value,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let exact = AggregateLimits {
        max_elements: 2,
        max_nodes: nodes.len(),
        max_work: built.statistics().work,
        max_states: 0,
        max_subsets: 0,
    };
    let mut repeated = prefix();
    assert_eq!(
        append_value_extremum(
            &mut repeated,
            &elements,
            Extremum::Min,
            Comparison::Eq,
            &value,
            exact,
            &Cancellation::default()
        )
        .unwrap(),
        built
    );
    assert_eq!(repeated, nodes);
    for (limits, expected) in [
        (
            AggregateLimits {
                max_elements: 1,
                ..exact
            },
            Error::ElementLimit,
        ),
        (
            AggregateLimits {
                max_nodes: exact.max_nodes - 1,
                ..exact
            },
            Error::NodeLimit,
        ),
        (
            AggregateLimits {
                max_work: exact.max_work - 1,
                ..exact
            },
            Error::WorkLimit,
        ),
    ] {
        let mut nodes = prefix();
        let error = append_value_extremum(
            &mut nodes,
            &elements,
            Extremum::Min,
            Comparison::Eq,
            &value,
            limits,
            &Cancellation::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), expected);
        assert_eq!(nodes, prefix());
    }
}

#[test]
fn bad_inputs_and_cancellation_restore_the_existing_prefix() {
    let value = ordered()[6].clone();
    let mut nodes = prefix();
    let error = append_value_extremum(
        &mut nodes,
        &[Element {
            value: value.clone(),
            condition: usize::MAX,
        }],
        Extremum::Max,
        Comparison::Eq,
        &value,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), Error::InvalidCondition { element: 0 });
    assert_eq!(nodes, prefix());
    nodes.push(Node::And(usize::MAX, 0));
    let bad = nodes.clone();
    assert!(matches!(
        append_value_extremum(
            &mut nodes,
            &[],
            Extremum::Min,
            Comparison::Eq,
            &value,
            AggregateLimits::default(),
            &Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        Error::InvalidPrefix { .. }
    ));
    assert_eq!(nodes, bad);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut nodes = prefix();
    assert!(matches!(
        append_value_extremum(
            &mut nodes,
            &[],
            Extremum::Min,
            Comparison::Eq,
            &value,
            AggregateLimits::default(),
            &cancellation
        )
        .unwrap_err()
        .kind(),
        Error::Control(_)
    ));
    assert_eq!(nodes, prefix());
}

#[test]
fn borrowed_elements_cannot_name_newly_appended_nodes() {
    let value = Value::Number(1);
    let mut nodes = prefix();
    let original = nodes.clone();
    let element = Element {
        value: (&value).into(),
        condition: nodes.len(),
    };
    let error = append_value_extremum_refs(
        &mut nodes,
        std::iter::once(element),
        Extremum::Min,
        Comparison::Eq,
        (&value).into(),
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error.kind(),
        Error::InvalidCondition { element: 0 }
    ));
    assert_eq!(nodes, original);
}

#[test]
fn borrowed_elements_obey_the_actual_element_limit() {
    let value = Value::Number(1);
    let mut nodes = prefix();
    let original = nodes.clone();
    let elements = (0..2).filter(|_| true).map(|_| Element {
        value: (&value).into(),
        condition: 2,
    });
    let limits = AggregateLimits {
        max_elements: 1,
        ..AggregateLimits::default()
    };
    let error = append_value_extremum_refs(
        &mut nodes,
        elements,
        Extremum::Max,
        Comparison::Eq,
        (&value).into(),
        limits,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(error.kind(), Error::ElementLimit));
    assert_eq!(nodes, original);
}
