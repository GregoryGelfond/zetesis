//! The required/possible carrier: the measure of every selection between an
//! assignment's required tuples and its possible ones.

use super::{Contributions, Numeric, Reduced, Sides, carrier};
use crate::formula_binding::Binding;
use crate::formula_support::Computation;
use crate::formula_support::testing::Fixture;
use crate::test_support::given_site;
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits};
use themelios_program::term::EvalError;
use zetesis_core::ValueNodeRef;
use zetesis_ferraris::AggregateExtremum;

/// The carrier of `function` from the `required` and `optional` contributions,
/// as numbers, or its refusal.
fn numbers(
    function: Numeric,
    required: &[i32],
    optional: &[i32],
) -> Result<Vec<i32>, FormulaFailure> {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let contributions = Contributions::Numeric {
            function,
            sides: Sides {
                required: required.iter().copied(),
                optional: optional.iter().copied(),
            },
        };
        let values = carrier(
            contributions,
            computation,
            &FormulaLimits::default(),
            counters,
            given_site(),
        )?;
        Ok(read(&values, computation))
    })
}

fn read(values: &Binding, computation: &Computation<'_, '_>) -> Vec<i32> {
    (0..values.len())
        .map(|slot| {
            let key = values.slots().key(slot).unwrap().unwrap();
            match computation.read().term(&key).unwrap().descriptor() {
                ValueNodeRef::Number(number) => number,
                other => panic!("a numeric proposal, not {other:?}"),
            }
        })
        .collect()
}

/// The `#min` or `#max` carrier of numeric values, the empty value spelled
/// `None`.
fn extrema(function: AggregateExtremum, required: &[i32], optional: &[i32]) -> Vec<Option<i32>> {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut keys = |numbers: &[i32]| {
            let mut binding = Binding::new(computation, &limits, counters, given_site()).unwrap();
            binding
                .extend_scope(numbers.len(), computation, &limits, counters, given_site())
                .unwrap();
            for (slot, &number) in numbers.iter().enumerate() {
                let key = computation
                    .number(number, &limits, counters, given_site())
                    .unwrap();
                binding
                    .set(slot, &key, &limits, counters, given_site())
                    .unwrap();
            }
            binding
        };
        let contributions = Reduced::Extremum {
            function,
            sides: Sides {
                required: keys(required),
                optional: keys(optional),
            },
        };
        let values = carrier(
            contributions.streams(),
            computation,
            &limits,
            counters,
            given_site(),
        )
        .unwrap();
        (0..values.len())
            .map(|slot| {
                let key = values.slots().key(slot).unwrap().unwrap();
                match computation.read().term(&key).unwrap().descriptor() {
                    ValueNodeRef::Number(number) => Some(number),
                    _ => None,
                }
            })
            .collect()
    })
}

fn overflowed(result: &Result<Vec<i32>, FormulaFailure>) -> bool {
    matches!(
        result,
        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
            error: EvalError::Overflow,
            ..
        }))
    )
}

#[test]
fn counts_start_at_the_required_tuples() {
    assert_eq!(numbers(Numeric::Count, &[1, 1], &[1]).unwrap(), [2, 3]);
}

#[test]
fn sums_add_every_optional_selection_to_the_required_total() {
    assert_eq!(
        numbers(Numeric::Sum, &[2, -1], &[3, 4]).unwrap(),
        [1, 4, 5, 8]
    );
}

#[test]
fn positive_sums_ignore_nonpositive_required_weights() {
    assert_eq!(
        numbers(Numeric::SumPlus, &[-5, 2], &[3, 0]).unwrap(),
        [2, 5]
    );
}

#[test]
fn no_required_tuple_gives_every_optional_selection() {
    assert_eq!(numbers(Numeric::Count, &[], &[1, 1]).unwrap(), [0, 1, 2]);
    assert_eq!(numbers(Numeric::Sum, &[], &[2, 3]).unwrap(), [0, 2, 3, 5]);
}

#[test]
fn the_required_total_does_not_depend_on_summation_order() {
    // A partial sum leaves i32 in two of these orders; the total fits.
    for order in [[i32::MAX, 1, -10], [1, i32::MAX, -10], [-10, 1, i32::MAX]] {
        assert_eq!(numbers(Numeric::Sum, &order, &[]).unwrap(), [i32::MAX - 9]);
    }
}

#[test]
fn an_optional_only_overflow_is_no_refusal_when_every_carrier_value_fits() {
    // {-10} plus every selection of {2147483647, 1}; the optional pair alone
    // exceeds i32, but no carrier value does.
    assert_eq!(
        numbers(Numeric::Sum, &[-10], &[i32::MAX, 1]).unwrap(),
        [-10, -9, i32::MAX - 10, i32::MAX - 9]
    );
}

#[test]
fn a_carrier_value_outside_i32_refuses_the_sum() {
    assert!(overflowed(&numbers(Numeric::Sum, &[i32::MAX], &[1])));
}

#[test]
fn a_required_total_outside_i32_refuses_the_sum() {
    assert!(overflowed(&numbers(Numeric::Sum, &[i32::MAX, 1], &[])));
}

#[test]
fn a_seeded_sum_of_many_units_matches_the_unseeded_one_shifted() {
    // Values only: the carrier of a required -7 is the unseeded carrier moved
    // by -7. The packed route from a nonzero origin is asserted by the packed
    // tests' `seeded_packed` cases.
    let optional = [1; 64];
    let seeded = numbers(Numeric::Sum, &[-7], &optional).unwrap();
    let shifted: Vec<_> = numbers(Numeric::Sum, &[], &optional)
        .unwrap()
        .into_iter()
        .map(|sum| sum - 7)
        .collect();
    assert_eq!(seeded, shifted);
}

#[test]
fn a_required_minimum_keeps_only_values_below_it() {
    assert_eq!(
        extrema(AggregateExtremum::Min, &[5, 3], &[1, 4, 7]),
        [Some(1), Some(3)]
    );
}

#[test]
fn a_required_maximum_keeps_only_values_above_it() {
    assert_eq!(
        extrema(AggregateExtremum::Max, &[5, 3], &[1, 4, 7]),
        [Some(5), Some(7)]
    );
}

#[test]
fn the_extremum_carrier_leaves_the_endpoint_guard_to_its_callers() {
    // The proposal sites refuse an i32 endpoint where they reduce a tuple to
    // its value; the operation itself proposes whatever values it is given.
    assert_eq!(
        extrema(AggregateExtremum::Max, &[i32::MAX], &[i32::MIN]),
        [Some(i32::MAX)]
    );
}

#[test]
fn a_minimum_without_a_required_tuple_keeps_its_empty_value() {
    // The empty value of #min is #sup, the last term in term order.
    assert_eq!(
        extrema(AggregateExtremum::Min, &[], &[1, 4]),
        [Some(1), Some(4), None]
    );
}

#[test]
fn a_maximum_without_a_required_tuple_keeps_its_empty_value() {
    // The empty value of #max is #inf, the first term in term order.
    assert_eq!(
        extrema(AggregateExtremum::Max, &[], &[1, 4]),
        [None, Some(1), Some(4)]
    );
}
