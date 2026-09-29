//! The CPU reference a device's aggregates are compared with: acquired
//! original and frozen truth over every connective of a two-atom theory.

use zetesis_core::Value;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, Interpretation, Node, Theory,
    native_aggregate::{self as native, Bound, Eligibility, Function, Group, Guard, Tuple},
};

/// Every aggregate function.
pub const FUNCTIONS: [Function; 5] = [
    Function::Count,
    Function::Sum,
    Function::SumPlus,
    Function::Min,
    Function::Max,
];

/// Every aggregate comparison.
pub const COMPARISONS: [Comparison; 6] = [
    Comparison::Eq,
    Comparison::Ne,
    Comparison::Lt,
    Comparison::Le,
    Comparison::Gt,
    Comparison::Ge,
];

/// A theory over atoms `a` and `b` without roots, whose nodes are every
/// connective the conditions use: falsity and truth, `a` and `b`, `not a`,
/// `not not a` and `not b`, and `a` with `b` under conjunction, disjunction
/// and implication.
///
/// # Panics
/// Panics if the default admission limits refuse the theory; they admit it.
#[must_use]
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

/// The group of `function` over one tuple per entry of `first`, under
/// `guards`. A tuple's key is its entry with its position, or empty for
/// `None`, and its condition is the next of [`theory`]'s eight nodes past the
/// constants, in turn.
///
/// # Panics
/// Panics if the group is refused, as it is over a theory other than
/// [`theory`].
#[must_use]
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

/// The group of `function` over `count` tuples whose values run through
/// -3, 2, 0, 7 and -1 in turn, guarded by `≥ 0`.
///
/// # Panics
/// As [`operation`].
#[must_use]
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

/// The four interpretations of [`theory`]'s two atoms.
///
/// # Panics
/// Panics if `theory` has fewer than two atoms.
#[must_use]
pub fn worlds(theory: &Theory) -> Vec<Interpretation> {
    (0..4)
        .map(|bits| {
            Interpretation::new(theory, (0..2).filter(|atom| bits & (1 << atom) != 0)).unwrap()
        })
        .collect()
}

/// `count` eligibilities of `group`, five over each original world of
/// `worlds` in turn: the first four freeze each world in turn, and the fifth
/// is acquired without a frozen world.
///
/// # Panics
/// Panics if `worlds` holds fewer than four interpretations or an
/// eligibility is refused.
#[must_use]
pub fn observations<'a>(
    group: impl Into<native::GroupRef<'a>>,
    worlds: &'a [Interpretation],
    count: usize,
) -> Vec<Eligibility<'a>> {
    let group = group.into();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_acquires_one_eligibility_per_observation() {
        let theory = theory();
        let worlds = worlds(&theory);
        assert_eq!(worlds.len(), 4);
        let group = group(&theory, Function::Sum, 5);
        assert_eq!(observations(&group, &worlds, 7).len(), 7);
    }
}
