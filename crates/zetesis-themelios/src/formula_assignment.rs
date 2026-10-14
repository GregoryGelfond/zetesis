//! An aggregate assignment's carrier: the measure of every selection that
//! contains its required tuples and lies within its possible ones. [`carrier`]
//! is the one operation that computes it, from [`Contributions`] of the kind
//! [`Kind::of`] maps the assignment's function to: weights for `#count`,
//! `#sum` and `#sum+`, first values for `#min` and `#max`. Joins propose an
//! assignment's values through [`values`], from complete, canonical full-tuple
//! local joins; final grounding's assignment families and the objective
//! refinement call [`carrier`] directly.
mod counts;
mod sums;
#[cfg(test)]
mod tests;
use sums::sums;

use crate::ProgramSite;
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::{AggregateIr, AggregateKey};
use crate::formula_support::components::Term;
use crate::formula_support::{
    Buffer, Computation, Context, Counters, GroundingWork, Join, Support, TermSelection,
};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};
use themelios_program::program::AggregateFunction;
use themelios_program::term::EvalError;
use zetesis_core::catalog::{AssignmentSlice, TermKey, TermRef};
use zetesis_core::{Value, ValueNodeRef};
use zetesis_ferraris::AggregateExtremum;

/// The proposed values of the assignment `aggregate` under the outer
/// `assignment`, in ascending term order: the carrier of the full tuples its
/// elements' local joins yield over `support`, each coalesced once. A tuple is
/// required when an element whose condition the join decides yields it, so no
/// support changes the required tuples. Final grounding's assignment family is
/// the carrier of the same required tuples over a support that contains this
/// one, and a carrier only grows with its possible tuples, so the family has a
/// root for every value proposed here. Charges work and storage as it reduces
/// the tuples; refuses as the local joins, the endpoint guard and [`carrier`]
/// do, or on the aggregate-element ceiling per distinct tuple.
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
    let contributions = split(
        Kind::of(aggregate.function),
        &tuples,
        &required_tuples,
        computation,
        limits,
        counters,
        location,
    )?;
    carrier(
        contributions.streams(),
        computation,
        limits,
        counters,
        location,
    )
}

/// What an aggregate's function reduces each of its tuples to, with the
/// function itself: a weight for `#count`, `#sum` and `#sum+`, the first value
/// for `#min` and `#max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Numeric(Numeric),
    Extremum(AggregateExtremum),
}

impl Kind {
    /// The kind of `function`, as the carrier and final grounding's coalescing
    /// and nonbinding guards read it. Three other sites split the functions
    /// the same way with their own match: the IR lowering, whose refusal of an
    /// empty `#min` or `#max` tuple guarantees the first value the extremum
    /// readers take, objective presence, and head aggregates.
    pub(crate) fn of(function: AggregateFunction) -> Self {
        match function {
            AggregateFunction::Count => Self::Numeric(Numeric::Count),
            AggregateFunction::Sum => Self::Numeric(Numeric::Sum),
            AggregateFunction::SumPlus => Self::Numeric(Numeric::SumPlus),
            AggregateFunction::Min => Self::Extremum(AggregateExtremum::Min),
            AggregateFunction::Max => Self::Extremum(AggregateExtremum::Max),
        }
    }

    /// The extremum, for `#min` and `#max`.
    pub(crate) fn extremum(self) -> Option<AggregateExtremum> {
        match self {
            Self::Extremum(extremum) => Some(extremum),
            Self::Numeric(_) => None,
        }
    }
}

/// A function whose tuples contribute weights: one each for `#count`, the
/// first value for `#sum`, and a positive first value for `#sum+`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Numeric {
    Count,
    Sum,
    SumPlus,
}

/// An assignment's contributions of the one kind its function reduces each
/// tuple to, split by side. The kind is outside the sides, so both sides are
/// of one kind. A numeric side is any stream of weights `W`, so final
/// grounding passes the weights its elements already hold without copying
/// them; an extremum side is its values `V`.
pub(crate) enum Contributions<W, V> {
    Numeric {
        function: Numeric,
        sides: Sides<W>,
    },
    Extremum {
        function: AggregateExtremum,
        sides: Sides<V>,
    },
}

/// Contributions reduced into storage: each side's weights in a buffer, or
/// its values in a binding.
pub(crate) type Reduced = Contributions<Buffer<i32>, Binding<'static>>;

impl Reduced {
    /// Empty sides for `kind`'s contributions.
    fn new(
        kind: Kind,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        Ok(match kind {
            Kind::Numeric(function) => Self::Numeric {
                function,
                sides: Sides {
                    required: Buffer::new(computation, limits, counters, location)?,
                    optional: Buffer::new(computation, limits, counters, location)?,
                },
            },
            Kind::Extremum(function) => Self::Extremum {
                function,
                sides: Sides {
                    required: Binding::new(computation, limits, counters, location)?,
                    optional: Binding::new(computation, limits, counters, location)?,
                },
            },
        })
    }

    /// These contributions as [`carrier`] reads them: each side's weights
    /// copied out of its buffer as they are read, or its values' slots.
    pub(crate) fn streams(
        &self,
    ) -> Contributions<std::iter::Copied<std::slice::Iter<'_, i32>>, AssignmentSlice<'_>> {
        match self {
            Self::Numeric { function, sides } => Contributions::Numeric {
                function: *function,
                sides: Sides {
                    required: sides.required.iter().copied(),
                    optional: sides.optional.iter().copied(),
                },
            },
            Self::Extremum { function, sides } => Contributions::Extremum {
                function: *function,
                sides: Sides {
                    required: sides.required.slots(),
                    optional: sides.optional.slots(),
                },
            },
        }
    }
}

