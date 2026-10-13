//! The required/possible carrier: the measure of every selection between an
//! assignment's required tuples and its possible ones.

use super::{carrier, extrema_carrier};
use crate::formula_binding::Binding;
use crate::formula_support::Computation;
use crate::formula_support::testing::Fixture;
use crate::test_support::given_site;
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits};
use themelios_program::program::AggregateFunction;
use themelios_program::term::EvalError;
use zetesis_core::ValueNodeRef;

/// The carrier of `function` from the `required` and `optional` contributions,
/// as numbers, or its refusal.
fn numbers(
    function: AggregateFunction,
    required: &[i32],
    optional: &[i32],
) -> Result<Vec<i32>, FormulaFailure> {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let values = carrier(
            function,
            required.iter().copied(),
            optional.iter().copied(),
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
fn extrema(function: AggregateFunction, required: &[i32], optional: &[i32]) -> Vec<Option<i32>> {
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
        let (required, optional) = (keys(required), keys(optional));
        let values = extrema_carrier(
            function,
            required.slots(),
            optional.slots(),
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
    assert_eq!(
        numbers(AggregateFunction::Count, &[1, 1], &[1]).unwrap(),
        [2, 3]
    );
}

#[test]
fn sums_add_every_optional_selection_to_the_required_total() {
    assert_eq!(
        numbers(AggregateFunction::Sum, &[2, -1], &[3, 4]).unwrap(),
        [1, 4, 5, 8]
    );
}

#[test]
fn positive_sums_ignore_nonpositive_required_weights() {
    assert_eq!(
        numbers(AggregateFunction::SumPlus, &[-5, 2], &[3, 0]).unwrap(),
        [2, 5]
    );
}

#[test]
fn no_required_tuple_gives_every_optional_selection() {
    assert_eq!(
        numbers(AggregateFunction::Count, &[], &[1, 1]).unwrap(),
        [0, 1, 2]
    );
    assert_eq!(
        numbers(AggregateFunction::Sum, &[], &[2, 3]).unwrap(),
        [0, 2, 3, 5]
    );
}

#[test]
fn the_required_total_does_not_depend_on_summation_order() {
    // A partial sum leaves i32 in two of these orders; the total fits.
    for order in [[i32::MAX, 1, -10], [1, i32::MAX, -10], [-10, 1, i32::MAX]] {
        assert_eq!(
            numbers(AggregateFunction::Sum, &order, &[]).unwrap(),
            [i32::MAX - 9]
        );
    }
}

#[test]
fn an_optional_only_overflow_is_no_refusal_when_every_carrier_value_fits() {
    // {-10} plus every selection of {2147483647, 1}; the optional pair alone
    // exceeds i32, but no carrier value does.
    assert_eq!(
        numbers(AggregateFunction::Sum, &[-10], &[i32::MAX, 1]).unwrap(),
        [-10, -9, i32::MAX - 10, i32::MAX - 9]
    );
}

#[test]
fn a_carrier_value_outside_i32_refuses_the_sum() {
    assert!(overflowed(&numbers(
        AggregateFunction::Sum,
        &[i32::MAX],
        &[1]
    )));
}

#[test]
fn a_required_total_outside_i32_refuses_the_sum() {
    assert!(overflowed(&numbers(
        AggregateFunction::Sum,
        &[i32::MAX, 1],
        &[]
    )));
}

#[test]
fn a_seeded_sum_of_many_units_matches_the_unseeded_one_shifted() {
    // Values only: the carrier of a required -7 is the unseeded carrier moved
    // by -7. The packed route from a nonzero origin is asserted by the packed
    // tests' `seeded_packed` cases.
    let optional = [1; 64];
    let seeded = numbers(AggregateFunction::Sum, &[-7], &optional).unwrap();
    let shifted: Vec<_> = numbers(AggregateFunction::Sum, &[], &optional)
        .unwrap()
        .into_iter()
        .map(|sum| sum - 7)
        .collect();
    assert_eq!(seeded, shifted);
}

#[test]
fn a_required_minimum_keeps_only_values_below_it() {
    assert_eq!(
        extrema(AggregateFunction::Min, &[5, 3], &[1, 4, 7]),
        [Some(1), Some(3)]
    );
}

#[test]
fn a_required_maximum_keeps_only_values_above_it() {
    assert_eq!(
        extrema(AggregateFunction::Max, &[5, 3], &[1, 4, 7]),
        [Some(5), Some(7)]
    );
}

#[test]
fn the_extremum_carrier_leaves_the_endpoint_guard_to_its_callers() {
    // The proposal sites refuse an i32 endpoint where they reduce a tuple to
    // its value; the operation itself proposes whatever values it is given.
    assert_eq!(
        extrema(AggregateFunction::Max, &[i32::MAX], &[i32::MIN]),
        [Some(i32::MAX)]
    );
}

#[test]
fn an_extremum_without_a_required_tuple_keeps_the_empty_value() {
    // The empty value of #min is #sup, the last term in term order.
    assert_eq!(
        extrema(AggregateFunction::Min, &[], &[1, 4]),
        [Some(1), Some(4), None]
    );
}
