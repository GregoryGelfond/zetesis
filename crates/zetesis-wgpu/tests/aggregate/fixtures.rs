//! Acquired original/frozen truth over every connective in a small prefix.

use zetesis_core::Value;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, Interpretation, Node, Theory,
    native_aggregate::{self as native, Bound, Eligibility, Function, Group, Guard, Tuple},
};

pub const FUNCTIONS: [Function; 5] = [
    Function::Count,
    Function::Sum,
    Function::SumPlus,
    Function::Min,
    Function::Max,
];
pub const COMPARISONS: [Comparison; 6] = [
    Comparison::Eq,
    Comparison::Ne,
    Comparison::Lt,
    Comparison::Le,
    Comparison::Gt,
    Comparison::Ge,
];

pub fn theory() -> Theory {
    Theory::new(
        2,
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
        ],
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}

pub fn operation(
    theory: &Theory,
    function: Function,
    first: Vec<Option<Value>>,
    guards: Vec<Guard>,
) -> Group {
    let tuples = first
        .into_iter()
        .enumerate()
        .map(|(index, value)| Tuple {
            key: value.map_or_else(Vec::new, |value| {
                vec![value, Value::Number(i32::try_from(index).unwrap())]
            }),
            condition: [2, 3, 4, 5, 6, 7, 8, 9][index % 8],
        })
        .collect::<Vec<_>>();
    let limits = native::AdmissionLimits {
        max_guards: guards.len(),
        ..native::AdmissionLimits::default()
    };
    Group::new(
        theory,
        function,
        tuples,
        guards,
        limits,
        &Cancellation::default(),
    )
    .unwrap()
}

pub fn group(theory: &Theory, function: Function, count: usize) -> Group {
    operation(
        theory,
        function,
        (0..count)
            .map(|index| Some(Value::Number([-3, 2, 0, 7, -1][index % 5])))
            .collect(),
        vec![Guard {
            comparison: Comparison::Ge,
            bound: Bound::Integer(0),
        }],
    )
}

pub fn worlds(theory: &Theory) -> Vec<Interpretation> {
    (0..4)
        .map(|bits| {
            Interpretation::new(theory, (0..2).filter(|atom| bits & (1 << atom) != 0)).unwrap()
        })
        .collect()
}

pub fn observations<'a>(
    group: &'a Group,
    worlds: &'a [Interpretation],
    count: usize,
) -> Vec<Eligibility<'a>> {
    (0..count)
        .map(|index| {
            group
                .eligibility(
                    &worlds[(index / 5) % 4],
                    (index % 5 != 4).then(|| &worlds[index % 5]),
                    native::EligibilityLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
        })
        .collect()
}