/// Reduce each ordered tuple to its contribution under `kind`, on the side its
/// tuple belongs to: required when it is in `required_tuples`.
fn split(
    kind: Kind,
    tuples: &Binding<'static>,
    required_tuples: &TermSelection,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Reduced, FormulaFailure> {
    let mut split = Reduced::new(kind, computation, limits, counters, location)?;
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
            Contributions::Extremum { sides, .. } => {
                let first = first.expect("admitted nonempty extremum tuple");
                endpoint_guard(first, location)?;
                let key = read.term_key(first).map_err(|error| {
                    crate::formula_binding::assignment(
                        zetesis_core::catalog::AssignmentError::Read(error),
                        location,
                    )
                })?;
                append(
                    sides.side_mut(required),
                    &key,
                    computation,
                    limits,
                    counters,
                    location,
                )?;
            }
            Contributions::Numeric { function, sides } => {
                if let Some(weight) = contribution(*function, first) {
                    sides.side_mut(required).push(
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
#[derive(Clone, Copy)]
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

/// The numeric endpoint guard: refuses `value`, a `#min` or `#max` bound or the
/// first value of a possible `#min` or `#max` tuple, when it is `i32::MIN` or
/// `i32::MAX`
/// ([`AdmissionFailure::ExtremumEndpoint`](crate::AdmissionFailure::ExtremumEndpoint)).
pub(crate) fn endpoint_guard(
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
fn precedes(function: AggregateExtremum, order: std::cmp::Ordering) -> bool {
    match function {
        AggregateExtremum::Min => order.is_lt(),
        AggregateExtremum::Max => order.is_gt(),
    }
}

/// The proposed values of an assignment, in ascending term order: the measure
/// of every selection that contains its required tuples and lies within its
/// possible ones, from `contributions`.
///
/// - `#count`, `#sum` and `#sum+`: from each required and each optional
///   tuple's contribution (`#count`: one; `#sum` and `#sum+`: its weight).
///   Without a required tuple, every selection of the optional ones. Charges
///   work and cancellation per step, assignment values per distinct value and
///   storage through its buffers.
/// - `#min` and `#max`: with a required tuple, the required extremum and every
///   optional value strictly before it, since every selection contains the
///   required tuples; without one, the empty value (`#sup` for `#min`, `#inf`
///   for `#max`) and every optional value. Charges work per step and
///   assignment values per distinct value. The endpoint guard
///   ([`endpoint_guard`]) is the proposal sites' admission policy, applied
///   where they reduce their tuples to values; this operation applies no guard
///   of its own.
///
/// Precondition, for a numeric function: each side holds at most one
/// contribution per distinct full tuple, its reduction by [`contribution`],
/// and no tuple contributes to both sides; `#count` counts contributions,
/// never their values. Every caller takes its function's kind from
/// [`Kind::of`] and coalesces full tuples before reducing them: [`values`] in
/// one term selection, with each tuple on the side its required selection
/// names; final grounding by ground key, with one required flag per key; and
/// the objective refinement in its sorted closed tuples, where an optional
/// occurrence of a required key adds no contribution.
pub(crate) fn carrier<W: IntoIterator<Item = i32>>(
    contributions: Contributions<W, AssignmentSlice<'_>>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Binding<'static>, FormulaFailure> {
    match contributions {
        Contributions::Numeric { function, sides } => {
            numeric_values(function, sides, computation, limits, counters, location)
        }
        Contributions::Extremum { function, sides } => {
            extremum_values(function, sides, computation, limits, counters, location)
        }
    }
}

/// The carrier of a numeric function's weights, as [`carrier`] states it.
fn numeric_values(
    function: Numeric,
    sides: Sides<impl IntoIterator<Item = i32>>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Binding<'static>, FormulaFailure> {
    let Sides { required, optional } = sides;
    let numbers = match function {
        Numeric::Count => {
            let mut count = 0_usize;
            for _ in required {
                counters.work(limits, location)?;
                count += 1;
            }
            counts::counts(count, optional, computation, limits, counters, location)?
        }
        Numeric::Sum | Numeric::SumPlus => {
            let positive = |weight: &i32| function != Numeric::SumPlus || *weight > 0;
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
    };
    let mut result = Binding::new(computation, limits, counters, location)?;
    result.extend_scope(numbers.len(), computation, limits, counters, location)?;
    for (slot, &number) in numbers.iter().enumerate() {
        let key = computation.number(number, limits, counters, location)?;
        result.set(slot, &key, limits, counters, location)?;
    }
    Ok(result)
}

/// The carrier of an extremum's values, as [`carrier`] states it.
fn extremum_values(
    function: AggregateExtremum,
    sides: Sides<AssignmentSlice<'_>>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Binding<'static>, FormulaFailure> {
    let Sides { required, optional } = sides;
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
            AggregateExtremum::Min => Value::Supremum,
            AggregateExtremum::Max => Value::Infimum,
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

/// What one tuple contributes to a numeric `function`, from its `first`
/// value: one for `#count`, its number for `#sum`, its positive number for
/// `#sum+`, and nothing for a tuple a sum does not count.
pub(crate) fn contribution(function: Numeric, first: Option<TermRef<'_>>) -> Option<i32> {
    match (function, first.map(TermRef::descriptor)) {
        (Numeric::Count, _) => Some(1),
        (Numeric::Sum, Some(ValueNodeRef::Number(weight))) => Some(weight),
        (Numeric::SumPlus, Some(ValueNodeRef::Number(weight))) if weight > 0 => Some(weight),
        (Numeric::Sum | Numeric::SumPlus, _) => None,
    }
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
