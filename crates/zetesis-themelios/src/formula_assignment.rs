//! An aggregate assignment's carrier: the measure of every selection that
//! contains its required tuples and lies within its possible ones. [`carrier`]
//! computes it from numeric contributions and [`extrema_carrier`] from extremum
//! values. Joins propose an assignment's values through [`values`], from
//! complete, canonical full-tuple local joins; final grounding's assignment
//! families and the objective refinement call the two operations directly.
mod counts;
mod sums;
#[cfg(test)]
mod tests;
use sums::sums;

use crate::ProgramSite;
use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::{AggregateIr, AggregateKey};
use crate::formula_support::components::Term;
use crate::formula_support::{
    Buffer, Computation, Context, Counters, GroundingWork, Join, Support, TermSelection,
};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature};
use themelios_program::program::AggregateFunction;
use themelios_program::term::EvalError;
use zetesis_core::catalog::{AssignmentSlice, TermKey, TermRef};
use zetesis_core::{Value, ValueNodeRef};

/// The proposed values of the assignment `aggregate` under the outer
/// `assignment`, in ascending term order: the carrier of the full tuples its
/// elements' local joins yield over `support`, each coalesced once. A tuple is
/// required when an element whose condition the join decides yields it, so no
/// support changes the required tuples. Final grounding's assignment family is
/// the carrier of the same required tuples over a support that contains this
/// one, and a carrier only grows with its possible tuples, so the family has a
/// root for every value proposed here. Charges work and storage as it reduces
/// the tuples; refuses as the local joins, the endpoint guard and the carrier
/// operations do, or on the aggregate-element ceiling per distinct tuple.
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
    // The tuples with an occurrence from an element whose condition the local
    // join decides: present in every interpretation for this binding.
    let mut required_tuples = TermSelection::new(computation, limits, counters, location)?;
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
            if element.required {
                required_tuples.insert(
                    &tuple,
                    FormulaResource::AggregateElements,
                    limits.aggregate.max_elements,
                    Context::new(&*computation, limits, counters, location),
                )?;
            }
        }
    }
    let tuples = tuples.ordered(true, computation, limits, counters, location)?;
    match split(
        aggregate.function,
        &tuples,
        &required_tuples,
        computation,
        limits,
        counters,
        location,
    )? {
        Split::Values(firsts) => extrema_carrier(
            aggregate.function,
            firsts.required.slots(),
            firsts.optional.slots(),
            computation,
            limits,
            counters,
            location,
        ),
        Split::Weights(weights) => carrier(
            aggregate.function,
            weights.required.iter().copied(),
            weights.optional.iter().copied(),
            computation,
            limits,
            counters,
            location,
        ),
    }
}

