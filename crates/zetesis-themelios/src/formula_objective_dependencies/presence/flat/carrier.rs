//! Completed source measure carriers between required and possible tuples.
//!
//! Full tuple identities coalesce before reduction. An optional occurrence of a
//! required key adds no contribution. The carrier contains the measure of every
//! key set R ⊆ S ⊆ P. Shared conditions do not erase source values; original
//! aggregate equalities determine which values occur in an answer set. Numeric
//! subset construction can take exponential work and space, bounded by the
//! assignment-value and work ceilings. Completed values also consume presence
//! entries and scalar payload; transient sums have their separate value ceiling.

use std::collections::BTreeSet;

use themelios_program::program::AggregateFunction;
use themelios_program::term::EvalError;
use zetesis_core::{Predicate, Term, Value};

use super::{Activity, Context, activity, assignment, transport};
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{AggregateKey, LiteralIr, Prepared, RuleIr};
use crate::formula_support::{Counters, copy};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};

pub(in super::super) struct Carrier<'a> {
    pub predicates: BTreeSet<&'a Predicate>,
    pub values: BTreeSet<Value>,
}

pub(in super::super) fn certify<'a>(
    prepared: &'a Prepared,
    rule: &'a RuleIr,
    requested: &BTreeSet<&Predicate>,
    retained: usize,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<Option<Carrier<'a>>, FormulaFailure> {
    let mut context = Context {
        limits,
        counters,
        location: rule.location,
        entries: retained,
    };
    context.inspect()?;
    let [LiteralIr::Aggregate(aggregate)] = rule.body.as_slice() else {
        return Ok(None);
    };
    let Some(head) = assignment(rule, aggregate) else {
        return Ok(None);
    };
    let Some(mut predicates) = transport(prepared, head.predicate(), &mut context)? else {
        return Ok(None);
    };
    for _ in &predicates {
        context.inspect()?;
    }
    predicates.retain(|predicate| requested.contains(predicate));
    if predicates.is_empty() {
        return Ok(None);
    }
    let mut required = BTreeSet::new();
    let mut possible = BTreeSet::new();
    for element in &aggregate.elements {
        context.inspect()?;
        let AggregateKey::Tuple(tuple) = &element.key else {
            return Ok(None);
        };
        for term in tuple {
            context.inspect()?;
            if !matches!(term, Term::Constant(_)) {
                return Ok(None);
            }
        }
        let Some(activity) = activity(prepared, &element.condition, &mut context)? else {
            return Ok(None);
        };
        if activity != Activity::Absent {
            context.retain(&mut possible, tuple.as_slice())?;
        }
        if activity == Activity::Required {
            context.retain(&mut required, tuple.as_slice())?;
        }
    }
    let value = measure(aggregate.function, &required, budget, &mut context)?;
    let values = complete(
        aggregate.function,
        &value,
        possible.difference(&required).copied(),
        budget,
        &mut context,
    )?;
    Ok(Some(Carrier { predicates, values }))
}

/// Optional keys are independent only in the source abstraction. Original
/// eligibility remains responsible for their correlation in an interpretation.
fn complete<'a>(
    function: AggregateFunction,
    required: &Value,
    optional: impl Iterator<Item = &'a [Term]>,
    budget: &mut Budget,
    context: &mut Context<'_>,
) -> Result<BTreeSet<Value>, FormulaFailure> {
    let mut values = BTreeSet::new();
    match function {
        AggregateFunction::Count | AggregateFunction::Sum | AggregateFunction::SumPlus => {
            let Value::Number(initial) = required else {
                unreachable!("numeric aggregate reduction")
            };
            let mut weights = Vec::new();
            for tuple in optional {
                context.inspect()?;
                if let Some(weight) = crate::formula_assignment::contribution(
                    function,
                    first(tuple),
                    context.location,
                )? {
                    context.reserve_entry()?;
                    weights
                        .try_reserve(1)
                        .map_err(|_| FormulaFailure::Objective {
                            error: zetesis_objective::AdmissionError::Allocation,
                            location: context.location,
                        })?;
                    weights.push(weight);
                }
            }
            for delta in crate::formula_assignment::sums(
                weights,
                context.limits,
                context.counters,
                context.location,
            )? {
                let total = initial
                    .checked_add(delta)
                    .ok_or(ExpansionFailure::Evaluation {
                        error: EvalError::Overflow,
                        location: context.location,
                    })?;
                retain_value(&mut values, &Value::Number(total), budget, context)?;
            }
        }
        AggregateFunction::Min | AggregateFunction::Max => {
            retain_value(&mut values, required, budget, context)?;
            for tuple in optional {
                context.inspect()?;
                let optional = first(tuple).expect("admitted nonempty extremum tuple");
                let order = optional.compare_terms(required);
                if (function == AggregateFunction::Min && order.is_lt())
                    || (function == AggregateFunction::Max && order.is_gt())
                {
                    retain_value(&mut values, optional, budget, context)?;
                }
            }
        }
    }
    Ok(values)
}

fn retain_value(
    values: &mut BTreeSet<Value>,
    value: &Value,
    budget: &mut Budget,
    context: &mut Context<'_>,
) -> Result<(), FormulaFailure> {
    context.inspect()?;
    if values.contains(value) {
        return Ok(());
    }
    ceiling(
        FormulaResource::AssignmentValues,
        values.len() as u128 + 1,
        context.limits.max_assignment_values as u128,
        context.location,
    )?;
    context.reserve_entry()?;
    let value = copy(value, budget, context.location)?;
    values.insert(value);
    Ok(())
}

fn measure(
    function: AggregateFunction,
    required: &BTreeSet<&[Term]>,
    budget: &mut Budget,
    context: &mut Context<'_>,
) -> Result<Value, FormulaFailure> {
    match function {
        AggregateFunction::Count | AggregateFunction::Sum | AggregateFunction::SumPlus => {
            let mut total = 0_i32;
            for tuple in required {
                context.inspect()?;
                if let Some(weight) = crate::formula_assignment::contribution(
                    function,
                    first(tuple),
                    context.location,
                )? {
                    total = total
                        .checked_add(weight)
                        .ok_or(ExpansionFailure::Evaluation {
                            error: EvalError::Overflow,
                            location: context.location,
                        })?;
                }
            }
            Ok(Value::Number(total))
        }
        AggregateFunction::Min | AggregateFunction::Max => {
            let empty = if function == AggregateFunction::Min {
                Value::Supremum
            } else {
                Value::Infimum
            };
            let mut selected = &empty;
            for tuple in required {
                context.inspect()?;
                let value = first(tuple).expect("admitted nonempty extremum tuple");
                let order = value.compare_terms(selected);
                if (function == AggregateFunction::Min && order.is_lt())
                    || (function == AggregateFunction::Max && order.is_gt())
                {
                    selected = value;
                }
            }
            copy(selected, budget, context.location)
        }
    }
}

fn first(tuple: &[Term]) -> Option<&Value> {
    tuple.first().map(|term| {
        let Term::Constant(value) = term else {
            unreachable!("certificate checked every tuple component")
        };
        value
    })
}
