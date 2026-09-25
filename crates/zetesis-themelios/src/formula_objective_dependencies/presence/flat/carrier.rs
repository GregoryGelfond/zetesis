//! Completed source measure carriers between required and possible tuples.
//!
//! Full tuple identities coalesce before reduction. An optional occurrence of a
//! required key adds no contribution. The carrier contains the measure of every
//! key set R ⊆ S ⊆ P. Shared conditions do not erase source values; original
//! aggregate equalities determine which values occur in an answer set. Numeric
//! subset construction can take exponential work and space, bounded by the
//! assignment-value, shared storage and work ceilings. Completed carriers select
//! canonical identities; transient numeric sums retain only scalar metadata.

use std::cmp::Ordering;

use super::super::Predicates;
use crate::formula_support::components::Term;
use themelios_program::program::AggregateFunction;
use themelios_program::term::EvalError;
use zetesis_core::catalog::{AssignmentError, TermKey, TermRef};
use zetesis_core::{TemplateTerm, Value, ValueNodeRef};

use super::{Activity, Context, activity, assignment, transport};
use crate::formula::ceiling;
use crate::formula_ir::{AggregateKey, LiteralIr, Prepared, RuleIr};
use crate::formula_support::{Buffer, Computation, Counters, TermSelection};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};

pub(in super::super) struct Carrier<'a> {
    pub predicates: Predicates<'a>,
    pub values: TermSelection,
}

pub(in super::super) fn certify<'source>(
    prepared: &Prepared,
    rule: &RuleIr,
    requested: &Predicates<'source>,
    retained: usize,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<Carrier<'source>>, FormulaFailure> {
    let mut context = Context {
        computation,
        limits,
        counters,
        location: rule.location,
        entries: retained,
    };
    context.inspect()?;
    let [LiteralIr::Aggregate(aggregate)] = rule.body.as_slice() else {
        return Ok(None);
    };
    let Some(head) = assignment(rule, aggregate, &mut context)? else {
        return Ok(None);
    };
    context.inspect()?;
    let Some(mut predicates) = transport(prepared, head.predicate(), &mut context)? else {
        return Ok(None);
    };
    let mut kept = 0;
    for at in 0..predicates.0.len() {
        context.inspect()?;
        let predicate = predicates.0[at];
        if requested.contains(predicate, limits, context.counters, context.location)? {
            context.inspect()?;
            predicates.0[kept] = predicate;
            kept += 1;
        }
    }
    predicates.0.truncate(kept);
    if predicates.is_empty() {
        return Ok(None);
    }
    let mut required = Tuples::new(&mut context)?;
    let mut possible = Tuples::new(&mut context)?;
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
            possible.insert(tuple, &mut context)?;
        }
        if activity == Activity::Required {
            required.insert(tuple, &mut context)?;
        }
    }
    let value = measure(aggregate.function, &required, &mut context)?;
    let mut optional = Buffer::new(
        context.computation,
        context.limits,
        context.counters,
        context.location,
    )?;
    for tuple in possible.values.iter().copied() {
        if !required.contains(tuple, &mut context)? {
            optional.push(
                tuple,
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?;
        }
    }
    let values = complete(
        aggregate.function,
        &value,
        optional.iter().copied(),
        &mut context,
    )?;
    Ok(Some(Carrier { predicates, values }))
}

