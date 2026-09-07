//! Numeric extrema are checked against full failing-subset implications.

use std::time::Instant;

use proptest::prelude::*;
use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement as Element,
    AggregateErrorKind as Error, AggregateExtremum as Extremum, AggregateLimits, AggregateProfile,
    ExtremumBound as Bound, Interpretation, Limits, Node, Theory, append_extremum, models,
    models_reduct,
};

const COMPARISONS: [Comparison; 6] = [
    Comparison::Eq,
    Comparison::Ne,
    Comparison::Lt,
    Comparison::Le,
    Comparison::Gt,
    Comparison::Ge,
];
const EXTREMA: [Extremum; 2] = [Extremum::Min, Extremum::Max];

fn prefix() -> Vec<Node> {
    vec![
        Node::False,
        Node::Implies(0, 0),
        Node::Atom(0),
        Node::Atom(1),
        Node::Implies(2, 0),
        Node::Implies(4, 0),
        Node::Implies(3, 0),
        Node::And(2, 3),
        Node::Or(2, 3),
        Node::Implies(2, 3),
        Node::Or(2, 4),
    ]
}

// Recursive evaluation is only the small independent test interpreter. It
// materializes neither the production topological mask nor its witness formula.
fn eval(nodes: &[Node], root: usize, world: u8, frozen: Option<u8>) -> bool {
    if frozen.is_some_and(|candidate| !eval(nodes, root, candidate, None)) {
        return false;
    }
    match nodes[root] {
        Node::False => false,
        Node::Atom(atom) => world & (1 << atom) != 0,
        Node::And(a, b) => eval(nodes, a, world, frozen) && eval(nodes, b, world, frozen),
        Node::Or(a, b) => eval(nodes, a, world, frozen) || eval(nodes, b, world, frozen),
        Node::Implies(a, b) => !eval(nodes, a, world, frozen) || eval(nodes, b, world, frozen),
    }
}

// The reference folds selected values directly. Its sentinel representation is
// outside both finite input and guard ranges; no threshold rewrite is shared.
fn value(extremum: Extremum, values: impl Iterator<Item = i32>) -> i128 {
    match extremum {
        Extremum::Min => values.map(i128::from).min().unwrap_or(i128::MAX),
        Extremum::Max => values.map(i128::from).max().unwrap_or(i128::MIN),
    }
}

fn holds(comparison: Comparison, value: i128, bound: Bound) -> bool {
    let bound = match bound {
        Bound::NegativeInfinity => i128::MIN,
        Bound::Number(number) => i128::from(number),
        Bound::PositiveInfinity => i128::MAX,
    };
    match comparison {
        Comparison::Eq => value == bound,
        Comparison::Ne => value != bound,
        Comparison::Lt => value < bound,
        Comparison::Le => value <= bound,
        Comparison::Gt => value > bound,
        Comparison::Ge => value >= bound,
    }
}

fn push(nodes: &mut Vec<Node>, node: Node) -> usize {
    let index = nodes.len();
    nodes.push(node);
    index
}

