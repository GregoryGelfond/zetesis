//! Checked finite fact products; no rule binding enumeration occurs here.

use std::ops::RangeInclusive;

use crate::ProgramSite;
use themelios_program::program::{DefaultNegation, Head, LiteralInner, Statement};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::Symbol;
use themelios_program::term::{EvalError, Term as SourceTerm};
use zetesis_core::{AtomPattern, Predicate, Template, Term, Value};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::{AdmissionFailure, ExpansionFailure, ExpansionResource, ProfileFeature, compile};

pub(crate) fn facts(
    carrier: &WithProvenance<Statement>,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<Option<Vec<Template>>, ExpansionFailure> {
    let Statement::Rule(rule) = carrier.get() else {
        return Ok(None);
    };
    if rule.body().get().elements().next().is_some() {
        return Ok(None);
    }
    let Head::Literal(literal) = rule.head().get() else {
        return Ok(None);
    };
    if literal.negation != DefaultNegation::None {
        // A default-negated literal is a formula, not a fact producer. Leave
        // its admission and reduct meaning to the enclosing source profile.
        return Ok(None);
    }
    let LiteralInner::Atom(atom) = &literal.inner else {
        return Ok(None);
    };
    let atom = atom.get();
    let mut facts = Vec::new();
    let mut retained_bytes = 0_u128;
    for arguments in atom.alternatives() {
        let sizes = arguments
            .iter()
            .map(|term| size(term, budget, location))
            .collect::<Result<Vec<_>, _>>()?;
        // Every argument is validated even if another has an empty interval.
        let count = sizes
            .iter()
            .fold(1_u128, |count, (size, _)| count.saturating_mul(*size));
        budget.charge(ExpansionResource::Templates, count, location)?;
        let intermediate = sizes
            .iter()
            .fold(0_u128, |sum, (size, _)| sum.saturating_add(*size));
        budget.charge(
            ExpansionResource::Values,
            intermediate.saturating_add(count.saturating_mul(arguments.len() as u128)),
            location,
        )?;
        if count == 0 {
            continue;
        }
        budget.charge(
            ExpansionResource::ScalarBytes,
            (atom.name.as_str().len() as u128).saturating_mul(count),
            location,
        )?;
        // The complete returned fact family remains live across atom alternatives.
        // Bound its carriers and argument payload together with this alternative's
        // value columns and cursor before constructing either population.
        let payload = sizes
            .iter()
            .fold(0_u128, |sum, (_, bytes)| sum.saturating_add(*bytes));
        let output = count.saturating_mul(
            std::mem::size_of::<Template>() as u128
                + arguments.len() as u128 * std::mem::size_of::<Term>() as u128
                + atom.name.as_str().len() as u128
                + payload,
        );
        retained_bytes = retained_bytes.saturating_add(output);
        let columns = intermediate
            .saturating_mul(std::mem::size_of::<Value>() as u128)
            .saturating_add(payload)
            .saturating_add(
                arguments.len() as u128
                    * (std::mem::size_of::<Vec<Value>>() + std::mem::size_of::<usize>()) as u128,
            );
        budget.check_family(retained_bytes.saturating_add(columns), location)?;
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            arguments.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|error| AdmissionFailure::Construction { error, location })?;
        let alternatives = arguments
            .iter()
            .map(|term| values(term, budget, location))
            .collect::<Result<Vec<_>, _>>()?;
        let mut cursor = vec![0; arguments.len()];
        for _ in 0..count {
            budget.poll(location)?;
            let mut terms = Vec::with_capacity(arguments.len());
            for (values, index) in alternatives.iter().zip(&cursor) {
                let value = &values[*index];
                budget.charge(ExpansionResource::ScalarBytes, value_bytes(value), location)?;
                terms.push(Term::Constant(value.clone()));
            }
            let head = AtomPattern::new(predicate.clone(), terms)
                .map_err(|error| AdmissionFailure::Construction { error, location })?;
            facts.push(Template::new(
                Some(head),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ));
            for (index, values) in cursor.iter_mut().zip(&alternatives).rev() {
                *index += 1;
                if *index < values.len() {
                    break;
                }
                *index = 0;
            }
        }
    }
    Ok(Some(facts))
}