/// Optional keys are independent only in the source abstraction. Original
/// eligibility remains responsible for their correlation in an interpretation.
fn complete<'a>(
    function: AggregateFunction,
    required: &TermKey,
    optional: impl Iterator<Item = &'a [Term]>,
    context: &mut Context<'_, '_, '_>,
) -> Result<TermSelection, FormulaFailure> {
    let mut values = TermSelection::new(
        context.computation,
        context.limits,
        context.counters,
        context.location,
    )?;
    match function {
        AggregateFunction::Count | AggregateFunction::Sum | AggregateFunction::SumPlus => {
            context.inspect()?;
            let read = context.computation.read();
            let required_value = read.term(required).map_err(|error| {
                crate::formula_binding::assignment(AssignmentError::Read(error), context.location)
            })?;
            let ValueNodeRef::Number(initial) = required_value.descriptor() else {
                unreachable!("numeric aggregate reduction")
            };
            let mut weights = Buffer::new(
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?;
            for tuple in optional {
                context.inspect()?;
                if let Some(weight) = crate::formula_assignment::contribution(
                    function,
                    first(tuple, context)?,
                    context.location,
                )? {
                    context.reserve_entry()?;
                    weights.push(
                        weight,
                        context.computation,
                        context.limits,
                        context.counters,
                        context.location,
                    )?;
                }
            }
            let sums = crate::formula_assignment::sums(
                weights.iter().copied(),
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?;
            for &delta in sums.iter() {
                context.inspect()?;
                let total = initial
                    .checked_add(delta)
                    .ok_or(ExpansionFailure::Evaluation {
                        error: EvalError::Overflow,
                        location: context.location,
                    })?;
                let value = context.computation.number(
                    total,
                    context.limits,
                    context.counters,
                    context.location,
                )?;
                retain_value(&mut values, &value, context)?;
            }
        }
        AggregateFunction::Min | AggregateFunction::Max => {
            retain_value(&mut values, required, context)?;
            for tuple in optional {
                context.inspect()?;
                let optional = first(tuple, context)?.expect("admitted nonempty extremum tuple");
                let order = {
                    let read = context.computation.read();
                    let required = read.term(required).map_err(|error| {
                        crate::formula_binding::assignment(
                            AssignmentError::Read(error),
                            context.location,
                        )
                    })?;
                    optional.compare_terms_with(required, || {
                        context.counters.work(context.limits, context.location)
                    })?
                };
                if (function == AggregateFunction::Min && order.is_lt())
                    || (function == AggregateFunction::Max && order.is_gt())
                {
                    context.inspect()?;
                    let value = context
                        .computation
                        .read()
                        .term_key(optional)
                        .map_err(|error| {
                            crate::formula_binding::assignment(error.into(), context.location)
                        })?;
                    retain_value(&mut values, &value, context)?;
                }
            }
        }
    }
    Ok(values)
}

fn retain_value(
    values: &mut TermSelection,
    value: &TermKey,
    context: &mut Context<'_, '_, '_>,
) -> Result<(), FormulaFailure> {
    context.inspect()?;
    if values.contains(value, context.limits, context.counters, context.location)? {
        return Ok(());
    }
    ceiling(
        FormulaResource::AssignmentValues,
        values.len() as u128 + 1,
        context.limits.max_assignment_values as u128,
        context.location,
    )?;
    context.reserve_entry()?;
    values.insert(
        value,
        FormulaResource::AssignmentValues,
        context.limits.max_assignment_values,
        crate::formula_support::Context::new(
            &*context.computation,
            context.limits,
            context.counters,
            context.location,
        ),
    )
}

fn measure(
    function: AggregateFunction,
    required: &Tuples<'_>,
    context: &mut Context<'_, '_, '_>,
) -> Result<TermKey, FormulaFailure> {
    match function {
        AggregateFunction::Count | AggregateFunction::Sum | AggregateFunction::SumPlus => {
            let mut total = 0_i32;
            for tuple in required.values.iter().copied() {
                context.inspect()?;
                if let Some(weight) = crate::formula_assignment::contribution(
                    function,
                    first(tuple, context)?,
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
            context
                .computation
                .number(total, context.limits, context.counters, context.location)
        }
        AggregateFunction::Min | AggregateFunction::Max => {
            let mut selected = None;
            for tuple in required.values.iter().copied() {
                context.inspect()?;
                let value = first(tuple, context)?.expect("admitted nonempty extremum tuple");
                if let Some(previous) = selected {
                    let order = value.compare_terms_with(previous, || context.inspect())?;
                    if (function == AggregateFunction::Min && order.is_lt())
                        || (function == AggregateFunction::Max && order.is_gt())
                    {
                        selected = Some(value);
                    }
                } else {
                    selected = Some(value);
                }
            }
            if let Some(selected) = selected {
                context.inspect()?;
                context
                    .computation
                    .read()
                    .term_key(selected)
                    .map_err(|error| {
                        crate::formula_binding::assignment(error.into(), context.location)
                    })
            } else {
                let empty = if function == AggregateFunction::Min {
                    Value::Supremum
                } else {
                    Value::Infimum
                };
                context.computation.import(
                    (&empty).into(),
                    context.limits,
                    context.counters,
                    context.location,
                )
            }
        }
    }
}

fn first<'source>(
    tuple: &[Term],
    context: &mut Context<'_, '_, 'source>,
) -> Result<Option<TermRef<'source>>, FormulaFailure> {
    tuple
        .first()
        .map(|term| {
            let TemplateTerm::Constant(value) = context.term(*term)? else {
                unreachable!("certificate checked every tuple component");
            };
            Ok(value)
        })
        .transpose()
}

/// Sorted borrowed tuple topology. Constant occurrence coordinates are never
/// semantic keys: equal separately compiled constants must coalesce here.
struct Tuples<'ir> {
    values: Buffer<&'ir [Term]>,
}
impl<'ir> Tuples<'ir> {
    fn new(context: &mut Context<'_, '_, '_>) -> Result<Self, FormulaFailure> {
        Ok(Self {
            values: Buffer::new(
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?,
        })
    }
    fn search(
        &self,
        tuple: &[Term],
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Result<usize, usize>, FormulaFailure> {
        let mut start = 0;
        let mut end = self.values.len();
        while start < end {
            context.inspect()?;
            let middle = start + (end - start) / 2;
            match tuple_compare(self.values.slice()[middle], tuple, context)? {
                Ordering::Less => start = middle + 1,
                Ordering::Greater => end = middle,
                Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(start))
    }
    fn contains(
        &self,
        tuple: &[Term],
        context: &mut Context<'_, '_, '_>,
    ) -> Result<bool, FormulaFailure> {
        self.search(tuple, context).map(|found| found.is_ok())
    }
    fn insert(
        &mut self,
        tuple: &'ir [Term],
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let Err(at) = self.search(tuple, context)? else {
            return Ok(());
        };
        context.reserve_entry()?;
        context.counters.charge_work(
            (self.values.len() - at + 1) as u128,
            context.limits,
            context.location,
        )?;
        self.values.push(
            tuple,
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        self.values.slice_mut()[at..].rotate_right(1);
        Ok(())
    }
}
fn tuple_compare(
    left: &[Term],
    right: &[Term],
    context: &mut Context<'_, '_, '_>,
) -> Result<Ordering, FormulaFailure> {
    for (&left, &right) in left.iter().zip(right) {
        let (TemplateTerm::Constant(left), TemplateTerm::Constant(right)) =
            (context.term(left)?, context.term(right)?)
        else {
            unreachable!("closed source carrier tuples");
        };
        let order = left.compare_ref_with(right, || context.inspect())?;
        if !order.is_eq() {
            return Ok(order);
        }
    }
    context.inspect()?;
    Ok(left.len().cmp(&right.len()))
}

#[cfg(test)]
mod tests;