fn reference(
    elements: &[Element],
    extremum: Extremum,
    comparison: Comparison,
    bound: Bound,
) -> (Vec<Node>, usize) {
    let mut nodes = prefix();
    let mut root = 1;
    for subset in 0usize..(1 << elements.len()) {
        let selected = elements
            .iter()
            .enumerate()
            .filter(|(index, _)| subset & (1 << index) != 0)
            .map(|(_, element)| element.weight);
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

fn world(theory: &Theory, mask: u8) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap()
}

fn verify(elements: &[Element], extremum: Extremum, comparison: Comparison, bound: Bound) {
    let input = prefix();
    let mut nodes = input.clone();
    let built = append_extremum(
        &mut nodes,
        elements,
        extremum,
        comparison,
        bound,
        AggregateLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let theory = Theory::new(2, nodes, vec![built.root()], AdmissionLimits::default()).unwrap();
    let (reference, root) = reference(elements, extremum, comparison, bound);
    for outer in 0..4 {
        let selected = elements
            .iter()
            .filter(|element| eval(&input, element.condition, outer, None))
            .map(|element| element.weight);
        let classical = holds(comparison, value(extremum, selected), bound);
        let candidate = world(&theory, outer);
        assert_eq!(eval(&reference, root, outer, None), classical);
        assert_eq!(
            models(&theory, &candidate, Limits::default(), &Control::default()).unwrap(),
            classical,
            "classical: {elements:?}, {extremum:?}, {comparison:?}, {bound:?}, M={outer}"
        );
        // Include interpretations outside M as well as actual subset queries.
        for inner in 0..4 {
            let selected = elements
                .iter()
                .filter(|element| eval(&input, element.condition, inner, Some(outer)))
                .map(|element| element.weight);
            let expected = classical && holds(comparison, value(extremum, selected), bound);
            assert_eq!(eval(&reference, root, inner, Some(outer)), expected);
            assert_eq!(
                models_reduct(
                    &theory,
                    &candidate,
                    &world(&theory, inner),
                    Limits::default(),
                    &Control::default()
                )
                .unwrap(),
                expected,
                "reduct: {elements:?}, {extremum:?}, {comparison:?}, {bound:?}, M={outer}, J={inner}"
            );
        }
    }
}

#[test]
fn exhaustive_small_extrema_match_full_subset_implications_and_primitive_reducts() {
    for left in -2..=2 {
        for right in -2..=2 {
            for second_condition in [2, 3, 4, 5, 9, 10] {
                let elements = [
                    Element {
                        weight: left,
                        condition: 2,
                    },
                    Element {
                        weight: right,
                        condition: second_condition,
                    },
                ];
                for extremum in EXTREMA {
                    for comparison in COMPARISONS {
                        for bound in [
                            Bound::NegativeInfinity,
                            Bound::Number(-2),
                            Bound::Number(-1),
                            Bound::Number(0),
                            Bound::Number(1),
                            Bound::Number(2),
                            Bound::PositiveInfinity,
                        ] {
                            verify(&elements, extremum, comparison, bound);
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    #[test]
    fn generated_composite_conditions_preserve_frozen_extrema(
        entries in prop::collection::vec((-3i32..=3, 0usize..11), 0..=5),
        extremum in 0usize..2,
        comparison in 0usize..6,
        bound in prop_oneof![
            Just(Bound::NegativeInfinity),
            (-5i64..=5).prop_map(Bound::Number),
            Just(Bound::PositiveInfinity),
        ],
    ) {
        let elements: Vec<_> = entries.into_iter()
            .map(|(weight, condition)| Element { weight, condition }).collect();
        verify(&elements, EXTREMA[extremum], COMPARISONS[comparison], bound);
    }
}

#[test]
fn empty_tied_and_extreme_values_keep_infinities_distinct_from_finite_limits() {
    for extremum in EXTREMA {
        for comparison in COMPARISONS {
            for bound in [
                Bound::NegativeInfinity,
                Bound::Number(i64::MIN),
                Bound::Number(i64::from(i32::MIN)),
                Bound::Number(0),
                Bound::Number(i64::from(i32::MAX)),
                Bound::Number(i64::MAX),
                Bound::PositiveInfinity,
            ] {
                verify(&[], extremum, comparison, bound);
                for weight in [i32::MIN, 0, i32::MAX] {
                    verify(
                        &[
                            Element {
                                weight,
                                condition: 4,
                            },
                            Element {
                                weight,
                                condition: 3,
                            },
                            Element {
                                weight,
                                condition: 0,
                            },
                        ],
                        extremum,
                        comparison,
                        bound,
                    );
                }
            }
        }
    }
}

#[test]
fn linear_extrema_need_neither_subset_carriers_nor_threshold_rows() {
    for extremum in EXTREMA {
        let mut nodes: Vec<_> = (0..4_096).map(Node::Atom).collect();
        let elements: Vec<_> = (0..4_096)
            .map(|condition| Element {
                weight: if condition % 2 == 0 {
                    i32::MIN
                } else {
                    i32::MAX
                },
                condition,
            })
            .collect();
        let built = append_extremum(
            &mut nodes,
            &elements,
            extremum,
            Comparison::Ne,
            0,
            AggregateLimits {
                max_states: 0,
                max_subsets: 0,
                ..AggregateLimits::default()
            },
            &Control::default(),
        )
        .unwrap();
        assert_eq!(built.profile(), AggregateProfile::Extremum);
        assert_eq!(built.statistics().states, 0);
        assert_eq!(built.statistics().subsets, 0);
        assert!(built.appended_nodes() <= 2 * elements.len() + 4);
        let theory =
            Theory::new(4_096, nodes, vec![built.root()], AdmissionLimits::default()).unwrap();
        let empty = Interpretation::new(&theory, []).unwrap();
        assert!(models(&theory, &empty, Limits::default(), &Control::default()).unwrap());
    }
}

fn limited(elements: &[Element], limits: AggregateLimits) -> Error {
    let original = prefix();
    let mut nodes = original.clone();
    let error = append_extremum(
        &mut nodes,
        elements,
        Extremum::Min,
        Comparison::Eq,
        1,
        limits,
        &Control::default(),
    )
    .unwrap_err();
    assert_eq!(nodes, original);
    error.kind()
}

#[test]
fn inclusive_limits_and_failed_append_restore_every_prefix_node() {
    let elements = [
        Element {
            weight: 0,
            condition: 2,
        },
        Element {
            weight: 1,
            condition: 3,
        },
    ];
    let mut nodes = prefix();
    let built = append_extremum(
        &mut nodes,
        &elements,
        Extremum::Min,
        Comparison::Eq,
        1,
        AggregateLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let exact = AggregateLimits {
        max_elements: elements.len(),
        max_nodes: nodes.len(),
        max_work: built.statistics().work,
        max_states: 0,
        max_subsets: 0,
    };
    let mut repeated = prefix();
    assert_eq!(
        append_extremum(
            &mut repeated,
            &elements,
            Extremum::Min,
            Comparison::Eq,
            1,
            exact,
            &Control::default(),
        )
        .unwrap(),
        built
    );
    assert_eq!(repeated, nodes);
    assert_eq!(
        limited(
            &elements,
            AggregateLimits {
                max_elements: exact.max_elements - 1,
                ..exact
            }
        ),
        Error::ElementLimit
    );
    assert_eq!(
        limited(
            &elements,
            AggregateLimits {
                max_nodes: exact.max_nodes - 1,
                ..exact
            }
        ),
        Error::NodeLimit
    );
    assert_eq!(
        limited(
            &elements,
            AggregateLimits {
                max_work: exact.max_work - 1,
                ..exact
            }
        ),
        Error::WorkLimit
    );
}

#[test]
fn bad_conditions_and_topology_are_typed_failures_with_rollback() {
    assert_eq!(
        limited(
            &[Element {
                weight: 1,
                condition: usize::MAX
            }],
            AggregateLimits::default()
        ),
        Error::InvalidCondition { element: 0 }
    );
    let mut nodes = vec![Node::Implies(0, 0)];
    let original = nodes.clone();
    let error = append_extremum(
        &mut nodes,
        &[],
        Extremum::Max,
        Comparison::Eq,
        Bound::NegativeInfinity,
        AggregateLimits::default(),
        &Control::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), Error::InvalidPrefix { node: 0 });
    assert_eq!(nodes, original);
}

#[test]
fn cancelled_and_expired_empty_extrema_are_refusals_before_work() {
    let cancelled = Control::default();
    cancelled.cancel();
    for (control, expected) in [
        (cancelled, Stop::Cancelled),
        (Control::with_deadline(Instant::now()), Stop::Deadline),
    ] {
        let mut nodes = Vec::new();
        let error = append_extremum(
            &mut nodes,
            &[],
            Extremum::Min,
            Comparison::Eq,
            Bound::PositiveInfinity,
            AggregateLimits {
                max_work: 0,
                ..AggregateLimits::default()
            },
            &control,
        )
        .unwrap_err();
        assert_eq!(error.kind(), Error::Control(expected));
        assert_eq!(error.statistics().work, 0);
        assert!(nodes.is_empty());
    }
}
