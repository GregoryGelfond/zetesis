//! The lifted objective's complete numeric-presence scan, shared by the
//! ordinary cursor and the private parallel cursor adapter. Every row remains
//! visited: Boolean absorption does not establish source-family completion.

use super::{
    Binding, Budget, Computation, Context, Evidence, FormulaFailure, GroundingWork, Join,
    ObjectiveField, ObjectiveIr, ProgramSite, Support, TermKey, TermRef, ValueNodeRef, Warnings,
    formula_support,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod scope_tests;

/// Only called after the existing lifted-objective predicate has succeeded.
/// A checked constant or an `AsWritten` bound variable cannot hide an earlier
/// per-row scalar failure behind a later task's resource stop. Other cases
/// retain the original serial cursor and its failure order.
fn parallel_weight(
    objective: &ObjectiveIr,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    counters.work(limits, location)?;
    match &objective.weight {
        ObjectiveField::Term(crate::formula_support::components::Term::Variable(_)) => {
            Ok(objective.polarity == zetesis_objective::WeightPolarity::AsWritten)
        }
        ObjectiveField::Term(crate::formula_support::components::Term::Constant(weight)) => {
            Ok(matches!(
                computation.static_scalar(*weight, limits, counters, location)?.descriptor(),
                ValueNodeRef::Number(weight) if objective.polarity.normalize(weight).is_some()
            ))
        }
        ObjectiveField::Expression(_) => Ok(false),
    }
}

pub(super) fn try_parallel<'source>(
    executor: &crate::formula_support::Executor,
    objective: &'source ObjectiveIr,
    bindings: &mut Join<'_, 'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<Option<(bool, Evidence)>, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    if !parallel_weight(
        objective,
        Context::new(&*computation, limits, counters, location),
    )? {
        return Ok(None);
    }
    // Source/query identity is authenticated by the common cursor adapter.
    // Do not capture `support`: its mutable query caches stay on the caller.
    crate::formula_support::row_blocks::parallel_numeric_rows(
        executor,
        objective,
        bindings,
        budget,
        Context::new(computation, limits, counters, location),
        |bindings, support, budget, context| {
            scan(
                objective,
                bindings,
                support,
                budget,
                &mut Warnings::default(),
                context,
            )
        },
    )
}

pub(super) fn scan<'source>(
    objective: &'source ObjectiveIr,
    bindings: &mut Join<'_, 'source>,
    support: &Support<'source>,
    budget: &mut Budget,
    warnings: &mut Warnings,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<(bool, Evidence), FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let mut numeric = false;
    let mut scopes = super::super::arithmetic::Scopes::new(objective.condition.literals());
    while let Some(row) = bindings.next_row(computation, limits, budget, counters, location)? {
        if row.passes {
            scopes.validate(
                &row.values,
                support,
                budget,
                warnings,
                Context::new(&mut *computation, limits, counters, location),
            )?;
        }
        let weight = weight_descriptor(
            &objective.weight,
            &row.values,
            Context::new(&mut *computation, limits, counters, location),
        )?;
        if let ValueNodeRef::Number(weight) = weight {
            if objective.polarity.normalize(weight).is_none() {
                if !row.passes {
                    continue;
                }
                return Err(crate::diagnostic::unsupported(
                    crate::ProfileFeature::NumericOverflow,
                    location,
                )
                .into());
            }
            numeric |= row.passes;
        }
    }
    Ok((numeric, bindings.take_family()))
}

/// The weight's value as [`field`] computes it, with the same charges. A term
/// is read in place rather than through an owned key, so concurrent scans
/// share no reference count.
fn weight_descriptor<'read>(
    weight: &ObjectiveField,
    binding: &Binding,
    context: Context<'_, &'read mut Computation<'_, '_>>,
) -> Result<ValueNodeRef<'read>, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    match weight {
        ObjectiveField::Term(crate::formula_support::components::Term::Variable(slot)) => {
            counters.work(limits, location)?;
            Ok(binding
                .read(*slot, computation.read(), location)?
                .descriptor())
        }
        ObjectiveField::Term(crate::formula_support::components::Term::Constant(value)) => {
            counters.work(limits, location)?;
            let value = computation.static_scalar(*value, limits, counters, location)?;
            counters.work(limits, location)?;
            Ok(value.descriptor())
        }
        ObjectiveField::Expression(_) => {
            let key = field(
                weight,
                binding,
                Context::new(&mut *computation, limits, counters, location),
            )?;
            let computation: &'read Computation<'_, '_> = computation;
            Ok(value_ref(computation, &key, location)?.descriptor())
        }
    }
}

pub(super) fn field(
    field: &ObjectiveField,
    binding: &Binding,
    context: Context<'_, &mut Computation<'_, '_>>,
) -> Result<TermKey, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    counters.work(limits, location)?;
    match field {
        ObjectiveField::Term(value) => term(
            value,
            binding,
            Context::new(computation, limits, counters, location),
        ),
        ObjectiveField::Expression(expression) => formula_support::expression(
            expression,
            binding,
            computation,
            limits,
            counters,
            location,
        ),
    }
}

pub(super) fn term(
    term: &crate::formula_support::components::Term,
    binding: &Binding,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<TermKey, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    match term {
        crate::formula_support::components::Term::Variable(slot) => binding.key(*slot, location),
        crate::formula_support::components::Term::Constant(value) => {
            computation.static_key(*value, limits, counters, location)
        }
    }
}

pub(super) fn value_ref<'read>(
    computation: &'read Computation<'_, '_>,
    key: &TermKey,
    location: ProgramSite,
) -> Result<TermRef<'read>, FormulaFailure> {
    computation.read().term(key).map_err(|error| {
        crate::formula_binding::assignment(
            zetesis_core::catalog::AssignmentError::Read(error),
            location,
        )
    })
}
