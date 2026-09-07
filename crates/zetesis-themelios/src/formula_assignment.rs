//! Bounded candidate values from complete, full-tuple local joins.

use std::collections::BTreeSet;

use themelios_base::span::Location;
use themelios_program::program::AggregateFunction;
use themelios_program::term::EvalError;
use zetesis_core::Value;

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{AggregateIr, AggregateKey, value_bytes};
use crate::formula_support::{Counters, Join, Support};
use crate::{
    ExpansionFailure, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource,
    ProfileFeature,
};

pub(crate) fn values(
    aggregate: &AggregateIr,
    assignment: &[Value],
    support: &Support,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Vec<Value>, FormulaFailure> {
    let mut tuples = BTreeSet::new();
    for element in &aggregate.elements {
        let mut local = Join::new(
            &element.condition,
            assignment,
            element.variables,
            support,
            budget,
            location,
        )?;
        while let Some(binding) = local.next(limits, budget, counters, location)? {
            let AggregateKey::Tuple(terms) = &element.key else {
                unreachable!("assignment is a function aggregate")
            };
            let mut tuple = Vec::new();
            for term in terms {
                counters.work(limits, location)?;
                let value = term.resolve(&binding).expect("safe aggregate key assigned");
                budget.charge(ExpansionResource::ScalarBytes, value_bytes(value), location)?;
                tuple.push(value.clone());
            }
            if !tuples.contains(&tuple) {
                ceiling(
                    FormulaResource::AggregateElements,
                    tuples.len() as u128 + 1,
                    limits.aggregate.max_elements as u128,
                    location,
                )?;
                tuples.insert(tuple);
            }
        }
    }
    if matches!(
        aggregate.function,
        AggregateFunction::Min | AggregateFunction::Max
    ) {
        return extrema_candidates(
            aggregate.function,
            tuples
                .iter()
                .map(|tuple| tuple.first().expect("admitted nonempty extremum tuple")),
            limits,
            budget,
            counters,
            location,
        );
    }
    let mut weights = Vec::new();
    for tuple in tuples {
        if let Some(weight) = tuple_weight(aggregate.function, &tuple, location)? {
            weights.push(weight);
        }
    }
    candidates(aggregate.function, weights, limits, counters, location)
}

/// Preserve the independently recorded endpoint profile while extending term
/// classes. Source endpoint behavior is a separate compatibility obligation.
pub(crate) fn extremum_value(value: &Value, location: Location) -> Result<(), FormulaFailure> {
    if matches!(value, Value::Number(i32::MIN | i32::MAX)) {
        return Err(unsupported(ProfileFeature::Aggregate, location).into());
    }
    Ok(())
}

/// A completed possible tuple carrier covers every actual selected value.
/// Storage order is only the deterministic candidate/cache enumeration order.
pub(crate) fn extrema_candidates<'a>(
    function: AggregateFunction,
    possible: impl IntoIterator<Item = &'a Value>,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Vec<Value>, FormulaFailure> {
    let empty = match function {
        AggregateFunction::Min => Value::Supremum,
        AggregateFunction::Max => Value::Infimum,
        _ => unreachable!("value candidates are extrema only"),
    };
    ceiling(
        FormulaResource::AssignmentValues,
        1,
        limits.max_assignment_values as u128,
        location,
    )?;
    let mut values = BTreeSet::from([empty]);
    for value in possible {
        counters.work(limits, location)?;
        extremum_value(value, location)?;
        if !values.contains(value) {
            ceiling(
                FormulaResource::AssignmentValues,
                values.len() as u128 + 1,
                limits.max_assignment_values as u128,
                location,
            )?;
            values.insert(crate::formula_support::copy(value, budget, location)?);
        }
    }
    Ok(values.into_iter().collect())
}

/// Select the numeric contribution before eligibility lowering. Whole tuples
/// still identify distinct elements; ignored sums never become zero-weight keys.
pub(crate) fn tuple_weight(
    function: AggregateFunction,
    tuple: &[Value],
    location: Location,
) -> Result<Option<i32>, FormulaFailure> {
    match (function, tuple.first()) {
        (AggregateFunction::Count, _) => Ok(Some(1)),
        (AggregateFunction::SumPlus, Some(Value::Number(weight))) if *weight > 0 => {
            Ok(Some(*weight))
        }
        (AggregateFunction::SumPlus, _) => Ok(None),
        (_, Some(Value::Number(weight))) => Ok(Some(*weight)),
        (AggregateFunction::Sum, _) => Ok(None),
        _ => Err(unsupported(ProfileFeature::Aggregate, location).into()),
    }
}

/// Actual enabled tuples are a subset of the complete possible-key set.
/// Correlated conditions do not justify deleting candidate values here.
pub(crate) fn sums(
    weights: impl IntoIterator<Item = i32>,
    limits: FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<Vec<i32>, FormulaFailure> {
    let mut sums = BTreeSet::new();
    ceiling(
        FormulaResource::AssignmentValues,
        1,
        limits.max_assignment_values as u128,
        location,
    )?;
    sums.insert(0_i32);
    for weight in weights {
        let mut delta = Vec::new();
        for sum in &sums {
            counters.work(limits, location)?;
            let value = sum
                .checked_add(weight)
                .ok_or(ExpansionFailure::Evaluation {
                    error: EvalError::Overflow,
                    location,
                })?;
            if !sums.contains(&value) {
                ceiling(
                    FormulaResource::AssignmentValues,
                    sums.len() as u128 + delta.len() as u128 + 1,
                    limits.max_assignment_values as u128,
                    location,
                )?;
                delta.push(value);
            }
        }
        sums.extend(delta);
    }
    Ok(sums.into_iter().collect())
}

/// Extrema range over eligible numeric values plus their genuine empty sentinel.
/// Count/sum retain the complete finite subset-value upper approximation.
pub(crate) fn candidates(
    function: AggregateFunction,
    weights: impl IntoIterator<Item = i32>,
    limits: FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<Vec<Value>, FormulaFailure> {
    let empty = match function {
        AggregateFunction::Count | AggregateFunction::Sum | AggregateFunction::SumPlus => {
            let weights = weights
                .into_iter()
                .filter(|weight| function != AggregateFunction::SumPlus || *weight > 0);
            return sums(weights, limits, counters, location)
                .map(|values| values.into_iter().map(Value::Number).collect());
        }
        AggregateFunction::Min => Value::Supremum,
        AggregateFunction::Max => Value::Infimum,
    };
    ceiling(
        FormulaResource::AssignmentValues,
        1,
        limits.max_assignment_values as u128,
        location,
    )?;
    let mut values = BTreeSet::from([empty]);
    for weight in weights {
        counters.work(limits, location)?;
        let value = Value::Number(weight);
        if !values.contains(&value) {
            ceiling(
                FormulaResource::AssignmentValues,
                values.len() as u128 + 1,
                limits.max_assignment_values as u128,
                location,
            )?;
            values.insert(value);
        }
    }
    Ok(values.into_iter().collect())
}