/// An assignment's contributions split by side: first values for `#min` and
/// `#max`, weights for `#count`, `#sum` and `#sum+`.
enum Split {
    Values(Sides<Binding<'static>>),
    Weights(Sides<Buffer<i32>>),
}

/// Reduce each ordered tuple to its contribution, on the side of the split its
/// tuple belongs to: required when it is in `required_tuples`.
fn split(
    function: AggregateFunction,
    tuples: &Binding<'static>,
    required_tuples: &TermSelection,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Split, FormulaFailure> {
    let mut split = if matches!(function, AggregateFunction::Min | AggregateFunction::Max) {
        Split::Values(Sides {
            required: Binding::new(computation, limits, counters, location)?,
            optional: Binding::new(computation, limits, counters, location)?,
        })
    } else {
        Split::Weights(Sides {
            required: Buffer::new(computation, limits, counters, location)?,
            optional: Buffer::new(computation, limits, counters, location)?,
        })
    };
    for slot in 0..tuples.len() {
        counters.work(limits, location)?;
        let key = tuples
            .slots()
            .key(slot)
            .map_err(|error| crate::formula_binding::assignment(error, location))?
            .expect("ordered tuples have no missing entries");
        let required = required_tuples.contains(&key, limits, counters, location)?;
        let read = computation.read();
        let first = tuples.read(slot, read, location)?.child(0);
        match &mut split {
            Split::Values(firsts) => {
                let first = first.expect("admitted nonempty extremum tuple");
                extremum_value(first, location)?;
                let key = read.term_key(first).map_err(|error| {
                    crate::formula_binding::assignment(
                        zetesis_core::catalog::AssignmentError::Read(error),
                        location,
                    )
                })?;
                append(
                    firsts.side_mut(required),
                    &key,
                    computation,
                    limits,
                    counters,
                    location,
                )?;
            }
            Split::Weights(weights) => {
                if let Some(weight) = contribution(function, first, location)? {
                    weights.side_mut(required).push(
                        weight,
                        computation,
                        limits,
                        counters,
                        location,
                    )?;
                }
            }
        }
    }
    Ok(split)
}

/// Append `key` as the last slot of `values`.
pub(crate) fn append(
    values: &mut Binding<'static>,
    key: &TermKey,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    let at = values.len();
    values.extend_scope(at + 1, computation, limits, counters, location)?;
    values.set(at, key, limits, counters, location)
}

/// An assignment's contributions split by whether every selection contains
/// their tuple: the required tuples' and the optional ones'.
pub(crate) struct Sides<T> {
    pub(crate) required: T,
    pub(crate) optional: T,
}

impl<T> Sides<T> {
    /// The required side when `required`, else the optional one.
    pub(crate) fn side_mut(&mut self, required: bool) -> &mut T {
        if required {
            &mut self.required
        } else {
            &mut self.optional
        }
    }
}

pub(crate) fn tuple(
    terms: &[Term],
    binding: &Binding,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
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

pub(crate) fn extremum_value(
    value: TermRef<'_>,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    if let ValueNodeRef::Number(endpoint @ (i32::MIN | i32::MAX)) = value.descriptor() {
        return Err(crate::AdmissionFailure::ExtremumEndpoint {
            value: endpoint,
            location,
        }
        .into());
    }
    Ok(())
}

/// Whether `order`, a value's order against the current extremum, puts the
/// value strictly before it in `function`'s order: below for `#min`, above for
/// `#max`.
fn precedes(function: AggregateFunction, order: std::cmp::Ordering) -> bool {
    match function {
        AggregateFunction::Min => order.is_lt(),
        AggregateFunction::Max => order.is_gt(),
        _ => unreachable!("extremum order applies to #min and #max only"),
    }
}

/// The proposed values of a `#min` or `#max` assignment from its required and
/// optional tuples' values. With a required tuple: the required extremum and
/// every optional value strictly before it, since every selection contains the
/// required tuples. Without one: the empty value (`#sup` for `#min`, `#inf` for
/// `#max`) and every optional value. Charges work per step and assignment
/// values per distinct value; returns the values in ascending term order. The
/// endpoint guard ([`extremum_value`]) is the proposal sites' admission policy,
/// applied where they reduce their tuples to values; this operation applies no
/// guard of its own.
pub(crate) fn extrema_carrier(
    function: AggregateFunction,
    required: AssignmentSlice<'_>,
    optional: AssignmentSlice<'_>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Binding<'static>, FormulaFailure> {
    let mut values = TermSelection::new(computation, limits, counters, location)?;
    let mut extremum: Option<TermKey> = None;
    for slot in 0..required.len() {
        counters.work(limits, location)?;
        let key = slice_key(required, slot, location)?;
        let value = read_term(computation, &key, location)?;
        let before = match &extremum {
            None => true,
            Some(current) => {
                let current = read_term(computation, current, location)?;
                precedes(
                    function,
                    value.compare_terms_with(current, || counters.work(limits, location))?,
                )
            }
        };
        if before {
            extremum = Some(key);
        }
    }
    if let Some(extremum) = &extremum {
        values.insert(
            extremum,
            FormulaResource::AssignmentValues,
            limits.max_assignment_values,
            Context::new(&*computation, limits, counters, location),
        )?;
    } else {
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
    }
    for slot in 0..optional.len() {
        counters.work(limits, location)?;
        let key = slice_key(optional, slot, location)?;
        let value = read_term(computation, &key, location)?;
        if let Some(current) = &extremum {
            let current = read_term(computation, current, location)?;
            let order = value.compare_terms_with(current, || counters.work(limits, location))?;
            if !precedes(function, order) {
                continue;
            }
        }
        values.insert(
            &key,
            FormulaResource::AssignmentValues,
            limits.max_assignment_values,
            Context::new(&*computation, limits, counters, location),
        )?;
    }
    values.ordered(false, computation, limits, counters, location)
}

fn slice_key(
    slice: AssignmentSlice<'_>,
    slot: usize,
    location: ProgramSite,
) -> Result<TermKey, FormulaFailure> {
    Ok(slice
        .key(slot)
        .map_err(|error| crate::formula_binding::assignment(error, location))?
        .expect("an extremum list has no missing entries"))
}

fn read_term<'a>(
    computation: &'a Computation<'_, '_>,
    key: &TermKey,
    location: ProgramSite,
) -> Result<TermRef<'a>, FormulaFailure> {
    computation.read().term(key).map_err(|error| {
        crate::formula_binding::assignment(
            zetesis_core::catalog::AssignmentError::Read(error),
            location,
        )
    })
}

pub(crate) fn contribution(
    function: AggregateFunction,
    first: Option<TermRef<'_>>,
    location: ProgramSite,
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

/// The proposed values of a `#count`, `#sum` or `#sum+` assignment: the measure
/// of every selection between its required tuples and its possible ones, from
/// each required and each optional tuple's contribution (`#count`: one; `#sum`
/// and `#sum+`: its weight). Without a required tuple, every selection of the
/// optional ones. Charges work and cancellation per step, assignment values
/// per distinct value and storage through its buffers; returns the values in
/// ascending order. Extremum contributions are terms, not numbers, and go to
/// [`extrema_carrier`].
///
/// Precondition: each side holds at most one contribution per distinct full
/// tuple, its reduction by [`contribution`] under `function`, and no tuple
/// contributes to both sides; `#count` counts contributions, never their
/// values. Every caller coalesces full tuples before reducing them: [`values`]
/// in one term selection, with each tuple on the side its required selection
/// names; final grounding by ground key, with one required flag per key; and
/// the objective refinement in its sorted closed tuples, where an optional
/// occurrence of a required key adds no contribution.
pub(crate) fn carrier(
    function: AggregateFunction,
    required: impl IntoIterator<Item = i32>,
    optional: impl IntoIterator<Item = i32>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Binding<'static>, FormulaFailure> {
    let positive = |weight: &i32| function != AggregateFunction::SumPlus || *weight > 0;
    let numbers = match function {
        AggregateFunction::Count => {
            let mut count = 0_usize;
            for _ in required {
                counters.work(limits, location)?;
                count += 1;
            }
            counts::counts(count, optional, computation, limits, counters, location)?
        }
        AggregateFunction::Sum | AggregateFunction::SumPlus => {
            let seed = required_total(
                required.into_iter().filter(positive),
                limits,
                counters,
                location,
            )?;
            sums(
                seed,
                optional.into_iter().filter(positive),
                computation,
                limits,
                counters,
                location,
            )?
        }
        AggregateFunction::Min | AggregateFunction::Max => {
            unreachable!("extremum contributions are terms and go to extrema_carrier")
        }
    };
    let mut result = Binding::new(computation, limits, counters, location)?;
    result.extend_scope(numbers.len(), computation, limits, counters, location)?;
    for (slot, &number) in numbers.iter().enumerate() {
        let key = computation.number(number, limits, counters, location)?;
        result.set(slot, &key, limits, counters, location)?;
    }
    Ok(result)
}

/// The total of the required contributions, summed exactly. It is the measure
/// of the selection of exactly the required tuples, so a total outside `i32`
/// refuses the aggregate as an overflow, whatever the summation order.
fn required_total(
    weights: impl IntoIterator<Item = i32>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<i32, FormulaFailure> {
    let overflow = || ExpansionFailure::Evaluation {
        error: EvalError::Overflow,
        location,
    };
    let mut total = 0_i64;
    for weight in weights {
        counters.work(limits, location)?;
        total = total.checked_add(i64::from(weight)).ok_or_else(overflow)?;
    }
    Ok(i32::try_from(total).map_err(|_| overflow())?)
}
