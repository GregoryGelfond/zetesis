//! Fixed aggregate values witnessed by required tuples.
//!
//! Full tuple identities coalesce before reduction. An optional occurrence of a
//! required key adds no contribution. Every remaining optional key must leave
//! the required measure unchanged; otherwise this finite profile is unqualified.
//! This proves an invariant value without enumerating choices or answer sets.

use std::collections::BTreeSet;

use themelios_program::program::AggregateFunction;
use themelios_program::term::EvalError;
use zetesis_core::{Predicate, Term, Value};

use super::{Activity, Context, activity, assignment, transport};
use crate::expansion::Budget;
use crate::formula_ir::{AggregateIr, AggregateKey, Prepared, RuleIr};
use crate::formula_support::{Counters, copy};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits};

pub(in super::super) struct Fixed<'a> {
    pub predicates: BTreeSet<&'a Predicate>,
    pub value: Value,
}

pub(in super::super) fn certify<'a>(
    prepared: &'a Prepared,
    rule: &'a RuleIr,
    aggregate: &AggregateIr,
    retained: usize,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<Option<Fixed<'a>>, FormulaFailure> {
    let mut context = Context {
        limits,
        counters,
        location: rule.location,
        entries: retained,
    };
    context.inspect()?;
    let Some(head) = assignment(rule, aggregate) else {
        return Ok(None);
    };
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
    for tuple in possible.difference(&required) {
        context.inspect()?;
        if !unchanged(aggregate.function, first(tuple), &value, &context)? {
            return Ok(None);
        }
    }
    let Some(predicates) = transport(prepared, head.predicate(), &mut context)? else {
        return Ok(None);
    };
    Ok(Some(Fixed { predicates, value }))
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
                if (function == AggregateFunction::Min && value < selected)
                    || (function == AggregateFunction::Max && value > selected)
                {
                    selected = value;
                }
            }
            copy(selected, budget, context.location)
        }
    }
}

fn unchanged(
    function: AggregateFunction,
    first: Option<&Value>,
    required: &Value,
    context: &Context<'_>,
) -> Result<bool, FormulaFailure> {
    Ok(match function {
        AggregateFunction::Count | AggregateFunction::Sum | AggregateFunction::SumPlus => {
            crate::formula_assignment::contribution(function, first, context.location)?
                .is_none_or(|weight| weight == 0)
        }
        AggregateFunction::Min => required <= first.expect("admitted nonempty extremum tuple"),
        AggregateFunction::Max => required >= first.expect("admitted nonempty extremum tuple"),
    })
}

fn first(tuple: &[Term]) -> Option<&Value> {
    tuple.first().map(|term| {
        let Term::Constant(value) = term else {
            unreachable!("certificate checked every tuple component")
        };
        value
    })
}