fn size(
    term: &SourceTerm,
    budget: &Budget,
    location: ProgramSite,
) -> Result<(u128, u128), ExpansionFailure> {
    let mut pending = vec![term];
    let mut size = 0_u128;
    let mut bytes = 0_u128;
    while let Some(term) = pending.pop() {
        budget.poll(location)?;
        match term {
            SourceTerm::Pool(items) => pending.extend(items),
            SourceTerm::Interval { lower, upper } => {
                let range = interval(lower, upper, location)?;
                let width = range.as_ref().map_or(0, crate::integer_range::width);
                size = size.saturating_add(u128::from(width));
            }
            SourceTerm::Symbolic(symbol) => {
                compile::validate_scalar(symbol, location)?;
                size = size.saturating_add(1);
                bytes = bytes.saturating_add(crate::structural_value::symbol_bytes(symbol));
            }
            SourceTerm::Variable(variable) => {
                return Err(ExpansionFailure::Evaluation {
                    error: EvalError::NotGround {
                        variable: variable.clone(),
                    },
                    location,
                });
            }
            _ => return Err(unsupported(ProfileFeature::Term, location).into()),
        }
    }
    Ok((size, bytes))
}

fn values(
    term: &SourceTerm,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<Vec<Value>, ExpansionFailure> {
    let mut pending = vec![term];
    let mut values = Vec::new();
    while let Some(term) = pending.pop() {
        budget.poll(location)?;
        match term {
            SourceTerm::Pool(items) => pending.extend(items.iter().rev()),
            SourceTerm::Interval { lower, upper } => {
                if let Some(range) = interval(lower, upper, location)? {
                    if budget.cancellation().is_some() {
                        for number in range {
                            budget.poll(location)?;
                            values.push(Value::Number(number));
                        }
                    } else {
                        values.extend(range.map(Value::Number));
                    }
                }
            }
            SourceTerm::Symbolic(symbol) => {
                let bytes = crate::structural_value::symbol_bytes(symbol);
                budget.charge(ExpansionResource::ScalarBytes, bytes, location)?;
                values.push(compile::scalar(symbol, location)?);
            }
            _ => return Err(unsupported(ProfileFeature::Term, location).into()),
        }
    }
    Ok(values)
}

fn interval(
    lower: &SourceTerm,
    upper: &SourceTerm,
    location: ProgramSite,
) -> Result<Option<RangeInclusive<i32>>, ExpansionFailure> {
    // Validate both endpoints before classifying an empty range. A nonnumeric
    // value cannot hide an unbound variable or an unsupported endpoint form.
    let lower = endpoint(lower, location)?;
    let upper = endpoint(upper, location)?;
    Ok(crate::integer_range::inclusive(lower, upper))
}

fn endpoint(term: &SourceTerm, location: ProgramSite) -> Result<Option<i32>, ExpansionFailure> {
    match term {
        SourceTerm::Symbolic(symbol) => {
            compile::validate_scalar(symbol, location)?;
            Ok(match symbol {
                Symbol::Number(number) => Some(*number),
                _ => None,
            })
        }
        SourceTerm::Variable(variable) => Err(ExpansionFailure::Evaluation {
            error: EvalError::NotGround {
                variable: variable.clone(),
            },
            location,
        }),
        _ => Err(unsupported(ProfileFeature::Term, location).into()),
    }
}

fn value_bytes(value: &Value) -> u128 {
    match value {
        Value::Infimum | Value::Supremum | Value::Number(_) => 0,
        Value::Structured(value) => value.payload_bytes() as u128,
        Value::String(value) | Value::Symbol(value) => value.len() as u128,
    }
}

#[cfg(test)]
mod tests;
