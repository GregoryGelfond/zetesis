//! Bounded candidate values from complete, canonical full-tuple local joins.
mod sums;
pub(crate) use sums::sums;

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::{AggregateIr, AggregateKey};
use crate::formula_support::components::Term;
use crate::formula_support::{
    Buffer, Computation, Context, Counters, GroundingWork, Join, Support, TermSelection,
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature};
use themelios_base::span::Location;
use themelios_program::program::AggregateFunction;
use zetesis_core::catalog::{AssignmentSlice, TermKey, TermRef};
use zetesis_core::{Value, ValueNodeRef};

pub(crate) fn values(
    aggregate: &AggregateIr,
    assignment: &Binding,
    support: &Support,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, '_>>,
) -> Result<Binding<'static>, FormulaFailure> {
    let Context { computation, work } = context;
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    let mut tuples = TermSelection::new(computation, limits, counters, location)?;
    for element in &aggregate.elements {
        let mut local = Join::new(
            &element.condition,
            assignment,
            element.variables,
            support,
            budget,
            Context::new(&*computation, limits, counters, location),
        )?;
        while let Some(binding) = local.next(computation, limits, budget, counters, location)? {
            let AggregateKey::Tuple(terms) = &element.key else {
                unreachable!("assignment is a function aggregate")
            };
            let tuple = tuple(terms, &binding, computation, limits, counters, location)?;
            tuples.insert(
                &tuple,
                FormulaResource::AggregateElements,
                limits.aggregate.max_elements,
                Context::new(&*computation, limits, counters, location),
            )?;
        }
    }
    let tuples = tuples.ordered(true, computation, limits, counters, location)?;
    let mut firsts = Binding::new(computation, limits, counters, location)?;
    let mut weights = Buffer::new(computation, limits, counters, location)?;
    for slot in 0..tuples.len() {
        counters.work(limits, location)?;
        let read = computation.read();
        let first = tuples.read(slot, read, location)?.child(0);
        if matches!(
            aggregate.function,
            AggregateFunction::Min | AggregateFunction::Max
        ) {
            let first = first.expect("admitted nonempty extremum tuple");
            extremum_value(first, location)?;
            let key = read.term_key(first).map_err(|error| {
                crate::formula_binding::assignment(
                    zetesis_core::catalog::AssignmentError::Read(error),
                    location,
                )
            })?;
            firsts.extend_scope(slot + 1, computation, limits, counters, location)?;
            firsts.set(slot, &key, limits, counters, location)?;
        } else if let Some(weight) = contribution(aggregate.function, first, location)? {
            weights.push(weight, computation, limits, counters, location)?;
        }
    }
    if matches!(
        aggregate.function,
        AggregateFunction::Min | AggregateFunction::Max
    ) {
        extrema_candidates(
            aggregate.function,
            firsts.slots(),
            computation,
            limits,
            counters,
            location,
        )
    } else {
        candidates(
            aggregate.function,
            weights.iter().copied(),
            computation,
            limits,
            counters,
            location,
        )
    }
}

pub(crate) fn tuple(
    terms: &[Term],
    binding: &Binding,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<TermKey, FormulaFailure> {
    let mut fields = Binding::new(computation, limits, counters, location)?;
    let mut children = Buffer::new(computation, limits, counters, location)?;
    fields.extend_scope(terms.len(), computation, limits, counters, location)?;
    for (slot, term) in terms.iter().enumerate() {
        counters.work(limits, location)?;
        let key = match term {
            Term::Constant(value) => computation.static_key(*value, limits, counters, location)?,
            Term::Variable(variable) => binding.key(*variable, location)?,
        };
        fields.set(slot, &key, limits, counters, location)?;
        children.push(slot, computation, limits, counters, location)?;
    }
    // This wrapper identifies an already admitted source tuple. Its added root
    // is metadata, not a fresh source expression with a new depth ceiling.
    computation.construct(
        ValueNodeRef::Tuple { arity: terms.len() },
        fields.slots(),
        children.slice(),
        zetesis_core::catalog::Limits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        },
        GroundingWork::new(limits, counters, location),
    )
}

