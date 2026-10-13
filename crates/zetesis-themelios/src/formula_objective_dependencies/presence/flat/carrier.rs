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
use zetesis_core::TemplateTerm;
use zetesis_core::catalog::{TermKey, TermRef};

use super::{Activity, Context, activity, assignment, transport};
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{AggregateKey, LiteralIr, Prepared, RuleIr};
use crate::formula_support::{Buffer, Computation, Counters, TermSelection};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

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
    let required_contributions = contributions(
        aggregate.function,
        required.values.iter().copied(),
        Entries::Unreserved,
        &mut context,
    )?;
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
    let optional_contributions = contributions(
        aggregate.function,
        optional.iter().copied(),
        Entries::Reserved,
        &mut context,
    )?;
    let values = complete(
        aggregate.function,
        &required_contributions,
        &optional_contributions,
        &mut context,
    )?;
    Ok(Some(Carrier { predicates, values }))
}

/// What each closed tuple contributes to the carrier operation the assignment
/// proposal sites share: its weight for `#count`, `#sum` and `#sum+`, its first
/// value for `#min` and `#max`.
enum Contributions {
    Weights(Buffer<i32>),
    Values(Binding<'static>),
}

/// Whether each numeric contribution takes an entry as it is reduced: the
/// optional ones do, one per weight before any sum is formed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Entries {
    Reserved,
    Unreserved,
}

/// Reduce `tuples` to their contributions.
fn contributions<'a>(
    function: AggregateFunction,
    tuples: impl Iterator<Item = &'a [Term]>,
    entries: Entries,
    context: &mut Context<'_, '_, '_>,
) -> Result<Contributions, FormulaFailure> {
    match function {
        AggregateFunction::Count | AggregateFunction::Sum | AggregateFunction::SumPlus => {
            let mut weights = Buffer::new(
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?;
            for tuple in tuples {
                context.inspect()?;
                if let Some(weight) = crate::formula_assignment::contribution(
                    function,
                    first(tuple, context)?,
                    context.location,
                )? {
                    if entries == Entries::Reserved {
                        context.reserve_entry()?;
                    }
                    weights.push(
                        weight,
                        context.computation,
                        context.limits,
                        context.counters,
                        context.location,
                    )?;
                }
            }
            Ok(Contributions::Weights(weights))
        }
        AggregateFunction::Min | AggregateFunction::Max => {
            let mut values = Binding::new(
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?;
            for tuple in tuples {
                context.inspect()?;
                let value = first(tuple, context)?.expect("admitted nonempty extremum tuple");
                let key = context
                    .computation
                    .read()
                    .term_key(value)
                    .map_err(|error| {
                        crate::formula_binding::assignment(error.into(), context.location)
                    })?;
                crate::formula_assignment::append(
                    &mut values,
                    &key,
                    context.computation,
                    context.limits,
                    context.counters,
                    context.location,
                )?;
            }
            Ok(Contributions::Values(values))
        }
    }
}

/// The carrier between the required and the possible tuples, from the shared
/// carrier operation; each retained value takes an entry. Optional keys are
/// independent only in the source abstraction. Original eligibility remains
/// responsible for their correlation in an interpretation.
fn complete(
    function: AggregateFunction,
    required: &Contributions,
    optional: &Contributions,
    context: &mut Context<'_, '_, '_>,
) -> Result<TermSelection, FormulaFailure> {
    let proposals = match (required, optional) {
        (Contributions::Weights(required), Contributions::Weights(optional)) => {
            crate::formula_assignment::carrier(
                function,
                required.iter().copied(),
                optional.iter().copied(),
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?
        }
        (Contributions::Values(required), Contributions::Values(optional)) => {
            crate::formula_assignment::extrema_carrier(
                function,
                required.slots(),
                optional.slots(),
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?
        }
        _ => unreachable!("one function reduces both sides to one kind of contribution"),
    };
    let mut values = TermSelection::new(
        context.computation,
        context.limits,
        context.counters,
        context.location,
    )?;
    for slot in 0..proposals.len() {
        let value = proposals
            .slots()
            .key(slot)
            .map_err(|error| crate::formula_binding::assignment(error, context.location))?
            .expect("carrier values have no missing entries");
        retain_value(&mut values, &value, context)?;
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
