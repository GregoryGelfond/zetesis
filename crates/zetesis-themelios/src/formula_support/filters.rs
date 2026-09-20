//! Complete-row arithmetic classification. Independent false scalar checks
//! exclude a row before its retained arithmetic failure is reached.

use super::{Comparisons, Counters, Coverage, Evaluation, Failures, Join, compare, comparison};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, LiteralIr};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits};
use themelios_base::span::Location;
use themelios_program::program::Relation;
use zetesis_core::Value;

pub(super) enum Selection {
    Defined(bool),
    Excluded,
    Zero(ExpansionFailure),
}

impl Join<'_, '_> {
    pub(super) fn filters(
        &mut self,
        binding: &Binding,
        comparisons: Comparisons,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Selection, FormulaFailure> {
        let mut passes = true;
        let mut excluded = false;
        let mut failure = match comparisons {
            Comparisons::Failed(error) => {
                Some((ExpansionFailure::Evaluation { error, location }, false))
            }
            Comparisons::Verified | Comparisons::Deferred => None,
        };
        for (index, literal) in self.literals.iter().enumerate() {
            if crate::formula_binding_cursor::target(literal)
                .is_some_and(|target| target >= binding.len())
                || (self.coverage == Coverage::Selected
                    && comparison(literal).is_some()
                    && self.decisions.decides(index))
            {
                continue;
            }
            let result = check(
                literal,
                binding,
                &mut self.evaluation,
                limits,
                budget,
                counters,
                location,
            );
            excluded |= matches!(result, Ok(false)) && self.decisions.decides(index);
            retain(
                result,
                self.evaluation.zero_divisor(),
                &mut passes,
                &mut failure,
            )?;
        }
        for expression in self.head_bounds.iter().map(|guard| &guard.bound).chain(
            self.checked_guard
                .into_iter()
                .flat_map(crate::formula_guard::Guard::expressions),
        ) {
            let result = self
                .evaluation
                .source_expression(
                    expression,
                    |variable| binding.read(variable, location),
                    limits,
                    budget,
                    counters,
                    location,
                )
                .map(|_| true);
            retain(
                result,
                self.evaluation.zero_divisor(),
                &mut passes,
                &mut failure,
            )?;
        }
        match failure {
            None => Ok(Selection::Defined(passes)),
            Some(_) if excluded => Ok(Selection::Excluded),
            Some((error, true)) => Ok(Selection::Zero(error)),
            Some((error, false)) => Err(error.into()),
        }
    }
}

/// Available checks may exclude a failed generator. A failed check never
/// excludes, and an unavailable generated input is not read here.
pub(super) fn excludes(
    (literals, decisions): (&[LiteralIr], &super::order::Decisions),
    evaluation: &mut Evaluation,
    binding: &Binding,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    let mut excluded = false;
    let mut fatal = None;
    for (index, literal) in literals.iter().enumerate() {
        match pending_check(
            literal, binding, evaluation, limits, budget, counters, location,
        ) {
            Ok(passes) => excluded |= !passes && decisions.decides(index),
            Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. })) => {
                if !evaluation.zero_divisor() {
                    fatal.get_or_insert(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
    if !excluded && let Some(error) = fatal {
        return Err(error.into());
    }
    Ok(excluded)
}

fn retain(
    result: Result<bool, FormulaFailure>,
    zero: bool,
    passes: &mut bool,
    failure: &mut Option<(ExpansionFailure, bool)>,
) -> Result<(), FormulaFailure> {
    match result {
        Ok(value) => *passes &= value,
        Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. })) => {
            if failure.is_none() || (!zero && failure.as_ref().is_some_and(|(_, zero)| *zero)) {
                *failure = Some((error, zero));
            }
        }
        Err(error) => return Err(error),
    }
    Ok(())
}