pub(crate) fn extremum_value(value: TermRef<'_>, location: Location) -> Result<(), FormulaFailure> {
    if let ValueNodeRef::Number(endpoint @ (i32::MIN | i32::MAX)) = value.descriptor() {
        return Err(crate::AdmissionFailure::ExtremumEndpoint {
            value: endpoint,
            location,
        }
        .into());
    }
    Ok(())
}

pub(crate) fn extrema_candidates(
    function: AggregateFunction,
    possible: AssignmentSlice<'_>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<Binding<'static>, FormulaFailure> {
    let mut values = TermSelection::new(computation, limits, counters, location)?;
    let empty = match function {
        AggregateFunction::Min => Value::Supremum,
        AggregateFunction::Max => Value::Infimum,
        _ => unreachable!("value candidates are extrema only"),
    };
    let empty = computation.import((&empty).into(), limits, counters, location)?;
    values.insert(
        &empty,
        FormulaResource::AssignmentValues,
        limits.max_assignment_values,
        Context::new(&*computation, limits, counters, location),
    )?;
    for slot in 0..possible.len() {
        counters.work(limits, location)?;
        let key = possible
            .key(slot)
            .map_err(|error| crate::formula_binding::assignment(error, location))?
            .expect("possible extrema list has no missing entries");
        let value = computation.read().term(&key).map_err(|error| {
            crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Read(error),
                location,
            )
        })?;
        extremum_value(value, location)?;
        values.insert(
            &key,
            FormulaResource::AssignmentValues,
            limits.max_assignment_values,
            Context::new(&*computation, limits, counters, location),
        )?;
    }
    values.ordered(false, computation, limits, counters, location)
}

pub(crate) fn contribution(
    function: AggregateFunction,
    first: Option<TermRef<'_>>,
    location: Location,
) -> Result<Option<i32>, FormulaFailure> {
    match (function, first.map(TermRef::descriptor)) {
        (AggregateFunction::Count, _) => Ok(Some(1)),
        (AggregateFunction::SumPlus, Some(ValueNodeRef::Number(weight))) if weight > 0 => {
            Ok(Some(weight))
        }
        (AggregateFunction::SumPlus, _) => Ok(None),
        (_, Some(ValueNodeRef::Number(weight))) => Ok(Some(weight)),
        (AggregateFunction::Sum, _) => Ok(None),
        _ => Err(unsupported(ProfileFeature::Aggregate, location).into()),
    }
}

pub(crate) fn candidates(
    function: AggregateFunction,
    weights: impl IntoIterator<Item = i32>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<Binding<'static>, FormulaFailure> {
    if matches!(
        function,
        AggregateFunction::Count | AggregateFunction::Sum | AggregateFunction::SumPlus
    ) {
        let numbers = sums(
            weights
                .into_iter()
                .filter(|weight| function != AggregateFunction::SumPlus || *weight > 0),
            computation,
            limits,
            counters,
            location,
        )?;
        let mut result = Binding::new(computation, limits, counters, location)?;
        result.extend_scope(numbers.len(), computation, limits, counters, location)?;
        for (slot, &number) in numbers.iter().enumerate() {
            let key = computation.number(number, limits, counters, location)?;
            result.set(slot, &key, limits, counters, location)?;
        }
        return Ok(result);
    }
    let mut possible = Binding::new(computation, limits, counters, location)?;
    for weight in weights {
        let slot = possible.len();
        possible.extend_scope(slot + 1, computation, limits, counters, location)?;
        let key = computation.number(weight, limits, counters, location)?;
        possible.set(slot, &key, limits, counters, location)?;
    }
    extrema_candidates(
        function,
        possible.slots(),
        computation,
        limits,
        counters,
        location,
    )
}