/// Only this rejected-cursor path interprets an absent generated value as an
/// inherited arithmetic-unavailable input. No such value enters an emitted row.
fn pending_check(
    literal: &LiteralIr,
    binding: &Binding,
    evaluation: &mut Evaluation,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    if let Some((left, relation, right)) = comparison(literal) {
        let mut failures = Failures::default();
        let result = evaluation.source_partial(
            left,
            |slot| partial_input(binding, slot, location),
            limits,
            budget,
            counters,
            location,
        );
        let left = failures.value(result, evaluation.zero_divisor())?;
        let result = evaluation.source_partial(
            right,
            |slot| partial_input(binding, slot, location),
            limits,
            budget,
            counters,
            location,
        );
        let right = failures.value(result, evaluation.zero_divisor())?;
        failures.finish(evaluation)?;
        return Ok(compare(
            &left.expect("defined left comparison component"),
            relation,
            &right.expect("defined right comparison component"),
        ));
    }
    let mut failures = Failures::default();
    let mut visit = |expression: &Expression| -> Result<(), FormulaFailure> {
        let result = evaluation.source_partial(
            expression,
            |slot| partial_input(binding, slot, location),
            limits,
            budget,
            counters,
            location,
        );
        failures.value(result, evaluation.zero_divisor())?;
        Ok(())
    };
    match literal {
        LiteralIr::TupleCompare(left, _, right) => {
            for expression in left.iter().chain(right) {
                visit(expression)?;
            }
        }
        LiteralIr::Guard(guard) => {
            for expression in guard.expressions() {
                visit(expression)?;
            }
        }
        LiteralIr::Range {
            lower,
            upper,
            binder: false,
            ..
        } => {
            visit(lower)?;
            visit(upper)?;
        }
        LiteralIr::Aggregate(aggregate) => {
            for guard in &aggregate.guards {
                visit(&guard.bound)?;
            }
        }
        _ => {}
    }
    failures.finish(evaluation)?;
    // These forms are not ordinary relational comparisons and cannot exclude
    // arithmetic in a neighboring expression, whatever their truth value.
    Ok(true)
}

fn partial_input<'a>(
    binding: &'a Binding<'_>,
    slot: usize,
    location: Location,
) -> Result<Option<&'a Value>, FormulaFailure> {
    binding
        .slots()
        .get(slot)
        .map(Option::as_ref)
        .ok_or(FormulaFailure::UnsafeVariable {
            variable: slot,
            location,
        })
}

fn check(
    literal: &LiteralIr,
    binding: &Binding,
    evaluation: &mut Evaluation,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    if let LiteralIr::Aggregate(aggregate) = literal {
        let mut failures = Failures::default();
        for guard in &aggregate.guards {
            let result = evaluation.source_expression(
                &guard.bound,
                |variable| binding.read(variable, location),
                limits,
                budget,
                counters,
                location,
            );
            failures.value(result, evaluation.zero_divisor())?;
        }
        failures.finish(evaluation)?;
        return Ok(true);
    }
    if let LiteralIr::Guard(guard) = literal {
        return guard.evaluate_in(binding, evaluation, limits, budget, counters, location);
    }
    if let Some((left, relation, right)) = comparison(literal) {
        let [left, right] = evaluation.source_values(
            [left, right],
            |variable| binding.read(variable, location),
            limits,
            budget,
            counters,
            location,
        )?;
        return Ok(compare(&left, relation, &right));
    }
    if let LiteralIr::TupleCompare(left, relation, right) = literal {
        let equal = evaluation.source_tuple(
            (left, right),
            |variable| binding.read(variable, location),
            limits,
            budget,
            counters,
            location,
        )?;
        return Ok(equal == (*relation == Relation::Eq));
    }
    if let LiteralIr::Range {
        target,
        lower,
        upper,
        binder: false,
    } = literal
    {
        let [lower, upper] = evaluation.source_values(
            [lower, upper],
            |variable| binding.read(variable, location),
            limits,
            budget,
            counters,
            location,
        )?;
        let (Value::Number(lower), Value::Number(upper)) = (lower, upper) else {
            return Ok(false);
        };
        return Ok(
            matches!(binding.read(*target, location)?, Value::Number(number) if *number >= lower && *number <= upper),
        );
    }
    Ok(true)
}
